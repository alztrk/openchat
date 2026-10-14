import 'dart:convert';
import 'dart:typed_data';

import 'package:openchat/features/chat/domain/chat_attachment.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/domain/temporary_conversation_id.dart';

enum TemporaryConversationSessionState { active, discarded }

/// Holds temporary chat content only in memory and exposes no persistence API.
final class TemporaryConversationSession {
  TemporaryConversationSession({required this.id}) : _messages = [] {
    TemporaryConversationId.parse(id);
  }

  static const int maximumTranscriptBytes = 16 * 1024 * 1024;
  static const int maximumMessages = 100;

  final String id;
  final List<ChatMessage> _messages;
  TemporaryConversationSessionState _state =
      TemporaryConversationSessionState.active;

  TemporaryConversationSessionState get state => _state;

  List<ChatMessage> get messages =>
      List<ChatMessage>.unmodifiable(_messages.map(_copyMessage));

  bool get isEmpty => _messages.isEmpty;

  List<ChatMessage> snapshotForSave() {
    _requireActive();
    return messages;
  }

  void addUserMessage(ChatMessage message) {
    _requireActive();
    if (message.role != ChatMessageRole.user ||
        message.status != ChatMessageStatus.completed) {
      throw ArgumentError.value(
        message,
        'message',
        'A completed user message is required.',
      );
    }
    _addMessage(message);
  }

  void updateAssistantMessage(ChatMessage message) {
    _requireActive();
    if (message.role != ChatMessageRole.assistant) {
      throw ArgumentError.value(
        message,
        'message',
        'An assistant message is required.',
      );
    }
    final index = _messages.indexWhere((current) => current.id == message.id);
    if (index == -1) {
      _addMessage(message);
      return;
    }
    if (_messages[index].role != ChatMessageRole.assistant) {
      throw ArgumentError.value(
        message.id,
        'message.id',
        'Message roles cannot change.',
      );
    }
    final previousMessage = _messages[index];
    _messages[index] = _copyMessage(message);
    try {
      _validateTranscriptSize();
    } on Object {
      _messages[index] = previousMessage;
      rethrow;
    }
  }

  void discard() {
    _messages.clear();
    _state = TemporaryConversationSessionState.discarded;
  }

  void _addMessage(ChatMessage message) {
    if (message.id.trim().isEmpty ||
        _messages.any((current) => current.id == message.id)) {
      throw ArgumentError.value(
        message.id,
        'message.id',
        'Message IDs must be unique.',
      );
    }
    if (_messages.length >= maximumMessages) {
      throw ArgumentError.value(
        _messages.length,
        'messages',
        'The temporary transcript exceeds its message limit.',
      );
    }
    if ((_messages.isEmpty && message.role != ChatMessageRole.user) ||
        (_messages.isNotEmpty && _messages.last.role == message.role)) {
      throw ArgumentError.value(
        message.role,
        'message.role',
        'Temporary transcript messages must alternate between user and assistant.',
      );
    }
    _messages.add(_copyMessage(message));
    try {
      _validateTranscriptSize();
    } on Object {
      _messages.removeLast();
      rethrow;
    }
  }

  void _validateTranscriptSize() {
    final totalBytes = _messages.fold<int>(0, (total, message) {
      final attachmentBytes = message.attachments.fold<int>(0, (
        size,
        attachment,
      ) {
        final bytes = attachment.bytes;
        if (bytes == null || bytes.length != attachment.sizeBytes) {
          throw ArgumentError.value(
            attachment.id,
            'attachment',
            'Temporary attachments must keep their bytes in memory.',
          );
        }
        return size + bytes.length;
      });
      return total + utf8.encode(message.content).length + attachmentBytes;
    });
    if (totalBytes > maximumTranscriptBytes) {
      throw ArgumentError.value(
        totalBytes,
        'messages',
        'The temporary transcript exceeds its in-memory size limit.',
      );
    }
  }

  ChatMessage _copyMessage(ChatMessage message) => ChatMessage(
    id: message.id,
    role: message.role,
    content: message.content,
    attachments: List<ChatAttachment>.unmodifiable(
      message.attachments.map(
        (attachment) => ChatAttachment(
          id: attachment.id,
          name: attachment.name,
          mimeType: attachment.mimeType,
          sizeBytes: attachment.sizeBytes,
          kind: attachment.kind,
          bytes: switch (attachment.bytes) {
            final bytes? => Uint8List.fromList(bytes),
            null => null,
          },
          localPath: attachment.localPath,
          isAvailable: attachment.isAvailable,
        ),
      ),
    ),
    createdAt: message.createdAt,
    outputTokens: message.outputTokens,
    tokensPerSecond: message.tokensPerSecond,
    elapsed: message.elapsed,
    providerId: message.providerId,
    modelId: message.modelId,
    citationSources: List<ChatCitationSource>.unmodifiable(
      message.citationSources,
    ),
    personalMemoryFactIds: List<String>.unmodifiable(
      message.personalMemoryFactIds,
    ),
    reasoningSummaries: List<ChatReasoningSummary>.unmodifiable(
      message.reasoningSummaries,
    ),
    toolActivities: List<ChatToolActivity>.unmodifiable(message.toolActivities),
    status: message.status,
    failureCode: message.failureCode,
  );

  void _requireActive() {
    if (_state != TemporaryConversationSessionState.active) {
      throw StateError('The temporary conversation has been discarded.');
    }
  }
}
