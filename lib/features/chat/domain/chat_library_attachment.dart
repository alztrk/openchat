import 'package:openchat/features/chat/domain/chat_attachment.dart';

class ChatLibraryAttachment {
  const ChatLibraryAttachment({
    required this.conversationId,
    required this.conversationTitle,
    required this.messageId,
    required this.attachment,
    required this.createdAt,
  });

  final String conversationId;
  final String conversationTitle;
  final String messageId;
  final ChatAttachment attachment;
  final DateTime createdAt;
}

class ChatLibraryAttachmentPage {
  const ChatLibraryAttachmentPage({
    required this.attachments,
    required this.nextMessageOffset,
  });

  final List<ChatLibraryAttachment> attachments;
  final int? nextMessageOffset;

  bool get hasMore => nextMessageOffset != null;
}
