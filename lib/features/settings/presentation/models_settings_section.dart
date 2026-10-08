import 'dart:async';

import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/openchat_toast.dart';
import 'package:openchat/features/chat/data/chat_repository.dart';
import 'package:openchat/features/chat/domain/chatgpt_connection.dart';
import 'package:openchat/features/chat/domain/default_model_preference.dart';
import 'package:openchat/features/chat/domain/model_favorite.dart';
import 'package:openchat/features/chat/presentation/widgets/provider_icon.dart';
import 'package:openchat/features/settings/data/api_compatible_provider_key_store.dart';
import 'package:openchat/features/settings/data/chat_gpt_api_key_store.dart';
import 'package:openchat/features/settings/data/open_code_api_key_store.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

class ModelsSettingsSection extends StatefulWidget {
  const ModelsSettingsSection({
    super.key,
    required this.serviceClient,
    required this.chatGptApiKeyStore,
    required this.apiCompatibleProviderKeyStore,
    required this.openCodeApiKeyStore,
    required this.settingsPreferences,
    this.chatRepository,
    this.onChanged,
  });

  final OpenChatServiceClient? serviceClient;
  final ChatGptApiKeyStore? chatGptApiKeyStore;
  final ApiCompatibleProviderKeyStore? apiCompatibleProviderKeyStore;
  final OpenCodeApiKeyStore? openCodeApiKeyStore;
  final SettingsPreferences settingsPreferences;
  final ChatRepository? chatRepository;
  final Future<void> Function()? onChanged;

  @override
  State<ModelsSettingsSection> createState() => _ModelsSettingsSectionState();
}

class _ModelsSettingsSectionState extends State<ModelsSettingsSection> {
  final _searchController = TextEditingController();
  bool _isLoading = true;
  String _searchQuery = '';
  Map<String, List<ChatGptModel>> _providerModels = {};
  Set<String> _hiddenModelKeys = {};
  DefaultModelPreference? _defaultModel;
  late Stream<List<FavoriteModel>>? _favoriteModelsStream;
  final Set<String> _favoriteChangesInProgress = {};
  final Map<String, String> _providerErrors = {};

  @override
  void initState() {
    super.initState();
    _favoriteModelsStream = widget.chatRepository?.watchModelFavorites();
    unawaited(_loadData());
  }

