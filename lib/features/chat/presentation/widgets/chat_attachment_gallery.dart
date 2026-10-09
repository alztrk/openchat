import 'dart:io';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/chat_attachment.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

class ChatAttachmentGallery extends StatefulWidget {
  const ChatAttachmentGallery({
    required this.attachments,
    required this.palette,
    required this.preferredImageWidth,
    required this.imageHeight,
    this.onRemoveAttachment,
    super.key,
  });

  static const _sliderThreshold = 4;
  static const _spacing = 8.0;

  final List<ChatAttachment> attachments;
  final OpenChatPalette palette;
  final double preferredImageWidth;
  final double imageHeight;
  final ValueChanged<String>? onRemoveAttachment;

  @override
  State<ChatAttachmentGallery> createState() => _ChatAttachmentGalleryState();
}

class _ChatAttachmentGalleryState extends State<ChatAttachmentGallery> {
  static const _thumbnailDecodeScale = 3.0;

  final ScrollController _scrollController = ScrollController();

  @override
  void dispose() {
    _scrollController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (widget.attachments.isEmpty) return const SizedBox.shrink();

    return LayoutBuilder(
      builder: (context, constraints) {
        final maxWidth = constraints.maxWidth;
        if (widget.attachments.length <=
            ChatAttachmentGallery._sliderThreshold) {
          final rowWidth = widget.attachments.length == 1
              ? maxWidth
              : math
                    .max(0.0, (maxWidth - ChatAttachmentGallery._spacing) / 2)
                    .toDouble();
          final tileWidth = math
              .min(widget.preferredImageWidth, rowWidth)
              .toDouble();
          return Wrap(
            spacing: ChatAttachmentGallery._spacing,
            runSpacing: ChatAttachmentGallery._spacing,
            children: widget.attachments
                .map(
                  (attachment) => SizedBox(
                    width: tileWidth,
                    child: _buildTile(
                      context,
                      attachment,
                      width: tileWidth,
                      imageHeight: widget.imageHeight,
                    ),
                  ),
                )
                .toList(growable: false),
          );
        }

        final tileWidth =
            ((maxWidth -
                        ChatAttachmentGallery._spacing *
                            (ChatAttachmentGallery._sliderThreshold - 1)) /
                    ChatAttachmentGallery._sliderThreshold)
                .clamp(64.0, widget.preferredImageWidth)
                .toDouble();
        final viewportWidth = math
            .min(
              maxWidth,
              tileWidth * ChatAttachmentGallery._sliderThreshold +
                  ChatAttachmentGallery._spacing *
                      (ChatAttachmentGallery._sliderThreshold - 1),
            )
            .toDouble();

        return SizedBox(
          width: viewportWidth,
          height: widget.imageHeight + 8,
          child: Scrollbar(
            controller: _scrollController,
            thumbVisibility: true,
            interactive: true,
            thickness: 4,
            radius: const Radius.circular(4),
            child: SingleChildScrollView(
              controller: _scrollController,
              scrollDirection: Axis.horizontal,
              child: Padding(
                padding: const EdgeInsets.only(bottom: 8),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: widget.attachments
                      .map(
                        (attachment) => Padding(
                          padding: const EdgeInsets.only(
                            right: ChatAttachmentGallery._spacing,
                          ),
                          child: SizedBox(
                            width: tileWidth,
                            height: widget.imageHeight,
                            child: _buildTile(
                              context,
                              attachment,
                              width: tileWidth,
                              imageHeight: widget.imageHeight,
                            ),
                          ),
                        ),
                      )
                      .toList(growable: false),
                ),
              ),
            ),
          ),
        );
      },
    );
  }

  Widget _buildTile(
    BuildContext context,
    ChatAttachment attachment, {
    required double width,
    required double imageHeight,
  }) {
    final imageProvider = _imageProvider(attachment);
    if (attachment.isImage && imageProvider != null) {
      return _imageTile(
        context,
        attachment,
        imageProvider,
        width: width,
        height: imageHeight,
      );
    }
    return Align(
      alignment: Alignment.topLeft,
      child: _fileTile(context, attachment, width: width),
    );
  }

  Widget _imageTile(
    BuildContext context,
    ChatAttachment attachment,
    ImageProvider<Object> imageProvider, {
    required double width,
    required double height,
  }) {
    final l10n = context.openchatL10n;
    final devicePixelRatio = MediaQuery.devicePixelRatioOf(context);
    final thumbnailImageProvider = ResizeImage(
      imageProvider,
      width: math.max(
        1,
        (width * devicePixelRatio * _thumbnailDecodeScale).ceil(),
      ),
      height: math.max(
        1,
        (height * devicePixelRatio * _thumbnailDecodeScale).ceil(),
      ),
      policy: ResizeImagePolicy.fit,
    );
    return SizedBox(
      width: width,
      height: height,
      child: Semantics(
        button: true,
        label: '${l10n.previewImage}: ${attachment.name}',
        child: Tooltip(
          message: l10n.previewImage,
          child: ClipRRect(
            borderRadius: BorderRadius.circular(9),
            child: Stack(
              fit: StackFit.expand,
              children: [
                Material(
                  color: widget.palette.surface,
                  child: InkWell(
                    onTap: () => _showImagePreview(
                      context,
                      imageProvider,
                      attachment.name,
                    ),
                    child: Image(
                      image: thumbnailImageProvider,
                      width: width,
                      height: height,
                      fit: BoxFit.cover,
                      errorBuilder: (context, error, stackTrace) =>
                          _unavailableTile(context),
                    ),
                  ),
                ),
                if (widget.onRemoveAttachment != null)
                  Positioned(
                    top: 5,
                    right: 5,
                    child: IconButton(
                      onPressed: () =>
                          widget.onRemoveAttachment?.call(attachment.id),
                      tooltip: l10n.removeAttachment,
                      constraints: const BoxConstraints.tightFor(
                        width: 30,
                        height: 30,
                      ),
                      padding: EdgeInsets.zero,
                      style: IconButton.styleFrom(
                        backgroundColor: Colors.black.withValues(alpha: 0.62),
                        foregroundColor: Colors.white,
                      ),
                      icon: const Icon(LucideIcons.x, size: 17),
                    ),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  Widget _fileTile(
    BuildContext context,
    ChatAttachment attachment, {
    required double width,
  }) {
    final l10n = context.openchatL10n;
    final sizeLabel = attachment.sizeBytes < 1024 * 1024
        ? '${(attachment.sizeBytes / 1024).ceil()} KB'
        : '${(attachment.sizeBytes / (1024 * 1024)).toStringAsFixed(1)} MB';
    return Container(
      constraints: BoxConstraints(maxWidth: width),
      padding: const EdgeInsets.fromLTRB(9, 8, 5, 8),
      decoration: BoxDecoration(
        color: widget.palette.surface,
        border: Border.all(color: widget.palette.border),
        borderRadius: BorderRadius.circular(9),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(
            attachment.isImage ? LucideIcons.image : LucideIcons.fileText,
            size: 19,
            color: widget.palette.secondaryIcon,
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  attachment.name,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(color: widget.palette.text, fontSize: 12),
                ),
                Text(
                  attachment.isAvailable
                      ? sizeLabel
                      : l10n.attachmentUnavailable,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: attachment.isAvailable
                        ? widget.palette.secondaryText
                        : widget.palette.accentIcon,
                    fontSize: OpenChatTypography.metadata,
                  ),
                ),
              ],
            ),
          ),
          if (widget.onRemoveAttachment != null)
            IconButton(
              onPressed: () => widget.onRemoveAttachment?.call(attachment.id),
              tooltip: l10n.removeAttachment,
              constraints: const BoxConstraints.tightFor(width: 28, height: 28),
              padding: EdgeInsets.zero,
              icon: Icon(
                LucideIcons.x,
                size: 16,
                color: widget.palette.secondaryIcon,
              ),
            ),
        ],
      ),
    );
  }

  Widget _unavailableTile(BuildContext context) => Container(
    color: widget.palette.surface,
    alignment: Alignment.center,
    padding: const EdgeInsets.all(10),
    child: Text(
      context.openchatL10n.attachmentUnavailable,
      maxLines: 2,
      overflow: TextOverflow.ellipsis,
      textAlign: TextAlign.center,
      style: TextStyle(color: widget.palette.accentIcon, fontSize: 12),
    ),
  );

  ImageProvider<Object>? _imageProvider(ChatAttachment attachment) {
    if (!attachment.isAvailable) return null;
    final bytes = attachment.bytes;
    if (bytes != null) return MemoryImage(bytes);
    final path = attachment.localPath;
    if (path != null) return FileImage(File(path));
    return null;
  }

  Future<void> _showImagePreview(
    BuildContext context,
    ImageProvider<Object> imageProvider,
    String imageName,
  ) async {
    final l10n = context.openchatL10n;
    final size = MediaQuery.sizeOf(context);
    final width = math.max(0.0, size.width - 48).toDouble();
    final height = math.max(0.0, size.height - 48).toDouble();
    try {
      await showDialog<void>(
        context: context,
        barrierColor: Colors.black.withValues(alpha: 0.84),
        builder: (dialogContext) => Dialog(
          backgroundColor: Colors.transparent,
          insetPadding: const EdgeInsets.all(24),
          constraints: BoxConstraints(maxWidth: width, maxHeight: height),
          child: SizedBox(
            width: width,
            height: height,
            child: Stack(
              children: [
                Positioned.fill(
                  child: InteractiveViewer(
                    minScale: 0.75,
                    maxScale: 6,
                    child: Semantics(
                      image: true,
                      label: imageName,
                      child: Image(
                        image: imageProvider,
                        fit: BoxFit.contain,
                        errorBuilder: (context, error, stackTrace) => Center(
                          child: Text(
                            l10n.attachmentUnavailable,
                            style: TextStyle(color: widget.palette.text),
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
                Positioned(
                  top: 8,
                  right: 8,
                  child: IconButton(
                    onPressed: () => Navigator.of(dialogContext).pop(),
                    tooltip: l10n.close,
                    style: IconButton.styleFrom(
                      backgroundColor: widget.palette.surface.withValues(
                        alpha: 0.94,
                      ),
                      foregroundColor: widget.palette.text,
                    ),
                    icon: const Icon(LucideIcons.x),
                  ),
                ),
              ],
            ),
          ),
        ),
      );
    } finally {
      await imageProvider.evict();
    }
  }
}
