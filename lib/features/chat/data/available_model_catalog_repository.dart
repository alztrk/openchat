import 'package:openchat/features/chat/domain/chatgpt_connection.dart';
import 'package:openchat/features/settings/data/api_compatible_provider_key_store.dart';
import 'package:openchat/features/settings/data/chat_gpt_api_key_store.dart';
import 'package:openchat/features/settings/domain/chat_gpt_api_key_connection.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

const _localEngineIds = <String>{'llama_cpp'};

class AvailableModelCatalogResult {
  const AvailableModelCatalogResult({
    required this.models,
    required this.hasUnavailableSources,
  });

  final List<ChatGptModel> models;
  final bool hasUnavailableSources;
}

class AvailableModelCatalogRepository {
  const AvailableModelCatalogRepository({
    required this.service,
    required this.chatGptApiKeyStore,
    required this.apiCompatibleProviderKeyStore,
  });

  final OpenChatServiceClient service;
  final ChatGptApiKeyStore? chatGptApiKeyStore;
  final ApiCompatibleProviderKeyStore? apiCompatibleProviderKeyStore;

  Future<AvailableModelCatalogResult> load({bool forceRefresh = false}) async {
    final loadedModels = <ChatGptModel>[];
    var sourceFailed = false;

    Future<void> collectCatalog({
      required String routeProviderId,
      required String groupId,
      required Future<Map<String, Object?>> Function() load,
      String? connectionId,
      String? workspaceId,
      String? sourceLabel,
      bool Function(ChatGptModel)? includeModel,
    }) async {
      try {
        final models = _parseCatalog(
          await load(),
          routeProviderId: routeProviderId,
          groupId: groupId,
          connectionId: connectionId,
          workspaceId: workspaceId,
          sourceLabel: sourceLabel,
        );
        loadedModels.addAll(
          models.where(
            (model) =>
                model.isAvailable &&
                (includeModel == null || includeModel(model)),
          ),
        );
      } on OpenChatServiceException {
        sourceFailed = true;
      } on ChatGptApiKeyStorageException {
        sourceFailed = true;
      } on ApiCompatibleProviderKeyStorageException {
        sourceFailed = true;
      } on FormatException {
        sourceFailed = true;
      }
    }

    var oauthConnections = const <ChatGptConnection>[];
    try {
      final response = await service.call('chatgpt.connections.list');
      final rawConnections = response['connections'];
      if (rawConnections is! List<Object?>) {
        throw const FormatException('The ChatGPT connection list was invalid.');
      }
      oauthConnections = rawConnections
          .map((value) => ChatGptConnection.fromJson(_objectMap(value)))
          .where(
            (connection) =>
                connection.authStatus == 'active' && connection.isSelected,
          )
          .take(1)
          .toList(growable: false);
    } on OpenChatServiceException {
      sourceFailed = true;
    } on FormatException {
      sourceFailed = true;
    }

    var apiConnections = const <ChatGptApiKeyConnection>[];
    try {
      apiConnections =
          await chatGptApiKeyStore?.readConnections() ??
          const <ChatGptApiKeyConnection>[];
    } on ChatGptApiKeyStorageException {
      sourceFailed = true;
    }

    var compatibleProviderIds = const <String>{};
    try {
      compatibleProviderIds =
          await apiCompatibleProviderKeyStore?.readConfiguredProviderIds() ??
          const <String>{};
    } on ApiCompatibleProviderKeyStorageException {
      sourceFailed = true;
    }

    List<CustomCompatibleProvider> customProviders = const [];
    try {
      customProviders =
          await apiCompatibleProviderKeyStore?.readCustomProviders() ??
          const <CustomCompatibleProvider>[];
    } on ApiCompatibleProviderKeyStorageException {
      sourceFailed = true;
    }

    final tasks = <Future<void>>[
      collectCatalog(
        routeProviderId: 'opencode',
        groupId: 'models',
        includeModel: (model) => model.groupId == 'free',
        load: () => service.call(
          'opencode.models.list',
          params: <String, Object?>{if (forceRefresh) 'forceRefresh': true},
        ),
      ),
    ];

    for (final connection in oauthConnections) {
      final selectedWorkspaces = connection.workspaces
          .where((workspace) => workspace.isSelected)
          .take(1);
      for (final workspace in selectedWorkspaces) {
        tasks.add(
          collectCatalog(
            routeProviderId: 'chatgpt',
            groupId: 'oauth',
            connectionId: connection.id,
            workspaceId: workspace.id,
            sourceLabel: _firstNonEmpty([
              connection.displayName,
              connection.email,
              workspace.displayName,
            ]),
            load: () => service.call(
              'chatgpt.models.list',
              params: <String, Object?>{
                'connectionId': connection.id,
                'workspaceId': workspace.id,
                if (forceRefresh) 'forceRefresh': true,
              },
            ),
          ),
        );
      }
    }

    for (final connection in apiConnections) {
      tasks.add(
        collectCatalog(
          routeProviderId: 'chatgpt_api',
          groupId: 'api',
          connectionId: connection.id,
          sourceLabel: apiConnections.length > 1 && connection.keySuffix != null
              ? '••••${connection.keySuffix}'
              : null,
          load: () async {
            final apiKey = await chatGptApiKeyStore?.readApiKey(connection.id);
            if (apiKey == null) {
              throw const ChatGptApiKeyStorageException();
            }
            return service.call(
              'chatgpt.api.models.list',
              params: <String, Object?>{
                'apiKey': apiKey,
                if (forceRefresh) 'forceRefresh': true,
              },
            );
          },
        ),
      );
    }

    for (final providerId in compatibleProviderIds) {
      tasks.add(
        collectCatalog(
          routeProviderId: providerId,
          groupId: 'models',
          connectionId: providerId,
          load: () async {
            final apiKey = await apiCompatibleProviderKeyStore?.readApiKey(
              providerId,
            );
            if (apiKey == null) {
              throw const ApiCompatibleProviderKeyStorageException();
            }
            return service.call(
              'compatible.models.list',
              params: <String, Object?>{
                'providerId': providerId,
                'apiKey': apiKey,
                if (forceRefresh) 'forceRefresh': true,
              },
            );
          },
        ),
      );
    }

    for (final provider in customProviders) {
      tasks.add(
        collectCatalog(
          routeProviderId: provider.id,
          groupId: 'models',
          connectionId: provider.id,
          sourceLabel: provider.name,
          load: () async {
            final apiKey = await apiCompatibleProviderKeyStore?.readApiKey(
              provider.id,
            );
            return service.call(
              'compatible.models.list',
              params: <String, Object?>{
                'providerId': provider.id,
                'baseUrl': provider.baseUrl,
                'protocol': provider.protocol.name,
                ...?switch (apiKey) {
                  final key? => <String, Object?>{'apiKey': key},
                  _ => null,
                },
                if (forceRefresh) 'forceRefresh': true,
              },
            );
          },
        ),
      );
    }

    for (final engineId in _localEngineIds) {
      tasks.add(
        collectCatalog(
          routeProviderId: engineId,
          groupId: 'models',
          load: () => service.call(
            'local.engines.chat_models.list',
            params: <String, Object?>{'engineId': engineId},
          ),
        ),
      );
    }

    await Future.wait(tasks);
    final uniqueModels = <String, ChatGptModel>{};
    for (final model in loadedModels) {
      uniqueModels.putIfAbsent(model.routeKey, () => model);
    }
    final models = uniqueModels.values.toList()
      ..sort((left, right) {
        final providerOrder = left.providerId.compareTo(right.providerId);
        return providerOrder != 0
            ? providerOrder
            : left.displayName.compareTo(right.displayName);
      });
    return AvailableModelCatalogResult(
      models: List.unmodifiable(models),
      hasUnavailableSources: sourceFailed,
    );
  }

