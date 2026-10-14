import 'dart:math';

final class TemporaryConversationId {
  const TemporaryConversationId._(this.value);

  static final Random _random = Random.secure();

  final String value;

  static TemporaryConversationId create() {
    final suffix = List<int>.generate(
      16,
      (_) => _random.nextInt(256),
    ).map((byte) => byte.toRadixString(16).padLeft(2, '0')).join();
    return TemporaryConversationId._('tmp_$suffix');
  }

  static TemporaryConversationId parse(String value) {
    if (!RegExp(r'^tmp_[a-f0-9]{32}$').hasMatch(value)) {
      throw const FormatException('A temporary conversation ID was invalid.');
    }
    return TemporaryConversationId._(value);
  }
}

bool isTemporaryConversationId(String value) => value.startsWith('tmp_');

void requireDurableConversationId(String value) {
  if (isTemporaryConversationId(value)) {
    throw ArgumentError.value(
      value,
      'conversationId',
      'Temporary conversation IDs cannot be written to durable history.',
    );
  }
}
