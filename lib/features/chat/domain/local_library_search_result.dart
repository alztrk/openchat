enum LocalLibrarySearchResultKind {
  project,
  savedResponse,
}

enum UnifiedLibrarySearchFilter {
  all,
  chats,
  projects,
  savedResponses,
}

class LocalLibrarySearchResult {
  const LocalLibrarySearchResult({
    required this.kind,
    required this.id,
    required this.title,
    required this.updatedAt,
    this.subtitle,
    this.conversationId,
    this.messageId,
  });

  final LocalLibrarySearchResultKind kind;
  final String id;
  final String title;
  final String? subtitle;
  final DateTime updatedAt;
  final String? conversationId;
  final String? messageId;
}
