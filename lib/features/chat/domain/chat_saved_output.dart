class ChatSavedOutput {
  const ChatSavedOutput({
    required this.conversationId,
    required this.messageId,
    required this.conversationTitle,
    required this.content,
    required this.savedAt,
    this.providerId,
    this.modelId,
  });

  final String conversationId;
  final String messageId;
  final String conversationTitle;
  final String content;
  final DateTime savedAt;
  final String? providerId;
  final String? modelId;
}
