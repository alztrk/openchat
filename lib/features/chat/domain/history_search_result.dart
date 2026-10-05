class HistorySearchResult {
  const HistorySearchResult({
    required this.conversationId,
    required this.conversationTitle,
    required this.messageId,
    required this.role,
    required this.excerpt,
    required this.createdAt,
  });

  final String conversationId;
  final String conversationTitle;
  final String messageId;
  final String role;
  final String excerpt;
  final DateTime createdAt;

  factory HistorySearchResult.fromServiceResponse(Object? rawResult) {
    if (rawResult is! Map) {
      throw const FormatException('Invalid conversation history result.');
    }
    final result = <String, Object?>{};
    for (final entry in rawResult.entries) {
      if (entry.key is! String) {
        throw const FormatException('Invalid conversation history field.');
      }
      result[entry.key as String] = entry.value;
    }

    final conversationId = result['conversationId'];
    final conversationTitle = result['conversationTitle'];
    final messageId = result['messageId'];
    final role = result['role'];
    final excerpt = result['excerpt'];
    final createdAtUnixMs = result['createdAtUnixMs'];
    if (conversationId is! String ||
        conversationId.isEmpty ||
        conversationTitle is! String ||
        conversationTitle.isEmpty ||
        messageId is! String ||
        messageId.isEmpty ||
        role is! String ||
        (role != 'user' && role != 'assistant') ||
        excerpt is! String ||
        createdAtUnixMs is! int ||
        createdAtUnixMs < 0 ||
        createdAtUnixMs > 8640000000000000) {
      throw const FormatException('Invalid conversation history result.');
    }
    return HistorySearchResult(
      conversationId: conversationId,
      conversationTitle: conversationTitle,
      messageId: messageId,
      role: role,
      excerpt: excerpt,
      createdAt: DateTime.fromMillisecondsSinceEpoch(
        createdAtUnixMs,
        isUtc: true,
      ),
    );
  }
}
