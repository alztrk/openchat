enum ChatFileChangeState { active, reverted, conflict }

enum ChatFileChangeKind { added, modified, deleted }

class ChatFileChange {
  const ChatFileChange({
    required this.id,
    required this.path,
    required this.kind,
    required this.status,
    required this.isBinary,
    required this.diffAvailable,
    required this.canRevert,
    this.addedLines,
    this.removedLines,
  });

  final String id;
  final String path;
  final ChatFileChangeKind kind;
  final ChatFileChangeState status;
  final int? addedLines;
  final int? removedLines;
  final bool isBinary;
  final bool diffAvailable;
  final bool canRevert;

  Map<String, Object?> toJson() => <String, Object?>{
    'id': id,
    'path': path,
    'kind': switch (kind) {
      ChatFileChangeKind.added => 'added',
      ChatFileChangeKind.modified => 'modified',
      ChatFileChangeKind.deleted => 'deleted',
    },
    'status': switch (status) {
      ChatFileChangeState.active => 'active',
      ChatFileChangeState.reverted => 'reverted',
      ChatFileChangeState.conflict => 'conflict',
    },
    'addedLines': addedLines,
    'removedLines': removedLines,
    'isBinary': isBinary,
    'diffAvailable': diffAvailable,
    'canRevert': canRevert,
  };

  static ChatFileChange fromJson(Object? value) {
    final map = _stringMap(value);
    final id = map?['id'];
    final path = map?['path'];
    final kind = map?['kind'];
    final status = map?['status'];
    final addedLines = map?['addedLines'];
    final removedLines = map?['removedLines'];
    final isBinary = map?['isBinary'];
    final diffAvailable = map?['diffAvailable'];
    final canRevert = map?['canRevert'];
    if (id is! String ||
        id.isEmpty ||
        path is! String ||
        path.isEmpty ||
        kind is! String ||
        status is! String ||
        (addedLines != null && addedLines is! int) ||
        (removedLines != null && removedLines is! int) ||
        isBinary is! bool ||
        diffAvailable is! bool ||
        canRevert is! bool) {
      throw const FormatException('A conversation file change was invalid.');
    }

    return ChatFileChange(
      id: id,
      path: path,
      kind: switch (kind) {
        'added' => ChatFileChangeKind.added,
        'modified' => ChatFileChangeKind.modified,
        'deleted' => ChatFileChangeKind.deleted,
        _ => throw const FormatException('A file change kind was invalid.'),
      },
      status: switch (status) {
        'active' => ChatFileChangeState.active,
        'reverted' => ChatFileChangeState.reverted,
        'conflict' => ChatFileChangeState.conflict,
        _ => throw const FormatException('A file change status was invalid.'),
      },
      addedLines: addedLines is int ? addedLines : null,
      removedLines: removedLines is int ? removedLines : null,
      isBinary: isBinary,
      diffAvailable: diffAvailable,
      canRevert: canRevert,
    );
  }

  static List<ChatFileChange> listFromJson(Object? value) {
    final map = _stringMap(value);
    final changes = map?['changes'];
    if (changes is! List) {
      throw const FormatException(
        'The conversation file change list was invalid.',
      );
    }
    return changes.map(ChatFileChange.fromJson).toList(growable: false);
  }

  static List<ChatFileChange> listFromActivityJson(Object? value) {
    if (value == null) return const <ChatFileChange>[];
    if (value is! List) {
      throw const FormatException('The tool file change list was invalid.');
    }
    return value.map(ChatFileChange.fromJson).toList(growable: false);
  }
}

class ChatFileChangeDiff {
  const ChatFileChangeDiff({
    required this.path,
    required this.isBinary,
    required this.available,
    required this.diff,
    required this.isTruncated,
  });

  final String path;
  final bool isBinary;
  final bool available;
  final String diff;
  final bool isTruncated;

  static ChatFileChangeDiff fromJson(Object? value) {
    final map = _stringMap(value);
    final path = map?['path'];
    final isBinary = map?['isBinary'];
    final available = map?['available'];
    final diff = map?['diff'];
    final truncated = map?['truncated'];
    if (path is! String ||
        path.isEmpty ||
        isBinary is! bool ||
        available is! bool ||
        diff is! String ||
        (truncated != null && truncated is! bool)) {
      throw const FormatException('A conversation file diff was invalid.');
    }
    return ChatFileChangeDiff(
      path: path,
      isBinary: isBinary,
      available: available,
      diff: diff,
      isTruncated: truncated == true,
    );
  }
}

Map<String, Object?>? _stringMap(Object? value) {
  if (value is! Map) return null;
  final map = <String, Object?>{};
  for (final entry in value.entries) {
    final key = entry.key;
    if (key is! String) return null;
    map[key] = entry.value;
  }
  return map;
}
