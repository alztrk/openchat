import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/presentation/widgets/assistant_message.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

void main() {
  testWidgets('explains context window failures in the assistant card', (
    tester,
  ) async {
    const locale = Locale('tr');
    final l10n = await AppLocalizations.delegate.load(locale);

    tester.view.physicalSize = const Size(1200, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final failedMessage = ChatMessage(
      id: 'msg-context-overflow',
      role: ChatMessageRole.assistant,
      content: '',
      createdAt: DateTime.utc(2026, 10, 1, 12),
      status: ChatMessageStatus.failed,
      failureCode: 'context_window_exceeded',
    );

    await tester.pumpWidget(
      MaterialApp(
        locale: locale,
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        home: Scaffold(
          body: AssistantMessage(
            message: failedMessage,
            modelLabel: 'OpenCode Test Model',
            providerId: 'opencode',
          ),
        ),
      ),
    );

    expect(find.text(l10n.contextWindowExceeded), findsOneWidget);
  });

  testWidgets(
    'renders live tokens per second and output tokens during streaming',
    (tester) async {
      const locale = Locale('tr');
      final l10n = await AppLocalizations.delegate.load(locale);

      tester.view.physicalSize = const Size(1200, 900);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final liveStreamingMessage = ChatMessage(
        id: 'msg-stream-1',
        role: ChatMessageRole.assistant,
        content: 'Merhaba, bu bir akış testidir.',
        createdAt: DateTime.utc(2026, 9, 30, 19, 15),
        status: ChatMessageStatus.streaming,
        outputTokens: 42,
        tokensPerSecond: 35.4,
      );

      await tester.pumpWidget(
        MaterialApp(
          locale: locale,
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          theme: OpenChatTheme.light,
          home: Scaffold(
            body: AssistantMessage(
              message: liveStreamingMessage,
              modelLabel: 'OpenCode Test Model',
              providerId: 'opencode',
            ),
          ),
        ),
      );

      await tester.pump();

      // Format localized time depending on local time zone, checking for '35,4' and '42'
      expect(find.textContaining('35,4'), findsOneWidget);
      expect(find.textContaining('42'), findsOneWidget);
      expect(find.textContaining(l10n.unavailableValue), findsNothing);
    },
  );

  testWidgets(
    'displays unavailable value when metrics are not yet computed, then updates when live metrics arrive',
    (tester) async {
      const locale = Locale('tr');
      final l10n = await AppLocalizations.delegate.load(locale);

      tester.view.physicalSize = const Size(1200, 900);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final initialMessage = ChatMessage(
        id: 'msg-stream-2',
        role: ChatMessageRole.assistant,
        content: 'Başlangıç...',
        createdAt: DateTime.utc(2026, 9, 30, 19, 15),
        status: ChatMessageStatus.streaming,
      );

      await tester.pumpWidget(
        MaterialApp(
          locale: locale,
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          theme: OpenChatTheme.light,
          home: Scaffold(
            body: AssistantMessage(
              message: initialMessage,
              modelLabel: 'OpenCode Test Model',
              providerId: 'opencode',
            ),
          ),
        ),
      );

      await tester.pump();

      // Initially unavailableValue '-' is rendered for both t/s and token
      expect(find.textContaining(l10n.unavailableValue), findsOneWidget);

      final updatedMessage = ChatMessage(
        id: 'msg-stream-2',
        role: ChatMessageRole.assistant,
        content: 'Başlangıç ve devam eden akış...',
        createdAt: DateTime.utc(2026, 9, 30, 19, 15),
        status: ChatMessageStatus.streaming,
        outputTokens: 78,
        tokensPerSecond: 28.0,
      );

      await tester.pumpWidget(
        MaterialApp(
          locale: locale,
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          theme: OpenChatTheme.light,
          home: Scaffold(
            body: AssistantMessage(
              message: updatedMessage,
              modelLabel: 'OpenCode Test Model',
              providerId: 'opencode',
            ),
          ),
        ),
      );

      await tester.pump();

      expect(find.textContaining('28'), findsOneWidget);
      expect(find.textContaining('78'), findsOneWidget);
    },
  );
}
