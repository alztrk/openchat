class ChatSavedOutput {
  const ChatSavedOutput({
    required this.conversationId,
    required this.messageId,
    required this.conversationTitle,
    required this.content,
    required this.savedAt,
    this.selectedVersion = 1,
    this.providerId,
    this.modelId,
  });

  final String conversationId;
  final String messageId;
  final String conversationTitle;
  final String content;
  final DateTime savedAt;
  final int selectedVersion;
  final String? providerId;
  final String? modelId;
}

class ChatSavedOutputVersion {
  const ChatSavedOutputVersion({
    required this.conversationId,
    required this.messageId,
    required this.versionNumber,
    required this.content,
    required this.createdAt,
  });

  final String conversationId;
  final String messageId;
  final int versionNumber;
  final String content;
  final DateTime createdAt;
}
