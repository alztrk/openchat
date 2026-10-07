class ChatGptWorkspace {
  const ChatGptWorkspace({
    required this.id,
    required this.externalId,
    required this.isSelected,
    this.displayName,
    this.planType,
  });

  final String id;
  final String externalId;
  final String? displayName;
  final String? planType;
  final bool isSelected;

  factory ChatGptWorkspace.fromJson(Map<String, Object?> json) {
    return ChatGptWorkspace(
      id: _requiredString(json, 'id'),
      externalId: _requiredString(json, 'externalId'),
      displayName: _optionalString(json, 'displayName'),
      planType: _optionalString(json, 'planType'),
      isSelected: json['isSelected'] == true,
    );
  }
}

class ChatGptConnection {
  const ChatGptConnection({
    required this.id,
    required this.authStatus,
    required this.isSelected,
    required this.workspaces,
    this.email,
    this.displayName,
    this.planType,
  });

  final String id;
  final String? email;
  final String? displayName;
  final String? planType;
  final String authStatus;
  final bool isSelected;
  final List<ChatGptWorkspace> workspaces;

  factory ChatGptConnection.fromJson(Map<String, Object?> json) {
    final workspaces = json['workspaces'];
    if (workspaces is! List<Object?>) {
      throw const FormatException('The connection did not contain workspaces.');
    }
    return ChatGptConnection(
      id: _requiredString(json, 'id'),
      email: _optionalString(json, 'email'),
      displayName: _optionalString(json, 'displayName'),
      planType: _optionalString(json, 'planType'),
      authStatus: _requiredString(json, 'authStatus'),
      isSelected: json['isSelected'] == true,
      workspaces: workspaces
          .map((workspace) => ChatGptWorkspace.fromJson(_objectMap(workspace)))
          .toList(growable: false),
    );
  }
}

class ChatGptTitlePreference {
  const ChatGptTitlePreference({this.connectionId, this.workspaceId});

  final String? connectionId;
  final String? workspaceId;

  factory ChatGptTitlePreference.fromJson(Map<String, Object?> json) {
    final connectionId = _optionalString(json, 'connectionId');
    final workspaceId = _optionalString(json, 'workspaceId');
    if (workspaceId != null && connectionId == null) {
      throw const FormatException(
        'The title workspace did not have an account.',
      );
    }
    return ChatGptTitlePreference(
      connectionId: connectionId,
      workspaceId: workspaceId,
    );
  }
}

class ChatGptModel {
  const ChatGptModel({
    required this.id,
    required this.displayName,
    required this.isAvailable,
    required this.reasoningLevels,
    this.supportsReasoning = false,
    this.supportsImages = false,
    this.supportsFastMode = false,
    this.supportsTools,
    this.description,
    this.contextWindow,
    this.defaultReasoningLevel,
    this.providerId = 'chatgpt',
    this.connectionId,
    this.workspaceId,
    this.sourceLabel,
    this.groupId,
    this.unavailabilityReason,
  });

  final String id;
  final String displayName;
  final String? description;
  final int? contextWindow;
  final String? defaultReasoningLevel;
  final List<String> reasoningLevels;
  final bool supportsReasoning;
  final bool supportsImages;
  final bool supportsFastMode;
  final bool? supportsTools;
  final bool isAvailable;
  final String providerId;
  final String? connectionId;
  final String? workspaceId;
  final String? sourceLabel;
  final String? groupId;
  final String? unavailabilityReason;

  String get routeKey =>
      '$providerId:${connectionId ?? ''}:${workspaceId ?? ''}:$id';

  ChatGptModel withRoute({
    required String providerId,
    String? connectionId,
    String? workspaceId,
    String? sourceLabel,
    String? groupId,
  }) => ChatGptModel(
    id: id,
    displayName: displayName,
    description: description,
    contextWindow: contextWindow,
    defaultReasoningLevel: defaultReasoningLevel,
    reasoningLevels: reasoningLevels,
    supportsReasoning: supportsReasoning,
    supportsImages: supportsImages,
    supportsFastMode: supportsFastMode,
    isAvailable: isAvailable,
    providerId: providerId,
    connectionId: connectionId,
    workspaceId: workspaceId,
    sourceLabel: sourceLabel,
    groupId: groupId,
    unavailabilityReason: unavailabilityReason,
    supportsTools: providerId == 'chatgpt' ? true : supportsTools,
  );

  factory ChatGptModel.fromJson(Map<String, Object?> json) {
    final reasoningLevels = json['reasoningLevels'];
    if (reasoningLevels is! List<Object?> ||
        reasoningLevels.any((level) => level is! String)) {
      throw const FormatException('The model reasoning levels were invalid.');
    }
    final supportsReasoning = json['supportsReasoning'];
    if (supportsReasoning != null && supportsReasoning is! bool) {
      throw const FormatException(
        'The model reasoning capability was invalid.',
      );
    }
    final supportsImages = json['supportsImages'];
    if (supportsImages != null && supportsImages is! bool) {
      throw const FormatException('The model image capability was invalid.');
    }
    final supportsFastMode = json['supportsFastMode'];
    if (supportsFastMode != null && supportsFastMode is! bool) {
      throw const FormatException(
        'The model Fast mode capability was invalid.',
      );
    }
    final supportsTools = json['supportsTools'];
    if (supportsTools != null && supportsTools is! bool) {
      throw const FormatException('The model tool capability was invalid.');
    }
    final contextWindow = json['contextWindow'];
    if (contextWindow != null && contextWindow is! int) {
      throw const FormatException('The model context window was invalid.');
    }
    return ChatGptModel(
      id: _requiredString(json, 'id'),
      displayName: _requiredString(json, 'displayName'),
      description: _optionalString(json, 'description'),
      contextWindow: contextWindow as int?,
      defaultReasoningLevel: _optionalString(json, 'defaultReasoningLevel'),
      reasoningLevels: reasoningLevels.cast<String>(),
      supportsReasoning:
          supportsReasoning == true || reasoningLevels.isNotEmpty,
      supportsImages: supportsImages == true,
      supportsFastMode: supportsFastMode == true,
      supportsTools: supportsTools as bool?,
      isAvailable: json['isAvailable'] == true,
      groupId: _optionalString(json, 'groupId'),
      unavailabilityReason: _optionalString(json, 'reason'),
    );
  }
}

Map<String, Object?> _objectMap(Object? value) {
  if (value is! Map) {
    throw const FormatException('A ChatGPT response item was invalid.');
  }
  final result = <String, Object?>{};
  for (final entry in value.entries) {
    if (entry.key is! String) {
      throw const FormatException('A ChatGPT response key was invalid.');
    }
    result[entry.key as String] = entry.value;
  }
  return result;
}

String _requiredString(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value is! String || value.isEmpty) {
    throw FormatException('The ChatGPT response did not contain $key.');
  }
  return value;
}

String? _optionalString(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value == null) return null;
  if (value is! String) {
    throw FormatException('The ChatGPT response field $key was invalid.');
  }
  return value;
}
