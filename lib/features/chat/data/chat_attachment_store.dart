import 'dart:convert';
import 'dart:io';

import 'package:openchat/features/chat/domain/chat_attachment.dart';

class ChatAttachmentStore {
  const ChatAttachmentStore(this.storageRoot);

  final String storageRoot;

  static const maximumFileCount = ChatAttachmentTypes.maximumFileCount;
  static const maximumImageCount = ChatAttachmentTypes.maximumImageCount;
  static const maximumImageBytes = ChatAttachmentTypes.maximumImageBytes;
  static const maximumTextBytes = ChatAttachmentTypes.maximumTextBytes;
  static const maximumTotalBytes = ChatAttachmentTypes.maximumTotalBytes;

  Future<void> saveMessageAttachments({
    required String conversationId,
    required String messageId,
    required List<ChatAttachment> attachments,
  }) async {
    if (attachments.isEmpty) return;
    _validateIdentifier(conversationId, 'conversationId');
    _validateIdentifier(messageId, 'messageId');
    if (attachments.length > maximumFileCount ||
        attachments.where((attachment) => attachment.isImage).length >
            maximumImageCount ||
        attachments.fold<int>(
              0,
              (total, attachment) => total + attachment.sizeBytes,
            ) >
            maximumTotalBytes) {
      throw const ChatAttachmentStorageException(
        'The selected attachments exceed the supported count or total size.',
      );
    }
    for (final attachment in attachments) {
      _validateIdentifier(attachment.id, 'attachmentId');
      final bytes = attachment.bytes;
      final maxBytes = attachment.isImage
          ? maximumImageBytes
          : maximumTextBytes;
      final extensionSeparator = attachment.name.lastIndexOf('.');
      final extension = extensionSeparator < 0
          ? ''
          : attachment.name.substring(extensionSeparator + 1).toLowerCase();
      if (bytes == null ||
          !attachment.isAvailable ||
          bytes.length != attachment.sizeBytes ||
          bytes.isEmpty ||
          bytes.length > maxBytes ||
          ChatAttachmentTypes.kindForExtension(extension) != attachment.kind ||
          ChatAttachmentTypes.mimeTypeForExtension(extension) !=
              attachment.mimeType) {
        throw const ChatAttachmentStorageException(
          'A selected attachment was no longer available or exceeded its size limit.',
        );
      }
      if (attachment.kind == ChatAttachmentKind.text) {
        try {
          const Utf8Decoder(allowMalformed: false).convert(bytes);
        } on FormatException catch (error) {
          throw ChatAttachmentStorageException(
            'A text attachment is not valid UTF-8.',
            error,
          );
        }
      } else if (!ChatAttachmentTypes.hasValidImageSignature(
        attachment.mimeType,
        bytes,
      )) {
        throw const ChatAttachmentStorageException(
          'The selected image data did not match its file type.',
        );
      }
    }

    final parent = Directory(_conversationPath(conversationId));
    final destination = Directory(_messagePath(conversationId, messageId));
    final staging = Directory(
      '${parent.path}${Platform.pathSeparator}.stage-$messageId-${DateTime.now().microsecondsSinceEpoch}',
    );
    try {
      await staging.create(recursive: true);
      for (final attachment in attachments) {
        final file = File(_attachmentPath(staging.path, attachment.id));
        final bytes = attachment.bytes;
        if (bytes == null) {
          throw const ChatAttachmentStorageException(
            'A selected attachment was no longer available.',
          );
        }
        await file.writeAsBytes(bytes, flush: true);
      }
      if (await destination.exists()) {
        await destination.delete(recursive: true);
      }
      await staging.rename(destination.path);
    } on Object catch (error, stackTrace) {
      try {
        if (await staging.exists()) await staging.delete(recursive: true);
      } on Object catch (cleanupError, cleanupStackTrace) {
        Error.throwWithStackTrace(
          ChatAttachmentStorageException(
            'Selected attachments could not be saved or temporary files cleaned up.',
            cleanupError,
          ),
          cleanupStackTrace,
        );
      }
      Error.throwWithStackTrace(
        ChatAttachmentStorageException(
          'The selected attachments could not be saved.',
          error,
        ),
        stackTrace,
      );
    }
  }

  Future<List<ChatAttachment>> readMessageAttachments({
    required String conversationId,
    required String messageId,
    required List<ChatAttachment> expectedAttachments,
  }) async {
    if (expectedAttachments.isEmpty) return const <ChatAttachment>[];
    if (!_isValidIdentifier(conversationId) || !_isValidIdentifier(messageId)) {
      throw const ChatAttachmentStorageException(
        'Saved attachment identifiers were invalid.',
      );
    }
    final directory = Directory(_messagePath(conversationId, messageId));
    final attachments = <ChatAttachment>[];
    for (final attachment in expectedAttachments) {
      if (!_isValidIdentifier(attachment.id)) {
        throw const ChatAttachmentStorageException(
          'A saved attachment identifier was invalid.',
        );
      }
      final file = File(_attachmentPath(directory.path, attachment.id));
      final exists = await file.exists();
      final sizeMatches = exists && await file.length() == attachment.sizeBytes;
      attachments.add(
        ChatAttachment(
          id: attachment.id,
          name: attachment.name,
          mimeType: attachment.mimeType,
          sizeBytes: attachment.sizeBytes,
          kind: attachment.kind,
          localPath: sizeMatches ? file.path : null,
          isAvailable: sizeMatches,
        ),
      );
    }
    return List<ChatAttachment>.unmodifiable(attachments);
  }

  Future<void> deleteMessageAttachments({
    required String conversationId,
    required String messageId,
  }) async {
    if (!_isValidIdentifier(conversationId) || !_isValidIdentifier(messageId)) {
      return;
    }
    final directory = Directory(_messagePath(conversationId, messageId));
    if (await directory.exists()) await directory.delete(recursive: true);
  }

  Future<void> deleteConversationAttachments(String conversationId) async {
    if (!_isValidIdentifier(conversationId)) return;
    final directory = Directory(_conversationPath(conversationId));
    if (await directory.exists()) await directory.delete(recursive: true);
  }

  Future<void> deleteAllAttachments() async {
    final directory = Directory(_attachmentsRoot);
    if (await directory.exists()) await directory.delete(recursive: true);
  }

  String get _attachmentsRoot =>
      '$storageRoot${Platform.pathSeparator}attachments';

  String _conversationPath(String conversationId) =>
      '$_attachmentsRoot${Platform.pathSeparator}$conversationId';

  String _messagePath(String conversationId, String messageId) =>
      '${_conversationPath(conversationId)}${Platform.pathSeparator}$messageId';

  String _attachmentPath(String directory, String attachmentId) =>
      '$directory${Platform.pathSeparator}$attachmentId.data';

  static void _validateIdentifier(String value, String name) {
    if (!_isValidIdentifier(value)) {
      throw ArgumentError.value(value, name, 'Invalid storage identifier.');
    }
  }

  static bool _isValidIdentifier(String value) =>
      value.isNotEmpty &&
      value.length <= 128 &&
      value.codeUnits.every(
        (unit) =>
            (unit >= 48 && unit <= 57) ||
            (unit >= 65 && unit <= 90) ||
            (unit >= 97 && unit <= 122) ||
            unit == 45 ||
            unit == 95,
      );
}

class ChatAttachmentStorageException implements Exception {
  const ChatAttachmentStorageException(this.message, [this.cause]);

  final String message;
  final Object? cause;

  @override
  String toString() => message;
}
