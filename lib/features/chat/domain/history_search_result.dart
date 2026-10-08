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

class HistorySearchFilters {
  const HistorySearchFilters({
    this.providerId,
    this.modelId,
    this.projectId,
    this.isArchived,
    this.tag,
  });

  final String? providerId;
  final String? modelId;
  final String? projectId;
  final bool? isArchived;
  final String? tag;

  bool get isEmpty =>
      providerId == null &&
      modelId == null &&
      projectId == null &&
      isArchived == null &&
      tag == null;
}

class HistorySearchFilterOptions {
  const HistorySearchFilterOptions({
    required this.providerIds,
    required this.modelIds,
    required this.tags,
  });

  final List<String> providerIds;
  final List<String> modelIds;
  final List<String> tags;

  factory HistorySearchFilterOptions.fromServiceResponse(Object? value) {
    if (value is! Map) {
      throw const FormatException('Invalid conversation history filters.');
    }
    final providerIds = value['providerIds'];
    final modelIds = value['modelIds'];
    final tags = value['tags'];
    if (providerIds is! List ||
        modelIds is! List ||
        tags is! List ||
        providerIds.length > 100 ||
        modelIds.length > 200 ||
        tags.length > 100 ||
        providerIds.any(
          (id) => id is! String || id.isEmpty || id.length > 256,
        ) ||
        modelIds.any((id) => id is! String || id.isEmpty || id.length > 256) ||
        tags.any((tag) => tag is! String || tag.isEmpty || tag.length > 32)) {
      throw const FormatException('Invalid conversation history filters.');
    }
    return HistorySearchFilterOptions(
      providerIds: List<String>.unmodifiable(providerIds.cast<String>()),
      modelIds: List<String>.unmodifiable(modelIds.cast<String>()),
      tags: List<String>.unmodifiable(tags.cast<String>()),
    );
  }
}

class SavedHistorySearch {
  const SavedHistorySearch({
    required this.name,
    required this.query,
    required this.filters,
    this.fromUnixMs,
    this.throughUnixMs,
  });

  final String name;
  final String query;
  final HistorySearchFilters filters;
  final int? fromUnixMs;
  final int? throughUnixMs;

  Map<String, Object?> toJson() => <String, Object?>{
    'name': name,
    'query': query,
    'providerId': filters.providerId,
    'modelId': filters.modelId,
    'projectId': filters.projectId,
    'isArchived': filters.isArchived,
    'tag': filters.tag,
    'fromUnixMs': fromUnixMs,
    'throughUnixMs': throughUnixMs,
  };

  factory SavedHistorySearch.fromJson(Object? value) {
    if (value is! Map) {
      throw const FormatException('A saved history search was invalid.');
    }
    final name = value['name'];
    final query = value['query'];
    final providerId = value['providerId'];
    final modelId = value['modelId'];
    final projectId = value['projectId'];
    final isArchived = value['isArchived'];
    final tag = value['tag'];
    final fromUnixMs = value['fromUnixMs'];
    final throughUnixMs = value['throughUnixMs'];
    if (name is! String ||
        name.trim().isEmpty ||
        name.runes.length > 60 ||
        query is! String ||
        query.trim().runes.length < 2 ||
        query.runes.length > 512 ||
        (providerId != null &&
            (providerId is! String || providerId.length > 256)) ||
        (modelId != null && (modelId is! String || modelId.length > 256)) ||
        (projectId != null &&
            (projectId is! String || projectId.length > 256)) ||
        (isArchived != null && isArchived is! bool) ||
        (tag != null && (tag is! String || tag.isEmpty || tag.length > 32)) ||
        (fromUnixMs != null && fromUnixMs is! int) ||
        (throughUnixMs != null && throughUnixMs is! int) ||
        (fromUnixMs is int &&
            (fromUnixMs < 0 || fromUnixMs > 8640000000000000)) ||
        (throughUnixMs is int &&
            (throughUnixMs < 0 || throughUnixMs > 8640000000000000)) ||
        (fromUnixMs is int &&
            throughUnixMs is int &&
            fromUnixMs >= throughUnixMs)) {
      throw const FormatException('A saved history search was invalid.');
    }
    return SavedHistorySearch(
      name: name.trim(),
      query: query.trim(),
      filters: HistorySearchFilters(
        providerId: providerId is String ? providerId : null,
        modelId: modelId is String ? modelId : null,
        projectId: projectId is String ? projectId : null,
        isArchived: isArchived is bool ? isArchived : null,
        tag: tag is String ? tag : null,
      ),
      fromUnixMs: fromUnixMs is int ? fromUnixMs : null,
      throughUnixMs: throughUnixMs is int ? throughUnixMs : null,
    );
  }
}
