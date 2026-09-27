class ChatConversation {
  const ChatConversation({
    required this.id,
    required this.title,
    required this.createdAt,
    required this.updatedAt,
    this.titleSource = ChatConversationTitleSource.automatic,
    this.connectionId,
    this.workspaceId,
    this.modelId,
    this.projectId,
    this.isPinned = false,
  });

  final String id;
  final String title;
  final ChatConversationTitleSource titleSource;
  final DateTime createdAt;
  final DateTime updatedAt;
  final String? connectionId;
  final String? workspaceId;
  final String? modelId;
  final String? projectId;
  final bool isPinned;
}

enum ChatConversationTitleSource { automatic, manual }
