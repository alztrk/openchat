import 'dart:async';

import 'package:openchat/features/chat/domain/chat_message.dart';

typedef ChatMessageWriter = Future<void> Function(ChatMessage message);

/// Coalesces rapid streaming snapshots while preserving the final message.
class ChatStreamMessagePersister {
  ChatStreamMessagePersister({
    required this.write,
    this.interval = const Duration(milliseconds: 100),
  }) {
    if (interval <= Duration.zero) {
      throw ArgumentError.value(interval, 'interval', 'Must be positive.');
    }
  }

  final ChatMessageWriter write;
  final Duration interval;

  Timer? _timer;
  ChatMessage? _pending;
  Future<void>? _inFlight;
  AsyncError? _failure;

  void add(ChatMessage message) {
    if (_failure != null) return;

    _pending = message;
    _timer ??= Timer(interval, _writePending);
  }

  Future<void> flush() async {
    _timer?.cancel();
    _timer = null;

    while (_inFlight != null || _pending != null) {
      _timer?.cancel();
      _timer = null;
      final inFlight = _inFlight;
      if (inFlight != null) {
        await inFlight;
        if (_failure != null) _pending = null;
        continue;
      }
      await _persistPending();
      if (_failure != null) _pending = null;
    }

    if (_failure case final failure?) {
      Error.throwWithStackTrace(failure.error, failure.stackTrace);
    }
  }

  void _writePending() {
    _timer = null;
    unawaited(_persistPending());
  }

  Future<void> _persistPending() async {
    if (_inFlight != null) return;
    if (_failure != null) {
      _pending = null;
      return;
    }

    final message = _pending;
    if (message == null) return;
    _pending = null;
    final write = _save(message);
    _inFlight = write;
    await write;
    if (identical(_inFlight, write)) _inFlight = null;

    if (_pending != null && _failure == null) {
      _timer ??= Timer(interval, _writePending);
    }
  }

  Future<void> _save(ChatMessage message) async {
    try {
      await write(message);
    } on Object catch (error, stackTrace) {
      _failure ??= AsyncError(error, stackTrace);
    }
  }
}
