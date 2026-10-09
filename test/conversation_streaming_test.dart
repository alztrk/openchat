import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/chat_file_change.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/presentation/widgets/assistant_message.dart';
import 'package:openchat/features/chat/presentation/widgets/conversation_pane.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

import 'support/shad_test_scope.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    final iconFont = FontLoader('MaterialIcons')
      ..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'));
    await iconFont.load();
    final fontLoader = FontLoader('Manrope')
      ..addFont(rootBundle.load('assets/fonts/Manrope[wght].ttf'));
    await fontLoader.load();
    final uiFontLoader = FontLoader(OpenChatTypography.uiFontFamily)
      ..addFont(rootBundle.load('assets/fonts/SourceSans3VF-Upright.ttf'));
    await uiFontLoader.load();
    final codeFontLoader = FontLoader(OpenChatTypography.codeFontFamily)
      ..addFont(rootBundle.load('assets/fonts/SourceCodeVF-Upright.ttf'));
    await codeFontLoader.load();
  });

  testWidgets('formats Markdown while an assistant response is streaming', (
    tester,
  ) async {
    _setViewport(tester);
    final message = ValueNotifier(
      const ChatMessage(
        id: 'streaming-answer',
        role: ChatMessageRole.assistant,
        content: 'A normal streamed sentence',
        status: ChatMessageStatus.streaming,
      ),
    );
    addTearDown(message.dispose);

    await tester.pumpWidget(
      _testApp(
        ValueListenableBuilder(
          valueListenable: message,
          builder: (context, value, _) => Padding(
            padding: const EdgeInsets.all(24),
            child: AssistantMessage(
              message: value,
              modelLabel: 'Model',
              providerId: 'opencode',
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    message.value = const ChatMessage(
      id: 'streaming-answer',
      role: ChatMessageRole.assistant,
      content: 'A normal streamed sentence with **bold text**.',
      status: ChatMessageStatus.streaming,
    );
    await tester.pumpAndSettle();

    final formattedText = find.byWidgetPredicate(
      (widget) =>
          widget is RichText && widget.text.toPlainText().contains('bold text'),
    );
    expect(formattedText, findsOneWidget);
    expect(_hasBoldSpan(tester.widget<RichText>(formattedText).text), isTrue);
    expect(
      tester
          .widgetList<RichText>(find.byType(RichText))
          .any((widget) => widget.text.toPlainText().contains('**bold text**')),
      isFalse,
    );
  });

  testWidgets('shows file changes only after the assistant response ends', (
    tester,
  ) async {
    _setViewport(tester);
    final controller = ScrollController();
    final messageController = TextEditingController();
    addTearDown(controller.dispose);
    addTearDown(messageController.dispose);
    addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));
    const activity = ChatToolActivity(
      callId: 'write-1',
      name: 'write_file',
      arguments: <String, Object?>{'path': 'src/example.dart'},
      output: 'File written.',
      status: ChatToolActivityStatus.completed,
      fileChanges: <ChatFileChange>[
        ChatFileChange(
          id: 'change-1',
          path: 'src/example.dart',
          kind: ChatFileChangeKind.modified,
          status: ChatFileChangeState.active,
          addedLines: 2,
          removedLines: 1,
          isBinary: false,
          diffAvailable: true,
          canRevert: true,
        ),
      ],
    );
    final streamingMessage = ChatMessage(
      id: 'assistant-message',
      role: ChatMessageRole.assistant,
      content: 'Updating the file.',
      status: ChatMessageStatus.streaming,
      toolActivities: const <ChatToolActivity>[activity],
    );
    await tester.pumpWidget(
      _testApp(
        _conversationPane(
          <ChatMessage>[streamingMessage],
          controller,
          messageController,
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('1 dosya değiştirildi'), findsNothing);

    final completedMessage = ChatMessage(
      id: streamingMessage.id,
      role: streamingMessage.role,
      content: streamingMessage.content,
      status: ChatMessageStatus.completed,
      toolActivities: streamingMessage.toolActivities,
    );
    await tester.pumpWidget(
      _testApp(
        _conversationPane(
          <ChatMessage>[completedMessage],
          controller,
          messageController,
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('1 dosya değiştirildi'), findsOneWidget);
    expect(find.text('+2 / -1'), findsOneWidget);
    expect(find.text('src/example.dart'), findsOneWidget);
  });

  testWidgets('follows streamed output unless the reader scrolls up', (
    tester,
  ) async {
    _setViewport(tester);
    final messages = ValueNotifier(_chatMessages('Short answer.'));
    final controller = ScrollController();
    final messageController = TextEditingController();
    addTearDown(controller.dispose);
    addTearDown(messageController.dispose);
    addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));

    await tester.pumpWidget(
      _testApp(
        ValueListenableBuilder(
          valueListenable: messages,
          builder: (context, value, _) =>
              _conversationPane(value, controller, messageController),
        ),
      ),
    );
    await tester.pumpAndSettle();

    messages.value = _chatMessages(_longResponse(36));
    await tester.pumpAndSettle();

    expect(controller.position.pixels, greaterThan(0));
    expect(
      controller.position.pixels,
      closeTo(controller.position.maxScrollExtent, 1),
    );

    final priorOffset = controller.position.maxScrollExtent / 2;
    controller.jumpTo(priorOffset);
    await tester.pump();
    messages.value = _chatMessages(_longResponse(48));
    await tester.pumpAndSettle();

    expect(controller.position.pixels, closeTo(priorOffset, 2));
    expect(controller.position.extentAfter, greaterThan(200));

    controller.jumpTo(controller.position.maxScrollExtent);
    await tester.pump();
    messages.value = _chatMessages(_longResponse(60));
    await tester.pumpAndSettle();
    expect(
      controller.position.pixels,
      closeTo(controller.position.maxScrollExtent, 1),
    );
  });

  testWidgets('middle click starts and stops automatic scrolling', (
    tester,
  ) async {
    _setViewport(tester);
    final controller = ScrollController();
    final messageController = TextEditingController();
    addTearDown(controller.dispose);
    addTearDown(messageController.dispose);
    addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));

    await tester.pumpWidget(
      _testApp(
        _conversationPane(
          _chatMessages(_longResponse(60)),
          controller,
          messageController,
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(controller.position.maxScrollExtent, greaterThan(500));
    controller.jumpTo(0);
    await tester.pump();
    final listRect = tester.getRect(find.byType(CustomScrollView));
    final anchor = listRect.center;
    final mouse = TestPointer(
      41,
      PointerDeviceKind.mouse,
      null,
      kMiddleMouseButton,
    );
    await tester.sendEventToBinding(mouse.addPointer(location: anchor));
    await tester.sendEventToBinding(mouse.hover(anchor));
    await tester.sendEventToBinding(mouse.down(anchor));
    await tester.pump();
    await tester.sendEventToBinding(mouse.up());
    await tester.pump();
    await tester.sendEventToBinding(mouse.hover(anchor + const Offset(0, 80)));
    await tester.pump(const Duration(milliseconds: 160));

    expect(controller.position.pixels, greaterThan(0));

    await tester.sendEventToBinding(mouse.down(anchor + const Offset(0, 80)));
    await tester.pump();
    final stoppedOffset = controller.position.pixels;
    await tester.pump(const Duration(milliseconds: 160));

    expect(controller.position.pixels, closeTo(stoppedOffset, 0.1));
    await tester.sendEventToBinding(mouse.up());
    await tester.sendEventToBinding(
      mouse.removePointer(location: anchor + const Offset(0, 80)),
    );
  });
}

Widget _testApp(Widget body) => MaterialApp(
  locale: const Locale('tr'),
  localizationsDelegates: AppLocalizations.localizationsDelegates,
  supportedLocales: AppLocalizations.supportedLocales,
  theme: OpenChatTheme.dark.copyWith(platform: TargetPlatform.windows),
  builder: openChatShadTestBuilder,
  home: Scaffold(body: body),
);

Widget _conversationPane(
  List<ChatMessage> messages,
  ScrollController controller,
  TextEditingController messageController,
) => ConversationPane(
  messageController: messageController,
  showHistoryButton: false,
  onOpenHistory: () {},
  onSendMessage: () {},
  providerId: 'opencode',
  messages: messages,
  messageScrollController: controller,
);

List<ChatMessage> _chatMessages(String assistantContent) => [
  const ChatMessage(
    id: 'user-message',
    role: ChatMessageRole.user,
    content: 'Continue the story.',
  ),
  ChatMessage(
    id: 'assistant-message',
    role: ChatMessageRole.assistant,
    content: assistantContent,
    status: ChatMessageStatus.streaming,
  ),
];

String _longResponse(int paragraphCount) => List<String>.generate(
  paragraphCount,
  (index) =>
      'Streaming paragraph $index. This response continues with enough text '
      'to fill the conversation window and exercise automatic scrolling.',
).join('\n\n');

void _setViewport(WidgetTester tester) {
  tester.view.physicalSize = const Size(1200, 800);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
}

bool _hasBoldSpan(InlineSpan span) {
  if (span is! TextSpan) return false;
  if (span.text?.contains('bold text') == true &&
      (span.style?.fontWeight?.value ?? 0) >= FontWeight.w600.value) {
    return true;
  }
  return span.children?.any(_hasBoldSpan) ?? false;
}