  List<ChatGptModel> _parseCatalog(
    Map<String, Object?> response, {
    required String routeProviderId,
    required String groupId,
    String? connectionId,
    String? workspaceId,
    String? sourceLabel,
  }) {
    final rawModels = response['models'];
    final freshness = response['freshness'];
    if (rawModels is! List<Object?> ||
        (freshness != 'current' && freshness != 'stale')) {
      throw const FormatException('A model catalog response was invalid.');
    }
    return rawModels
        .map((value) => ChatGptModel.fromJson(_objectMap(value)))
        .map(
          (model) => model.withRoute(
            providerId: routeProviderId,
            connectionId: connectionId,
            workspaceId: workspaceId,
            sourceLabel: sourceLabel,
            groupId: model.groupId ?? groupId,
          ),
        )
        .toList(growable: false);
  }
}

String? _firstNonEmpty(List<String?> values) {
  for (final value in values) {
    if (value != null && value.trim().isNotEmpty) return value;
  }
  return null;
}

Map<String, Object?> _objectMap(Object? value) {
  if (value is! Map) {
    throw const FormatException('A model catalog item was invalid.');
  }
  final result = <String, Object?>{};
  for (final entry in value.entries) {
    if (entry.key is! String) {
      throw const FormatException('A model catalog key was invalid.');
    }
    result[entry.key as String] = entry.value;
  }
  return result;
}
