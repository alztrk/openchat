import 'dart:convert';
import 'dart:typed_data';

enum ChatAttachmentKind { image, text }

class ChatAttachment {
  const ChatAttachment({
    required this.id,
    required this.name,
    required this.mimeType,
    required this.sizeBytes,
    required this.kind,
    this.bytes,
    this.localPath,
    this.isAvailable = true,
  });

  final String id;
  final String name;
  final String mimeType;
  final int sizeBytes;
  final ChatAttachmentKind kind;
  final Uint8List? bytes;
  final String? localPath;
  final bool isAvailable;

  bool get isImage => kind == ChatAttachmentKind.image;

  Map<String, Object?> toMetadataJson() => <String, Object?>{
    'id': id,
    'name': name,
    'mimeType': mimeType,
    'sizeBytes': sizeBytes,
    'kind': kind.name,
  };

  static ChatAttachment fromMetadataJson(Object? value) {
    if (value is! Map<String, Object?> ||
        value['id'] is! String ||
        value['name'] is! String ||
        value['mimeType'] is! String ||
        value['sizeBytes'] is! int ||
        value['kind'] is! String) {
      throw const FormatException('A saved attachment was invalid.');
    }
    final kind = switch (value['kind']) {
      'image' => ChatAttachmentKind.image,
      'text' => ChatAttachmentKind.text,
      _ => throw const FormatException('A saved attachment type was invalid.'),
    };
    final id = value['id'] as String;
    final name = value['name'] as String;
    final mimeType = value['mimeType'] as String;
    final sizeBytes = value['sizeBytes'] as int;
    final validMimeType = kind == ChatAttachmentKind.image
        ? const <String>{
            'image/png',
            'image/jpeg',
            'image/webp',
          }.contains(mimeType)
        : mimeType.startsWith('text/') ||
              const <String>{
                'application/json',
                'application/xml',
              }.contains(mimeType);
    if (!RegExp(r'^[A-Za-z0-9_-]{1,128}$').hasMatch(id) ||
        name.trim().isEmpty ||
        sizeBytes <= 0 ||
        sizeBytes >
            (kind == ChatAttachmentKind.image
                ? ChatAttachmentTypes.maximumImageBytes
                : ChatAttachmentTypes.maximumTextBytes) ||
        !validMimeType) {
      throw const FormatException('A saved attachment was invalid.');
    }
    return ChatAttachment(
      id: id,
      name: name,
      mimeType: mimeType,
      sizeBytes: sizeBytes,
      kind: kind,
    );
  }
}

abstract final class ChatMessageContentCodec {
  static const _prefix = '\u001eopenchat-attachments-v1:';

  static String encode(String content, List<ChatAttachment> attachments) {
    if (attachments.isEmpty) return content;
    return '$_prefix${jsonEncode(<String, Object?>{'content': content, 'attachments': attachments.map((attachment) => attachment.toMetadataJson()).toList(growable: false)})}';
  }

  static ({String content, List<ChatAttachment> attachments}) decode(
    String storedContent,
  ) {
    if (!storedContent.startsWith(_prefix)) {
      return (content: storedContent, attachments: const <ChatAttachment>[]);
    }
    try {
      final payload = jsonDecode(storedContent.substring(_prefix.length));
      if (payload is! Map<String, Object?> ||
          payload['content'] is! String ||
          payload['attachments'] is! List<Object?>) {
        throw const FormatException(
          'The saved message attachment data was invalid.',
        );
      }
      final attachments = (payload['attachments'] as List<Object?>)
          .map(ChatAttachment.fromMetadataJson)
          .toList(growable: false);
      final ids = attachments.map((attachment) => attachment.id).toSet();
      final totalBytes = attachments.fold<int>(
        0,
        (total, attachment) => total + attachment.sizeBytes,
      );
      if (attachments.isEmpty ||
          attachments.length > ChatAttachmentTypes.maximumFileCount ||
          ids.length != attachments.length ||
          attachments.where((attachment) => attachment.isImage).length >
              ChatAttachmentTypes.maximumImageCount ||
          totalBytes > ChatAttachmentTypes.maximumTotalBytes) {
        throw const FormatException(
          'The saved message attachment limits were exceeded.',
        );
      }
      return (content: payload['content'] as String, attachments: attachments);
    } on Object catch (error) {
      throw FormatException(
        'The saved message attachment data was invalid: $error',
      );
    }
  }
}

