class ChatConversation {
  const ChatConversation({
    required this.id,
    required this.title,
    required this.createdAt,
    required this.updatedAt,
    this.titleSource = ChatConversationTitleSource.automatic,
    this.connectionId,
    this.workspaceId,
    this.productWorkspaceId,
    this.apiKeyConnectionId,
    this.providerId,
    this.modelId,
    this.projectId,
    this.isPinned = false,
    this.isArchived = false,
    this.isBookmarked = false,
    this.tags = const <String>[],
  });

  final String id;
  final String title;
  final ChatConversationTitleSource titleSource;
  final DateTime createdAt;
  final DateTime updatedAt;
  final String? connectionId;
  final String? workspaceId;
  final String? productWorkspaceId;
  final String? apiKeyConnectionId;
  final String? providerId;
  final String? modelId;
  final String? projectId;
  final bool isPinned;
  final bool isArchived;
  final bool isBookmarked;
  final List<String> tags;
}

enum ChatConversationTitleSource { automatic, manual }
