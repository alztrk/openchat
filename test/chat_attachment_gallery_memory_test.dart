import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/chat_attachment.dart';
import 'package:openchat/features/chat/presentation/widgets/chat_attachment_gallery.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

void main() {
  testWidgets('thumbnail decode is bounded and preview keeps the source', (
    tester,
  ) async {
    tester.view.devicePixelRatio = 2;
    tester.view.physicalSize = const Size(700, 400);
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final imageBytes = Uint8List.fromList([0]);
    final attachment = ChatAttachment(
      id: 'memory-test-image',
      name: 'memory-test.png',
      mimeType: 'image/png',
      sizeBytes: imageBytes.length,
      kind: ChatAttachmentKind.image,
      bytes: imageBytes,
    );

    await tester.pumpWidget(
      MaterialApp(
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: Scaffold(
          body: Align(
            alignment: Alignment.topLeft,
            child: SizedBox(
              width: 280,
              child: ChatAttachmentGallery(
                attachments: [attachment],
                palette: OpenChatPalette.light,
                preferredImageWidth: 220,
                imageHeight: 120,
              ),
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.byType(Image), findsOneWidget);
    final thumbnail = tester.widget<Image>(find.byType(Image));
    expect(thumbnail.image, isA<ResizeImage>());
    final thumbnailProvider = thumbnail.image as ResizeImage;
    expect(thumbnailProvider.width, 1320);
    expect(thumbnailProvider.height, 720);
    expect(thumbnailProvider.policy, ResizeImagePolicy.fit);
    expect(thumbnailProvider.allowUpscaling, isFalse);

    await tester.tap(find.byType(InkWell));
    await tester.pumpAndSettle();

    final images = tester.widgetList<Image>(find.byType(Image)).toList();
    expect(images, hasLength(2));
    expect(images.last.image, isA<MemoryImage>());
  });
}