  @override
  void didUpdateWidget(covariant ModelsSettingsSection oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.chatRepository != widget.chatRepository) {
      _favoriteModelsStream = widget.chatRepository?.watchModelFavorites();
    }
  }

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  Map<String, Object?> _objectMap(Object? value) => switch (value) {
    final Map<String, Object?> map => map,
    final Map<Object?, Object?> map => map.map(
      (key, val) => MapEntry(key.toString(), val),
    ),
    _ => throw const FormatException('Expected an object map.'),
  };

  Future<void> _loadData({bool forceRefresh = false}) async {
    if (!mounted) return;
    setState(() {
      _isLoading = true;
      _providerErrors.clear();
    });

    final hiddenKeys = await widget.settingsPreferences.readHiddenModelKeys();
    final defaultModel = await widget.settingsPreferences.readDefaultModel();

    final service = widget.serviceClient;
    final modelsByProvider = <String, List<ChatGptModel>>{};

    if (service != null) {
      // 1. OpenCode models
      try {
        final apiKey = await widget.openCodeApiKeyStore?.readApiKey();
        final response = await service.call(
          'opencode.models.list',
          params: <String, Object?>{
            ...?switch (apiKey) {
              final key? => <String, Object?>{'apiKey': key},
              _ => null,
            },
            if (forceRefresh) 'forceRefresh': true,
          },
        );
        final rawModels = response['models'];
        if (rawModels is List<Object?>) {
          modelsByProvider['opencode'] = rawModels
              .map((item) => ChatGptModel.fromJson(_objectMap(item)))
              .map(
                (model) => model.withRoute(
                  providerId: 'opencode',
                  groupId: model.groupId ?? 'free',
                ),
              )
              .toList(growable: false);
        }
      } catch (error) {
        _providerErrors['opencode'] = error.toString();
      }

      // 2. ChatGPT models
      final chatGptModels = <ChatGptModel>[];
      // 2a. OAuth
      try {
        final connResponse = await service.call('chatgpt.connections.list');
        final rawConnections = connResponse['connections'];
        if (rawConnections is List<Object?>) {
          for (final rawConn in rawConnections) {
            final conn = _objectMap(rawConn);
            final connId = conn['id'] as String?;
            final workspaces = conn['workspaces'];
            if (connId != null && workspaces is List<Object?>) {
              for (final rawWs in workspaces) {
                final ws = _objectMap(rawWs);
                final wsId = ws['id'] as String?;
                if (wsId != null && ws['isSelected'] == true) {
                  try {
                    final response = await service.call(
                      'chatgpt.models.list',
                      params: <String, Object?>{
                        'connectionId': connId,
                        'workspaceId': wsId,
                        if (forceRefresh) 'forceRefresh': true,
                      },
                    );
                    final raw = response['models'];
                    if (raw is List<Object?>) {
                      chatGptModels.addAll(
                        raw
                            .map(
                              (item) => ChatGptModel.fromJson(_objectMap(item)),
                            )
                            .map(
                              (model) => model.withRoute(
                                providerId: 'chatgpt',
                                groupId: 'oauth',
                                connectionId: connId,
                                workspaceId: wsId,
                                sourceLabel:
                                    ws['displayName'] as String? ??
                                    conn['displayName'] as String?,
                              ),
                            ),
                      );
                    }
                  } catch (e) {
                    _providerErrors['chatgpt'] = e.toString();
                  }
                }
              }
            }
          }
        }
      } catch (e) {
        _providerErrors['chatgpt'] = e.toString();
      }

      // 2b. ChatGPT API Keys
      try {
        final apiConnections =
            await widget.chatGptApiKeyStore?.readConnections() ?? const [];
        for (final conn in apiConnections) {
          final apiKey = await widget.chatGptApiKeyStore?.readApiKey(conn.id);
          if (apiKey != null) {
            try {
              final response = await service.call(
                'chatgpt.api.models.list',
                params: <String, Object?>{
                  'apiKey': apiKey,
                  if (forceRefresh) 'forceRefresh': true,
                },
              );
              final raw = response['models'];
              if (raw is List<Object?>) {
                chatGptModels.addAll(
                  raw
                      .map((item) => ChatGptModel.fromJson(_objectMap(item)))
                      .map(
                        (model) => model.withRoute(
                          providerId: 'chatgpt_api',
                          groupId: 'api',
                          connectionId: conn.id,
                          sourceLabel:
                              apiConnections.length > 1 &&
                                  conn.keySuffix != null
                              ? '••••${conn.keySuffix}'
                              : null,
                        ),
                      ),
                );
              }
            } catch (e) {
              _providerErrors['chatgpt'] = e.toString();
            }
          }
        }
      } catch (e) {
        _providerErrors['chatgpt'] = e.toString();
      }

      if (chatGptModels.isNotEmpty) {
        modelsByProvider['chatgpt'] = chatGptModels;
      }

      // 3. Compatible providers (Gemini, Groq, Cerebras, OpenRouter)
      try {
        final configuredProviders =
            await widget.apiCompatibleProviderKeyStore
                ?.readConfiguredProviderIds() ??
            const <String>{};
        for (final providerId in configuredProviders) {
          final apiKey = await widget.apiCompatibleProviderKeyStore?.readApiKey(
            providerId,
          );
          if (apiKey != null) {
            try {
              final response = await service.call(
                'compatible.models.list',
                params: <String, Object?>{
                  'providerId': providerId,
                  'apiKey': apiKey,
                  if (forceRefresh) 'forceRefresh': true,
                },
              );
              final raw = response['models'];
              if (raw is List<Object?>) {
                modelsByProvider[providerId] = raw
                    .map((item) => ChatGptModel.fromJson(_objectMap(item)))
                    .map(
                      (model) => model.withRoute(
                        providerId: providerId,
                        groupId: 'models',
                        connectionId: providerId,
                      ),
                    )
                    .toList(growable: false);
              }
            } catch (e) {
              _providerErrors[providerId] = e.toString();
            }
          }
        }
      } catch (error) {
        _providerErrors['api_compatible'] = error.toString();
      }
    }

    if (!mounted) return;
    setState(() {
      _isLoading = false;
      _providerModels = modelsByProvider;
      _hiddenModelKeys = hiddenKeys;
      _defaultModel = defaultModel;
    });
  }

  Future<void> _toggleHideModel(ChatGptModel model) async {
    final key = model.routeKey;
    final updated = Set<String>.from(_hiddenModelKeys);
    final wasHidden = updated.contains(key);

    if (wasHidden) {
      updated.remove(key);
      showOpenChatToast(
        context,
        context.openchatL10n.modelUnhidden(model.displayName),
        type: OpenChatToastType.info,
      );
    } else {
      updated.add(key);
      // If this model was the default, remove default
      if (_defaultModel?.routeKey == key) {
        await _clearDefaultModel(silent: true);
      }
      if (mounted) {
        showOpenChatToast(
          context,
          context.openchatL10n.modelHidden(model.displayName),
          type: OpenChatToastType.info,
        );
      }
    }

    setState(() => _hiddenModelKeys = updated);
    await widget.settingsPreferences.writeHiddenModelKeys(updated);
    await widget.onChanged?.call();
  }

  Future<void> _setDefaultModel(ChatGptModel model) async {
    final preference = DefaultModelPreference(
      providerId: model.providerId,
      modelId: model.id,
      displayName: model.displayName,
      connectionId: model.connectionId,
      workspaceId: model.workspaceId,
      apiKeyConnectionId: model.providerId == 'chatgpt_api'
          ? model.connectionId
          : null,
    );

    // If it was hidden, unhide it
    Set<String>? updatedHidden;
    if (_hiddenModelKeys.contains(model.routeKey)) {
      updatedHidden = Set<String>.from(_hiddenModelKeys)
        ..remove(model.routeKey);
      await widget.settingsPreferences.writeHiddenModelKeys(updatedHidden);
    }

    setState(() {
      _defaultModel = preference;
      if (updatedHidden != null) _hiddenModelKeys = updatedHidden;
    });

    await widget.settingsPreferences.writeDefaultModel(preference);
    await widget.onChanged?.call();

    if (mounted) {
      showOpenChatToast(
        context,
        context.openchatL10n.defaultModelUpdated(model.displayName),
        type: OpenChatToastType.success,
      );
    }
  }

  Future<void> _clearDefaultModel({bool silent = false}) async {
    setState(() => _defaultModel = null);
    await widget.settingsPreferences.writeDefaultModel(null);
    await widget.onChanged?.call();
    if (!silent && mounted) {
      showOpenChatToast(
        context,
        context.openchatL10n.defaultModelCleared,
        type: OpenChatToastType.info,
      );
    }
  }

  Future<void> _toggleModelFavorite(
    ChatGptModel model, {
    required bool isFavorite,
  }) async {
    final repository = widget.chatRepository;
    final key = model.routeKey;
    if (repository == null || !_favoriteChangesInProgress.add(key)) return;

    setState(() {});
    try {
      await repository.setModelFavorite(
        providerId: model.providerId,
        modelId: model.id,
        displayName: model.displayName,
        sourceConnectionId: model.connectionId,
        isFavorite: !isFavorite,
      );
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'model_favorites',
          context: ErrorDescription('while saving a model favorite'),
        ),
      );
      if (mounted) {
        showOpenChatToast(
          context,
          context.openchatL10n.chatHistoryUnavailable,
          type: OpenChatToastType.error,
        );
      }
    } finally {
      _favoriteChangesInProgress.remove(key);
      if (mounted) setState(() {});
    }
  }

  String _providerLabel(String providerId, BuildContext context) {
    final l10n = context.openchatL10n;
    return switch (providerId) {
      'opencode' => l10n.openCodeProvider,
      'chatgpt' || 'chatgpt_api' => l10n.chatGptProvider,
      'gemini' => l10n.geminiProvider,
      'groq' => l10n.groqProvider,
      'cerebras' => l10n.cerebrasProvider,
      'openrouter' => l10n.openRouterProvider,
      'mistral' => l10n.mistralProvider,
      _ => providerId,
    };
  }

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final l10n = context.openchatL10n;

    return StreamBuilder<List<FavoriteModel>>(
      stream: _favoriteModelsStream,
      builder: (context, favoriteSnapshot) => _buildModelsContent(
        context,
        palette,
        l10n,
        favoriteModels: favoriteSnapshot.data ?? const <FavoriteModel>[],
        canManageFavorites:
            widget.chatRepository != null &&
            favoriteSnapshot.hasData &&
            !favoriteSnapshot.hasError,
      ),
    );
  }

  Widget _buildModelsContent(
    BuildContext context,
    OpenChatPalette palette,
    AppLocalizations l10n, {
    required List<FavoriteModel> favoriteModels,
    required bool canManageFavorites,
  }) {
    final query = _searchQuery.trim().toLowerCase();

    var filteredModelCount = 0;
    for (final models in _providerModels.values) {
      filteredModelCount += models.where((model) {
        if (query.isEmpty) return true;
        return model.displayName.toLowerCase().contains(query) ||
            model.id.toLowerCase().contains(query) ||
            (model.sourceLabel?.toLowerCase().contains(query) ?? false);
      }).length;
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          l10n.modelsDescription,
          style: TextStyle(
            color: palette.secondaryText,
            fontSize: 13,
            height: 1.45,
          ),
        ),
        if (_providerErrors.isNotEmpty) ...[
          const SizedBox(height: 12),
          Container(
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(
              color: palette.surface,
              borderRadius: BorderRadius.circular(OpenChatRadii.card),
              border: Border.all(color: palette.controlBorder),
            ),
            child: Wrap(
              alignment: WrapAlignment.spaceBetween,
              crossAxisAlignment: WrapCrossAlignment.center,
              spacing: 12,
              runSpacing: 8,
              children: [
                ConstrainedBox(
                  constraints: const BoxConstraints(maxWidth: 560),
                  child: Text(
                    l10n.modelCatalogUnavailable,
                    style: TextStyle(color: palette.text),
                  ),
                ),
                OutlinedButton.icon(
                  onPressed: _isLoading
                      ? null
                      : () => _loadData(forceRefresh: true),
                  icon: const Icon(LucideIcons.refreshCw),
                  label: Text(l10n.retry),
                ),
              ],
            ),
          ),
        ],
        const SizedBox(height: 16),
        LayoutBuilder(
          builder: (context, constraints) {
            final stacked =
                constraints.maxWidth /
                    MediaQuery.textScalerOf(context).scale(1) <
                560;
            final search = ConstrainedBox(
              constraints: const BoxConstraints(minHeight: 40),
              child: TextField(
                controller: _searchController,
                onChanged: (val) => setState(() => _searchQuery = val),
                style: TextStyle(color: palette.text, fontSize: 13),
                decoration: InputDecoration(
                  labelText: l10n.searchModels,
                  hintText: l10n.modelSearchHint,
                  hintStyle: TextStyle(
                    color: palette.secondaryText,
                    fontSize: 13,
                  ),
                  prefixIcon: Icon(
                    LucideIcons.search,
                    size: 18,
                    color: palette.secondaryIcon,
                  ),
                  prefixIconConstraints: const BoxConstraints(minWidth: 36),
                  filled: true,
                  fillColor: palette.navigation,
                  isDense: true,
                  contentPadding: const EdgeInsets.symmetric(
                    horizontal: 10,
                    vertical: 9,
                  ),
                ),
              ),
            );
            final refresh = OutlinedButton.icon(
              onPressed: _isLoading
                  ? null
                  : () => _loadData(forceRefresh: true),
              icon: _isLoading
                  ? Semantics(
                      liveRegion: true,
                      label: l10n.modelsLoading,
                      child: const SizedBox.square(
                        dimension: 14,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      ),
                    )
                  : const Icon(LucideIcons.refreshCw, size: 16),
              label: Text(l10n.refreshModels),
            );
            if (stacked) {
              return Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  search,
                  const SizedBox(height: 12),
                  Align(alignment: Alignment.centerLeft, child: refresh),
                ],
              );
            }
            return Row(
              children: [
                Expanded(child: search),
                const SizedBox(width: 10),
                refresh,
              ],
            );
          },
        ),
        const SizedBox(height: 16),
        if (_isLoading)
          Padding(
            padding: EdgeInsets.symmetric(vertical: 32),
            child: Center(
              child: Semantics(
                liveRegion: true,
                label: l10n.modelsLoading,
                child: const CircularProgressIndicator(strokeWidth: 2),
              ),
            ),
          )
        else if (filteredModelCount == 0)
          Card(
            margin: EdgeInsets.zero,
            color: palette.surface,
            elevation: 0,
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(10),
              side: BorderSide(color: palette.border),
            ),
            child: Padding(
              padding: const EdgeInsets.all(24),
              child: Center(
                child: Text(
                  query.isNotEmpty
                      ? l10n.noModelsFound
                      : _providerErrors.isNotEmpty
                      ? l10n.modelCatalogUnavailable
                      : l10n.noConnectedProviders,
                  style: TextStyle(color: palette.secondaryText, fontSize: 13),
                ),
              ),
            ),
          )
        else ...[
          // Group by provider order
          for (final providerId in [
            'opencode',
            'chatgpt',
            'gemini',
            'groq',
            'cerebras',
            'openrouter',
            'mistral',
          ]) ...[
            if (_providerModels.containsKey(providerId)) ...[
              _buildProviderCard(
                context,
                providerId: providerId,
                models: _providerModels[providerId]!,
                query: query,
                palette: palette,
                favoriteModels: favoriteModels,
                canManageFavorites: canManageFavorites,
              ),
              const SizedBox(height: 16),
            ],
          ],
        ],
      ],
    );
  }

  Widget _buildProviderCard(
    BuildContext context, {
    required String providerId,
    required List<ChatGptModel> models,
    required String query,
    required OpenChatPalette palette,
    required List<FavoriteModel> favoriteModels,
    required bool canManageFavorites,
  }) {
    final l10n = context.openchatL10n;
    final filtered = models
        .where((model) {
          if (query.isEmpty) return true;
          return model.displayName.toLowerCase().contains(query) ||
              model.id.toLowerCase().contains(query) ||
              (model.sourceLabel?.toLowerCase().contains(query) ?? false);
        })
        .toList(growable: false);

    if (filtered.isEmpty) return const SizedBox.shrink();

    return Card(
      margin: EdgeInsets.zero,
      color: palette.surface,
      elevation: 0,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(10),
        side: BorderSide(color: palette.border),
      ),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                ProviderIcon(
                  providerId: providerId,
                  color: palette.text,
                  size: 20,
                ),
                const SizedBox(width: 10),
                Text(
                  _providerLabel(providerId, context),
                  style: Theme.of(context).textTheme.titleMedium,
                ),
                const Spacer(),
                Text(
                  '${filtered.length} model',
                  style: TextStyle(color: palette.secondaryText, fontSize: 12),
                ),
              ],
            ),
            const SizedBox(height: 12),
            Divider(height: 1, color: palette.border),
            const SizedBox(height: 8),
            for (var i = 0; i < filtered.length; i++) ...[
              if (i > 0) Divider(height: 1, color: palette.border),
              _buildModelRow(
                context,
                filtered[i],
                palette,
                l10n,
                favoriteModels: favoriteModels,
                canManageFavorites: canManageFavorites,
              ),
            ],
          ],
        ),
      ),
    );
  }

  Widget _buildModelRow(
    BuildContext context,
    ChatGptModel model,
    OpenChatPalette palette,
    AppLocalizations l10n, {
    required List<FavoriteModel> favoriteModels,
    required bool canManageFavorites,
  }) {
    final key = model.routeKey;
    final isHidden = _hiddenModelKeys.contains(key);
    final isDefault = _defaultModel?.routeKey == key;
    final isFavorite = favoriteModels.any(
      (favorite) =>
          favorite.providerId == model.providerId &&
          favorite.modelId == model.id &&
          (!_isApiKeyModel(model.providerId) ||
              favorite.sourceConnectionId == model.connectionId),
    );
    final favoriteChangeInProgress = _favoriteChangesInProgress.contains(key);

    final openCodeTier = model.providerId == 'opencode'
        ? (model.groupId == 'free'
              ? l10n.openCodeFreeModel
              : l10n.openCodePaidModel)
        : null;

    final contextWindowText = model.contextWindow != null
        ? '${(model.contextWindow! / 1000).round()}k context'
        : null;

    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 8),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Wrap(
                  crossAxisAlignment: WrapCrossAlignment.center,
                  spacing: 6,
                  runSpacing: 4,
                  children: [
                    Text(
                      model.displayName,
                      style: TextStyle(
                        color: isHidden ? palette.secondaryText : palette.text,
                        fontWeight: FontWeight.w600,
                        fontSize: 13,
                        decoration: isHidden
                            ? TextDecoration.lineThrough
                            : null,
                      ),
                    ),
                    if (isDefault)
                      Container(
                        padding: const EdgeInsets.symmetric(
                          horizontal: 6,
                          vertical: 2,
                        ),
                        decoration: BoxDecoration(
                          color: palette.accent.withValues(alpha: 0.15),
                          borderRadius: BorderRadius.circular(4),
                          border: Border.all(color: palette.accent),
                        ),
                        child: Text(
                          l10n.defaultModel,
                          style: TextStyle(
                            color: palette.accent,
                            fontSize: OpenChatTypography.metadata,
                            fontWeight: FontWeight.w600,
                          ),
                        ),
                      ),
                    if (isHidden)
                      Container(
                        padding: const EdgeInsets.symmetric(
                          horizontal: 6,
                          vertical: 2,
                        ),
                        decoration: BoxDecoration(
                          color: palette.navigation,
                          borderRadius: BorderRadius.circular(4),
                          border: Border.all(color: palette.border),
                        ),
                        child: Text(
                          l10n.hiddenModel,
                          style: TextStyle(
                            color: palette.secondaryText,
                            fontSize: OpenChatTypography.metadata,
                          ),
                        ),
                      ),
                    if (openCodeTier != null)
                      Container(
                        padding: const EdgeInsets.symmetric(
                          horizontal: 5,
                          vertical: 2,
                        ),
                        decoration: BoxDecoration(
                          color: palette.navigation,
                          borderRadius: BorderRadius.circular(4),
                        ),
                        child: Text(
                          openCodeTier,
                          style: TextStyle(
                            color: palette.secondaryText,
                            fontSize: OpenChatTypography.metadata,
                          ),
                        ),
                      ),
                    if (model.sourceLabel != null)
                      Container(
                        padding: const EdgeInsets.symmetric(
                          horizontal: 5,
                          vertical: 2,
                        ),
                        decoration: BoxDecoration(
                          color: palette.navigation,
                          borderRadius: BorderRadius.circular(4),
                        ),
                        child: Text(
                          model.sourceLabel!,
                          style: TextStyle(
                            color: palette.secondaryText,
                            fontSize: OpenChatTypography.metadata,
                          ),
                        ),
                      ),
                    if (contextWindowText != null)
                      Text(
                        contextWindowText,
                        style: TextStyle(
                          color: palette.secondaryText,
                          fontSize: OpenChatTypography.metadata,
                        ),
                      ),
                  ],
                ),
                const SizedBox(height: 2),
                Text(
                  model.id,
                  style: TextStyle(
                    fontFamily: OpenChatTypography.codeFontFamily,
                    color: palette.secondaryText,
                    fontSize: OpenChatTypography.code,
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(width: 8),
          IconButton(
            tooltip: canManageFavorites
                ? isFavorite
                      ? l10n.removeModelFavorite
                      : l10n.addModelFavorite
                : l10n.chatHistoryUnavailable,
            onPressed: canManageFavorites && !favoriteChangeInProgress
                ? () => unawaited(
                    _toggleModelFavorite(model, isFavorite: isFavorite),
                  )
                : null,
            icon: Icon(
              isFavorite ? LucideIcons.star : LucideIcons.starOff,
              color: isFavorite ? palette.accent : palette.secondaryIcon,
              size: 18,
            ),
          ),
          // Default button
          if (isDefault)
            OutlinedButton.icon(
              onPressed: () => _clearDefaultModel(),
              style: OutlinedButton.styleFrom(
                foregroundColor: palette.accent,
                side: BorderSide(color: palette.accent),
                padding: const EdgeInsets.symmetric(
                  horizontal: 10,
                  vertical: 6,
                ),
                minimumSize: Size.zero,
                tapTargetSize: MaterialTapTargetSize.shrinkWrap,
              ),
              icon: const Icon(LucideIcons.check, size: 15),
              label: Text(
                l10n.defaultModel,
                style: const TextStyle(fontSize: 12),
              ),
            )
          else
            TextButton.icon(
              onPressed: () => _setDefaultModel(model),
              style: TextButton.styleFrom(
                foregroundColor: palette.secondaryText,
                padding: const EdgeInsets.symmetric(
                  horizontal: 10,
                  vertical: 6,
                ),
                minimumSize: Size.zero,
                tapTargetSize: MaterialTapTargetSize.shrinkWrap,
              ),
              label: Text(
                l10n.setDefaultModel,
                style: const TextStyle(fontSize: 12),
              ),
            ),
          const SizedBox(width: 4),
          // Hide/Show Button
          IconButton(
            tooltip: isHidden ? l10n.showModel : l10n.hideModel,
            onPressed: () => _toggleHideModel(model),
            icon: Icon(
              isHidden ? LucideIcons.eyeOff : LucideIcons.eye,
              size: 18,
              color: isHidden ? palette.secondaryIcon : palette.text,
            ),
            splashRadius: 18,
          ),
        ],
      ),
    );
  }
}

bool _isApiKeyModel(String providerId) =>
    providerId == 'chatgpt_api' ||
    ApiCompatibleProviderKeyStore.providerIds.contains(providerId);
