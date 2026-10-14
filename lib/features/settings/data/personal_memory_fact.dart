import 'dart:math';

const maxPersonalMemoryFacts = 100;
const maxPersonalMemoryFactCharacters = 500;

final class PersonalMemoryFact {
  const PersonalMemoryFact({required this.id, required this.content});

  final String id;
  final String content;

  factory PersonalMemoryFact.create(String content) {
    final random = Random.secure();
    return PersonalMemoryFact(
      id: List<int>.generate(
        16,
        (_) => random.nextInt(256),
      ).map((byte) => byte.toRadixString(16).padLeft(2, '0')).join(),
      content: _validatedContent(content),
    );
  }

  factory PersonalMemoryFact.fromJson(Object? value) {
    if (value is! Map) {
      throw const FormatException('A saved personal memory is invalid.');
    }
    final id = value['id'];
    final content = value['content'];
    if (id is! String || !RegExp(r'^[a-f0-9]{32}$').hasMatch(id)) {
      throw const FormatException('A saved personal memory ID is invalid.');
    }
    if (content is! String || _validatedContentOrNull(content) == null) {
      throw const FormatException('A saved personal memory is invalid.');
    }
    return PersonalMemoryFact(id: id, content: content.trim());
  }

  Map<String, Object?> toJson() => <String, Object?>{
    'id': id,
    'content': content,
  };

  static String _validatedContent(String content) {
    final normalized = content.trim();
    if (normalized.isEmpty ||
        normalized.length > maxPersonalMemoryFactCharacters) {
      throw ArgumentError.value(
        content,
        'content',
        'A personal memory must contain 1 to $maxPersonalMemoryFactCharacters characters.',
      );
    }
    return normalized;
  }

  static String? _validatedContentOrNull(String content) {
    final normalized = content.trim();
    if (normalized.isEmpty ||
        normalized.length > maxPersonalMemoryFactCharacters ||
        normalized != content) {
      return null;
    }
    return normalized;
  }
}
