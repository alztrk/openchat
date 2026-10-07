import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/presentation/widgets/assistant_message.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

void main() {
  testWidgets('explains local engine failures in the assistant card', (
    tester,
  ) async {
    const locale = Locale('tr');
    final l10n = await AppLocalizations.delegate.load(locale);
    final failures = <({String code, String description})>[
      (
        code: 'local_engine_start_failed',
        description: l10n.localModelStartError,
      ),
      (
        code: 'local_engine_start_timeout',
        description: l10n.localModelStartTimeout,
      ),
      (
        code: 'local_engine_runtime_unavailable',
        description: l10n.localModelRuntimeUnavailable,
      ),
      (
        code: 'local_engine_capability_unavailable',
        description: l10n.localModelContextUnavailable,
      ),
      (
        code: 'local_model_inference_failed',
        description: l10n.localModelInferenceFailed,
      ),
    ];

    tester.view.physicalSize = const Size(1200, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    for (final failure in failures) {
      await tester.pumpWidget(
        MaterialApp(
          locale: locale,
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          theme: OpenChatTheme.light,
          home: Scaffold(
            body: AssistantMessage(
              message: ChatMessage(
                id: 'msg-${failure.code}',
                role: ChatMessageRole.assistant,
                content: '',
                createdAt: DateTime.utc(2026, 10, 1, 12),
                status: ChatMessageStatus.failed,
                failureCode: failure.code,
              ),
              modelLabel: 'Local test model',
              providerId: 'llama_cpp',
            ),
          ),
        ),
      );

      expect(find.text(failure.description), findsOneWidget);
    }
  });

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

  testWidgets('explains provider network failures in the assistant card', (
    tester,
  ) async {
    const locale = Locale('tr');
    final l10n = await AppLocalizations.delegate.load(locale);
    tester.view.physicalSize = const Size(1200, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(
      MaterialApp(
        locale: locale,
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        home: Scaffold(
          body: AssistantMessage(
            message: ChatMessage(
              id: 'provider-network-failure',
              role: ChatMessageRole.assistant,
              content: '',
              createdAt: DateTime.utc(2026, 10, 1, 12),
              status: ChatMessageStatus.failed,
              failureCode: 'network_unavailable',
            ),
            modelLabel: 'Provider test model',
            providerId: 'opencode',
          ),
        ),
      ),
    );

    expect(find.text(l10n.providerNetworkUnavailable), findsOneWidget);
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

  testWidgets('omits missing metrics and updates when live metrics arrive', (
    tester,
  ) async {
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

    expect(find.textContaining(l10n.unavailableValue), findsNothing);
    expect(find.textContaining('t/s'), findsNothing);
    expect(find.textContaining('token'), findsNothing);

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
  });

  testWidgets('renders only available response metadata', (tester) async {
    final l10n = await AppLocalizations.delegate.load(const Locale('tr'));
    for (final outputTokens in <int?>[null, 0, 42]) {
      await tester.pumpWidget(
        MaterialApp(
          locale: const Locale('tr'),
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          theme: OpenChatTheme.dark,
          home: Scaffold(
            body: AssistantMessage(
              message: ChatMessage(
                id: 'metadata-test',
                role: ChatMessageRole.assistant,
                content: 'Response content',
                outputTokens: outputTokens,
              ),
              modelLabel: 'Test model',
              providerId: 'opencode',
            ),
          ),
        ),
      );
      expect(find.textContaining(l10n.unavailableValue), findsNothing);
      expect(find.textContaining(l10n.unavailableTime), findsNothing);
      expect(find.textContaining('t/s'), findsNothing);
      if (outputTokens != null) {
        expect(
          find.text(l10n.responseTokenCount('$outputTokens')),
          findsOneWidget,
        );
      } else {
        expect(find.textContaining('token'), findsNothing);
      }
      expect(find.byTooltip(l10n.copyMessage), findsOneWidget);
      expect(tester.takeException(), isNull);
    }
  });

  testWidgets('failed responses retain a working retry action', (tester) async {
    final l10n = await AppLocalizations.delegate.load(const Locale('tr'));
    var retries = 0;
    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('tr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark,
        home: Scaffold(
          body: AssistantMessage(
            message: const ChatMessage(
              id: 'retry-test',
              role: ChatMessageRole.assistant,
              content: '',
              status: ChatMessageStatus.failed,
              failureCode: 'network_unavailable',
            ),
            modelLabel: 'Test model',
            providerId: 'opencode',
            onRetry: () => retries++,
          ),
        ),
      ),
    );
    expect(find.text(l10n.providerNetworkUnavailable), findsOneWidget);
    await tester.tap(find.byTooltip(l10n.retry));
    expect(retries, 1);
    expect(tester.takeException(), isNull);
  });

  testWidgets('shows a localized explanation for tool request rejection', (
    tester,
  ) async {
    const locale = Locale('tr');
    final l10n = await AppLocalizations.delegate.load(locale);
    tester.view.physicalSize = const Size(1200, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(
      MaterialApp(
        locale: locale,
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        home: Scaffold(
          body: AssistantMessage(
            message: ChatMessage(
              id: 'tool-rejected',
              role: ChatMessageRole.assistant,
              content: '',
              createdAt: DateTime.utc(2026, 10, 1, 12),
              status: ChatMessageStatus.failed,
              failureCode: 'provider_tool_request_rejected',
            ),
            modelLabel: 'Provider test model',
            providerId: 'mistral',
          ),
        ),
      ),
    );

    expect(find.text(l10n.providerToolRequestRejected), findsOneWidget);
  });
}
