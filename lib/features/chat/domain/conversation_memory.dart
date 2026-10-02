class ConversationMemoryState {
  const ConversationMemoryState({
    required this.compactionKind,
    required this.compactionProviderId,
    required this.compactionModelId,
    required this.compactionConnectionId,
    required this.compactionWorkspaceId,
    required this.compactedThroughMessageId,
    required this.summary,
    required this.lastPrompt,
  });

  final String? compactionKind;
  final String? compactionProviderId;
  final String? compactionModelId;
  final String? compactionConnectionId;
  final String? compactionWorkspaceId;
  final String? compactedThroughMessageId;
  final String? summary;
  final ConversationMemoryPrompt? lastPrompt;

  bool get hasContext => compactionKind != null;

  factory ConversationMemoryState.fromServiceResponse(
    Map<String, Object?> response,
  ) {
    final kind = switch (response['compactionKind']) {
      null => null,
      String value when value == 'summary' || value == 'responses_checkpoint' =>
        value,
      _ => throw const FormatException('Invalid compaction kind.'),
    };
    final summary = switch (response['summary']) {
      null => null,
      String value => value,
      _ => throw const FormatException('Invalid compaction summary.'),
    };
    final compactionProviderId = _optionalString(
      response['compactionProviderId'],
      'Invalid compaction provider.',
    );
    final compactionModelId = _optionalString(
      response['compactionModelId'],
      'Invalid compaction model.',
    );
    final compactionConnectionId = _optionalString(
      response['compactionConnectionId'],
      'Invalid compaction connection.',
    );
    final compactionWorkspaceId = _optionalString(
      response['compactionWorkspaceId'],
      'Invalid compaction workspace.',
    );
    final compactedThroughMessageId =
        switch (response['compactedThroughMessageId']) {
          null => null,
          String value when value.isNotEmpty => value,
          _ => throw const FormatException('Invalid compaction boundary.'),
        };
    final lastPrompt = _optionalObjectMap(response['lastPrompt']);
    if ((kind == 'summary') != (summary != null)) {
      throw const FormatException('The conversation summary is inconsistent.');
    }
    return ConversationMemoryState(
      compactionKind: kind,
      compactionProviderId: compactionProviderId,
      compactionModelId: compactionModelId,
      compactionConnectionId: compactionConnectionId,
      compactionWorkspaceId: compactionWorkspaceId,
      compactedThroughMessageId: compactedThroughMessageId,
      summary: summary,
      lastPrompt: lastPrompt == null
          ? null
          : ConversationMemoryPrompt.fromServiceResponse(lastPrompt),
    );
  }
}

class ConversationMemoryPrompt {
  const ConversationMemoryPrompt({
    required this.inputTokens,
    required this.messageId,
    required this.providerId,
    required this.modelId,
    required this.connectionId,
    required this.workspaceId,
  });

  final int inputTokens;
  final String? messageId;
  final String providerId;
  final String modelId;
  final String? connectionId;
  final String? workspaceId;

  factory ConversationMemoryPrompt.fromServiceResponse(
    Map<String, Object?> response,
  ) {
    final inputTokens = switch (response['inputTokens']) {
      int value when value >= 0 => value,
      _ => throw const FormatException('Invalid input token count.'),
    };
    final messageId = switch (response['messageId']) {
      null => null,
      String value when value.isNotEmpty => value,
      _ => throw const FormatException('Invalid last prompt message ID.'),
    };
    return ConversationMemoryPrompt(
      inputTokens: inputTokens,
      messageId: messageId,
      providerId: _requiredString(response['providerId']),
      modelId: _requiredString(response['modelId']),
      connectionId: _optionalString(
        response['connectionId'],
        'Invalid prompt connection.',
      ),
      workspaceId: _optionalString(
        response['workspaceId'],
        'Invalid prompt workspace.',
      ),
    );
  }
}

class ArchivedMemoryResult {
  const ArchivedMemoryResult({
    required this.messageId,
    required this.role,
    required this.content,
    required this.createdAtUnixMs,
  });

  final String messageId;
  final String role;
  final String content;
  final int createdAtUnixMs;

  factory ArchivedMemoryResult.fromServiceResponse(Object? rawResult) {
    final result = _requiredObjectMap(rawResult);
    final messageId = _requiredString(result['messageId']);
    final role = _requiredString(result['role']);
    final content = _requiredString(result['content']);
    final createdAtUnixMs = switch (result['createdAtUnixMs']) {
      int value when value >= 0 && value <= 8640000000000000 => value,
      _ => throw const FormatException('Invalid archived message date.'),
    };
    if (role != 'user' && role != 'assistant') {
      throw const FormatException('Invalid archived message role.');
    }
    return ArchivedMemoryResult(
      messageId: messageId,
      role: role,
      content: content,
      createdAtUnixMs: createdAtUnixMs,
    );
  }
}

enum SemanticPreparationPhase { downloading, indexing, ready }

class SemanticPreparationProgress {
  const SemanticPreparationProgress({
    required this.phase,
    required this.downloadedBytes,
    required this.totalBytes,
  });

  final SemanticPreparationPhase phase;
  final int downloadedBytes;
  final int totalBytes;

  double get fraction => downloadedBytes / totalBytes;

  int get percentage => (fraction * 100).round();

  factory SemanticPreparationProgress.fromServiceResponse(
    Map<String, Object?> response,
  ) {
    final phase = switch (response['phase']) {
      'downloading' => SemanticPreparationPhase.downloading,
      'indexing' => SemanticPreparationPhase.indexing,
      'ready' => SemanticPreparationPhase.ready,
      _ => throw const FormatException('Invalid semantic preparation phase.'),
    };
    final downloadedBytes = switch (response['downloadedBytes']) {
      int value when value >= 0 => value,
      _ => throw const FormatException('Invalid semantic download progress.'),
    };
    final totalBytes = switch (response['totalBytes']) {
      int value when value > 0 => value,
      _ => throw const FormatException('Invalid semantic download size.'),
    };
    if (downloadedBytes > totalBytes ||
        (phase == SemanticPreparationPhase.ready &&
            downloadedBytes != totalBytes)) {
      throw const FormatException('Inconsistent semantic download progress.');
    }
    return SemanticPreparationProgress(
      phase: phase,
      downloadedBytes: downloadedBytes,
      totalBytes: totalBytes,
    );
  }
}

Map<String, Object?>? _optionalObjectMap(Object? value) {
  if (value == null) return null;
  return _requiredObjectMap(value);
}

Map<String, Object?> _requiredObjectMap(Object? value) {
  if (value is! Map) {
    throw const FormatException('Invalid conversation memory data.');
  }
  final result = <String, Object?>{};
  for (final entry in value.entries) {
    if (entry.key is! String) {
      throw const FormatException('Invalid conversation memory field.');
    }
    result[entry.key as String] = entry.value;
  }
  return result;
}

String _requiredString(Object? value) => switch (value) {
  String result when result.isNotEmpty => result,
  _ => throw const FormatException('Invalid conversation memory text.'),
};

String? _optionalString(Object? value, String error) => switch (value) {
  null => null,
  String result when result.isNotEmpty => result,
  _ => throw FormatException(error),
};
