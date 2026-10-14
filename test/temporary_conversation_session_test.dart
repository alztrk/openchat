import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/features/chat/domain/chat_attachment.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/domain/temporary_conversation_session.dart';

void main() {
  group('TemporaryConversationSession', () {
    late TemporaryConversationSession session;

    setUp(() {
      session = TemporaryConversationSession(
        id: 'tmp_0123456789abcdef0123456789abcdef',
      );
    });

    test('keeps messages in an immutable in-memory snapshot', () {
      session.addUserMessage(_message('user-1', ChatMessageRole.user, 'Hello'));
      session.updateAssistantMessage(
        _message('assistant-1', ChatMessageRole.assistant, 'Hi'),
      );

      final snapshot = session.snapshotForSave();
      expect(snapshot.map((message) => message.content), ['Hello', 'Hi']);
      expect(() => snapshot.clear(), throwsUnsupportedError);
    });

    test('requires an exact temporary ID', () {
      expect(
        () => TemporaryConversationSession(id: 'durable-id'),
        throwsFormatException,
      );
    });

    test('rejects duplicate IDs and role changes', () {
      session.addUserMessage(_message('same', ChatMessageRole.user, 'Hello'));
      expect(
        () => session.addUserMessage(
          _message('same', ChatMessageRole.user, 'Again'),
        ),
        throwsArgumentError,
      );
      expect(
        () => session.updateAssistantMessage(
          _message('same', ChatMessageRole.assistant, 'Changed role'),
        ),
        throwsArgumentError,
      );
    });

    test('requires alternating user and assistant turns', () {
      session.addUserMessage(_message('user-1', ChatMessageRole.user, 'Hello'));

      expect(
        () => session.addUserMessage(
          _message('user-2', ChatMessageRole.user, 'Another user turn'),
        ),
        throwsArgumentError,
      );
    });

    test('caps transcript messages at the service limit', () {
      for (var turn = 0; turn < 50; turn++) {
        session.addUserMessage(
          _message('user-$turn', ChatMessageRole.user, 'Question'),
        );
        session.updateAssistantMessage(
          _message('assistant-$turn', ChatMessageRole.assistant, 'Answer'),
        );
      }

      expect(
        () => session.addUserMessage(
          _message('user-over-limit', ChatMessageRole.user, 'One more'),
        ),
        throwsArgumentError,
      );
    });

    test('requires attachment bytes to remain in memory', () {
      final attachment = ChatAttachment(
        id: 'attachment-1',
        name: 'notes.txt',
        mimeType: 'text/plain',
        sizeBytes: 4,
        kind: ChatAttachmentKind.text,
        localPath: 'C:/private/notes.txt',
      );
      expect(
        () => session.addUserMessage(
          ChatMessage(
            id: 'user-1',
            role: ChatMessageRole.user,
            content: '',
            attachments: [attachment],
          ),
        ),
        throwsArgumentError,
      );
      expect(session.isEmpty, isTrue);
    });

    test('rejects a transcript larger than the memory cap without losing prior messages', () {
      session.addUserMessage(_message('user-1', ChatMessageRole.user, 'Hello'));
      expect(
        () => session.addUserMessage(
          _message(
            'user-2',
            ChatMessageRole.user,
            'x' * (TemporaryConversationSession.maximumTranscriptBytes + 1),
          ),
        ),
        throwsArgumentError,
      );
      expect(session.messages.single.id, 'user-1');
    });

    test('discard clears content and closes the session', () {
      session.addUserMessage(_message('user-1', ChatMessageRole.user, 'Hello'));

      session.discard();

      expect(session.state, TemporaryConversationSessionState.discarded);
      expect(session.messages, isEmpty);
      expect(session.snapshotForSave, throwsStateError);
    });

    test('owns byte-backed attachments and returns independent snapshots', () {
      final bytes = Uint8List.fromList([1, 2, 3]);
      final attachment = ChatAttachment(
        id: 'attachment-1',
        name: 'image.png',
        mimeType: 'image/png',
        sizeBytes: bytes.length,
        kind: ChatAttachmentKind.image,
        bytes: bytes,
      );

      session.addUserMessage(
        ChatMessage(
          id: 'user-1',
          role: ChatMessageRole.user,
          content: '',
          attachments: [attachment],
        ),
      );

      bytes[0] = 9;
      final storedBytes = session.messages.single.attachments.single.bytes;
      expect(storedBytes, isNotNull);
      expect(storedBytes?.toList(), [1, 2, 3]);
      if (storedBytes == null) fail('The session did not retain the bytes.');
      storedBytes[1] = 9;

      final nextSnapshotBytes = session
          .snapshotForSave()
          .single
          .attachments
          .single
          .bytes;
      expect(nextSnapshotBytes?.toList(), [1, 2, 3]);
    });
  });
}

ChatMessage _message(String id, ChatMessageRole role, String content) =>
    ChatMessage(id: id, role: role, content: content);
