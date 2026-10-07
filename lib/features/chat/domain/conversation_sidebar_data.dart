class ConversationSidebarConversation {
  const ConversationSidebarConversation({
    required this.id,
    required this.title,
    this.isPinned = false,
    this.isArchived = false,
  });

  final String id;
  final String title;
  final bool isPinned;
  final bool isArchived;
}

class ConversationSidebarProject {
  const ConversationSidebarProject({
    required this.id,
    required this.title,
    required this.conversations,
    this.hasMoreConversations = false,
  });

  final String id;
  final String title;
  final List<ConversationSidebarConversation> conversations;
  final bool hasMoreConversations;
}