abstract final class ChatAttachmentTypes {
  static const maximumFileCount = 10;
  static const maximumImageCount = 3;
  static const maximumImageBytes = 10 * 1024 * 1024;
  static const maximumTextBytes = 1024 * 1024;
  static const maximumTotalBytes = 14 * 1024 * 1024;

  static const imageExtensions = <String>{'png', 'jpg', 'jpeg', 'webp'};

  static const textExtensions = <String>{
    'txt',
    'md',
    'csv',
    'json',
    'jsonl',
    'xml',
    'html',
    'css',
    'js',
    'jsx',
    'ts',
    'tsx',
    'dart',
    'rs',
    'py',
    'go',
    'java',
    'kt',
    'swift',
    'c',
    'h',
    'cpp',
    'hpp',
    'cs',
    'sh',
    'ps1',
    'yaml',
    'yml',
    'toml',
    'ini',
    'sql',
    'log',
  };

  static const allowedExtensions = <String>{
    ...imageExtensions,
    ...textExtensions,
  };

  static ChatAttachmentKind? kindForExtension(String extension) {
    final normalized = extension.toLowerCase();
    if (imageExtensions.contains(normalized)) return ChatAttachmentKind.image;
    if (textExtensions.contains(normalized)) return ChatAttachmentKind.text;
    return null;
  }

  static String? mimeTypeForExtension(String extension) {
    return switch (extension.toLowerCase()) {
      'png' => 'image/png',
      'jpg' || 'jpeg' => 'image/jpeg',
      'webp' => 'image/webp',
      'txt' || 'log' => 'text/plain',
      'md' => 'text/markdown',
      'csv' => 'text/csv',
      'json' || 'jsonl' => 'application/json',
      'xml' => 'application/xml',
      'html' => 'text/html',
      'css' => 'text/css',
      'js' || 'jsx' => 'text/javascript',
      'ts' || 'tsx' => 'text/typescript',
      'dart' ||
      'rs' ||
      'py' ||
      'go' ||
      'java' ||
      'kt' ||
      'swift' ||
      'c' ||
      'h' ||
      'cpp' ||
      'hpp' ||
      'cs' ||
      'sh' ||
      'ps1' ||
      'yaml' ||
      'yml' ||
      'toml' ||
      'ini' ||
      'sql' => 'text/plain',
      _ => null,
    };
  }

  static bool hasValidImageSignature(String mimeType, Uint8List bytes) {
    return switch (mimeType) {
      'image/png' =>
        bytes.length >= 8 &&
            bytes[0] == 0x89 &&
            bytes[1] == 0x50 &&
            bytes[2] == 0x4e &&
            bytes[3] == 0x47 &&
            bytes[4] == 0x0d &&
            bytes[5] == 0x0a &&
            bytes[6] == 0x1a &&
            bytes[7] == 0x0a,
      'image/jpeg' =>
        bytes.length >= 3 &&
            bytes[0] == 0xff &&
            bytes[1] == 0xd8 &&
            bytes[2] == 0xff,
      'image/webp' =>
        bytes.length >= 12 &&
            bytes[0] == 0x52 &&
            bytes[1] == 0x49 &&
            bytes[2] == 0x46 &&
            bytes[3] == 0x46 &&
            bytes[8] == 0x57 &&
            bytes[9] == 0x45 &&
            bytes[10] == 0x42 &&
            bytes[11] == 0x50,
      _ => false,
    };
  }
}
