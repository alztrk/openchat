import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/features/chat/data/chat_stream_message_persister.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';

ChatMessage snapshot(String content) => ChatMessage(
  id: 'assistant-1',
  role: ChatMessageRole.assistant,
  content: content,
  status: ChatMessageStatus.streaming,
);

void main() {
  test('coalesces a burst and flushes the latest snapshot', () async {
    final saved = <ChatMessage>[];
    final persister = ChatStreamMessagePersister(
      interval: const Duration(hours: 1),
      write: (message) async => saved.add(message),
    );

    for (var index = 0; index < 100; index++) {
      persister.add(snapshot('chunk $index'));
    }
    await persister.flush();

    expect(saved, hasLength(1));
    expect(saved.single.content, 'chunk 99');
  });

  test('flushes snapshots queued while an earlier write is active', () async {
    final firstWriteStarted = Completer<void>();
    final releaseFirstWrite = Completer<void>();
    final saved = <ChatMessage>[];
    final persister = ChatStreamMessagePersister(
      interval: const Duration(hours: 1),
      write: (message) async {
        saved.add(message);
        if (message.content == 'first') {
          firstWriteStarted.complete();
          await releaseFirstWrite.future;
        }
      },
    );

    persister.add(snapshot('first'));
    final flushing = persister.flush();
    await firstWriteStarted.future;
    persister.add(snapshot('second'));
    releaseFirstWrite.complete();
    await flushing;

    expect(saved.map((message) => message.content), <String>[
      'first',
      'second',
    ]);
  });

  test('surfaces persistence failures during the final flush', () async {
    final persister = ChatStreamMessagePersister(
      write: (_) => Future<void>.error(StateError('database unavailable')),
    );
    persister.add(snapshot('content'));

    await expectLater(persister.flush(), throwsA(isA<StateError>()));
  });

  test(
    'does not throw from later stream callbacks after a write fails',
    () async {
      final persister = ChatStreamMessagePersister(
        interval: const Duration(hours: 1),
        write: (_) => Future<void>.error(StateError('database unavailable')),
      );
      persister.add(snapshot('first'));
      await Future<void>.delayed(Duration.zero);

      expect(() => persister.add(snapshot('second')), returnsNormally);
      await expectLater(persister.flush(), throwsA(isA<StateError>()));
    },
  );
}
