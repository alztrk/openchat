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
      'opencode' || 'llama_cpp' || 'vllm' || 'exllama' => null,
      'gemini' ||
      'groq' ||
      'cerebras' ||
      'openrouter' ||
      'mistral' => apiKeyConnectionId ?? connectionId ?? providerId,
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
    return DefaultModelPreference(
      providerId: json['providerId'] as String,
      modelId: json['modelId'] as String,
      displayName: json['displayName'] as String? ?? json['modelId'] as String,
      reasoningEffort: json['reasoningEffort'] as String?,
      connectionId: json['connectionId'] as String?,
      workspaceId: json['workspaceId'] as String?,
      apiKeyConnectionId: json['apiKeyConnectionId'] as String?,
    );
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
