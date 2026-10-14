import 'package:openchat/features/settings/data/api_compatible_provider_key_store.dart';

class DefaultModelPreference {
  const DefaultModelPreference({
    required this.providerId,
    required this.modelId,
    required this.displayName,
    this.reasoningEffort,
    this.connectionId,
    this.workspaceId,
    this.apiKeyConnectionId,
  });

  final String providerId;
  final String modelId;
  final String displayName;
  final String? reasoningEffort;
  final String? connectionId;
  final String? workspaceId;
  final String? apiKeyConnectionId;

  String get routeKey {
    final routeConnectionId = switch (providerId) {
      'chatgpt' => connectionId,
      'chatgpt_api' => apiKeyConnectionId ?? connectionId,
      'opencode' || 'llama_cpp' => null,
      final providerId
          when ApiCompatibleProviderKeyStore.isApiKeyProviderId(providerId) =>
        apiKeyConnectionId ?? connectionId ?? providerId,
      _ => null,
    };
    final routeWorkspaceId = providerId == 'chatgpt' ? workspaceId : null;
    return '$providerId:${routeConnectionId ?? ''}:${routeWorkspaceId ?? ''}:$modelId';
  }

  Map<String, Object?> toJson() => {
    'providerId': providerId,
    'modelId': modelId,
    'displayName': displayName,
    if (reasoningEffort != null) 'reasoningEffort': reasoningEffort,
    if (connectionId != null) 'connectionId': connectionId,
    if (workspaceId != null) 'workspaceId': workspaceId,
    if (apiKeyConnectionId != null) 'apiKeyConnectionId': apiKeyConnectionId,
  };

  factory DefaultModelPreference.fromJson(Map<String, Object?> json) {
    final providerId = _requiredString(json, 'providerId');
    final modelId = _requiredString(json, 'modelId');
    return DefaultModelPreference(
      providerId: providerId,
      modelId: modelId,
      displayName: _optionalString(json, 'displayName') ?? modelId,
      reasoningEffort: _optionalString(json, 'reasoningEffort'),
      connectionId: _optionalString(json, 'connectionId'),
      workspaceId: _optionalString(json, 'workspaceId'),
      apiKeyConnectionId: _optionalString(json, 'apiKeyConnectionId'),
    );
  }

  static String _requiredString(Map<String, Object?> json, String key) {
    final value = json[key];
    if (value is String && value.trim().isNotEmpty) return value;
    throw FormatException('The saved default model field "$key" is invalid.');
  }

  static String? _optionalString(Map<String, Object?> json, String key) {
    final value = json[key];
    if (value == null) return null;
    if (value is String) return value;
    throw FormatException('The saved default model field "$key" is invalid.');
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is DefaultModelPreference &&
          runtimeType == other.runtimeType &&
          providerId == other.providerId &&
          modelId == other.modelId &&
          displayName == other.displayName &&
          reasoningEffort == other.reasoningEffort &&
          connectionId == other.connectionId &&
          workspaceId == other.workspaceId &&
          apiKeyConnectionId == other.apiKeyConnectionId;

  @override
  int get hashCode => Object.hash(
    providerId,
    modelId,
    displayName,
    reasoningEffort,
    connectionId,
    workspaceId,
    apiKeyConnectionId,
  );
}
