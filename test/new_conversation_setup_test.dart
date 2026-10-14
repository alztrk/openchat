import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_ui/shadcn_ui.dart' as shad;
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/presentation/widgets/conversation_pane.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('offers connection recovery after authentication failure', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1000, 760);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final messageController = TextEditingController();
    addTearDown(messageController.dispose);
    var settingsOpenCount = 0;

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('tr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark.copyWith(platform: TargetPlatform.windows),
        builder: (context, child) => shad.ShadTheme(
          data: OpenChatTheme.shadFromTheme(Theme.of(context)),
          child: child ?? const SizedBox.shrink(),
        ),
        home: Scaffold(
          body: SizedBox(
            width: 900,
            height: 700,
            child: ConversationPane(
              messageController: messageController,
              showHistoryButton: false,
              onOpenHistory: () {},
              onSendMessage: () {},
              providerId: 'chatgpt',
              messages: const [
                ChatMessage(
                  id: 'failed-response',
                  role: ChatMessageRole.assistant,
                  content: '',
                  status: ChatMessageStatus.failed,
                  failureCode: 'authentication_required',
                ),
              ],
              onOpenConnections: () => settingsOpenCount++,
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    final recoveryAction = find.text('Bağlantılara Git');
    expect(recoveryAction, findsOneWidget);
    await tester.tap(recoveryAction);
    await tester.pump();
    expect(settingsOpenCount, 1);
  });

  testWidgets('keeps catalog verification and recovery visible on failure', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1000, 760);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final messageController = TextEditingController();
    addTearDown(messageController.dispose);
    var retryCount = 0;
    var settingsOpenCount = 0;

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('tr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark.copyWith(platform: TargetPlatform.windows),
        builder: (context, child) => shad.ShadTheme(
          data: OpenChatTheme.shadFromTheme(Theme.of(context)),
          child: child ?? const SizedBox.shrink(),
        ),
        home: Scaffold(
          body: SizedBox(
            width: 900,
            height: 700,
            child: ConversationPane(
              messageController: messageController,
              showHistoryButton: false,
              onOpenHistory: () {},
              onSendMessage: () {},
              providerId: 'chatgpt',
              showFirstRunGuide: true,
              hasConfiguredProvider: true,
              modelCatalogFailed: true,
              onOpenConnections: () => settingsOpenCount++,
              onRetryModels: () => retryCount++,
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Sağlayıcıyı doğrula'), findsOneWidget);
    expect(find.text('Modeller kullanılamıyor'), findsOneWidget);
    expect(find.text('Tekrar dene'), findsOneWidget);
    expect(find.textContaining('Asistan profili:'), findsOneWidget);

    await tester.tap(find.text('Tekrar dene'));
    await tester.pump();
    expect(retryCount, 1);

    await tester.tap(find.widgetWithText(FilledButton, 'Bağlantılar'));
    await tester.pump();
    expect(settingsOpenCount, 1);
  });

  for (final locale in AppLocalizations.supportedLocales) {
    testWidgets(
      'does not count a cached catalog as verified in ${locale.languageCode}',
      (tester) async {
        tester.view.physicalSize = const Size(1000, 760);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);

        final messageController = TextEditingController();
        addTearDown(messageController.dispose);
        var refreshCount = 0;

        await tester.pumpWidget(
          MaterialApp(
            locale: locale,
            localizationsDelegates: AppLocalizations.localizationsDelegates,
            supportedLocales: AppLocalizations.supportedLocales,
            theme: OpenChatTheme.dark.copyWith(
              platform: TargetPlatform.windows,
            ),
            builder: (context, child) => shad.ShadTheme(
              data: OpenChatTheme.shadFromTheme(Theme.of(context)),
              child: child ?? const SizedBox.shrink(),
            ),
            home: Scaffold(
              body: SizedBox(
                width: 900,
                height: 700,
                child: ConversationPane(
                  messageController: messageController,
                  showHistoryButton: false,
                  onOpenHistory: () {},
                  onSendMessage: () {},
                  providerId: 'opencode',
                  hasConfiguredProvider: true,
                  hasAvailableModels: true,
                  hasSelectedModel: true,
                  showFirstRunGuide: true,
                  modelCatalogStale: true,
                  onRetryModels: () => refreshCount++,
                ),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();

        final paneContext = tester.element(find.byType(ConversationPane));
        final l10n = AppLocalizations.of(paneContext);
        if (l10n == null) {
          throw StateError('Conversation setup localization is missing.');
        }
        expect(
          find.text(l10n.providerVerificationStaleCatalog),
          findsOneWidget,
        );
        final verificationStep = find.bySemanticsLabel(
          '2. ${l10n.providerCatalogVerification}',
        );
        expect(verificationStep, findsOneWidget);
        expect(
          tester.getSemantics(verificationStep).getSemanticsData().value,
          l10n.setupStepCurrent,
        );
        await tester.tap(find.text(l10n.refreshModels));
        await tester.pump();
        expect(refreshCount, 1);
        expect(tester.takeException(), isNull);
      },
    );
  }

  testWidgets('explains provider choices and opens connection settings', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1000, 760);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final messageController = TextEditingController();
    addTearDown(messageController.dispose);
    var settingsOpenCount = 0;

    Future<void> pumpSetup({
      required bool hasConfiguredProvider,
      bool hasAvailableModels = false,
      bool hasSelectedModel = false,
      bool hasCompletedFirstResponse = false,
    }) async {
      await tester.pumpWidget(
        MaterialApp(
          locale: const Locale('tr'),
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          theme: OpenChatTheme.dark.copyWith(platform: TargetPlatform.windows),
          builder: (context, child) => shad.ShadTheme(
            data: OpenChatTheme.shadFromTheme(Theme.of(context)),
            child: child ?? const SizedBox.shrink(),
          ),
          home: Scaffold(
            body: SizedBox(
              width: 900,
              height: 700,
              child: ConversationPane(
                messageController: messageController,
                showHistoryButton: false,
                onOpenHistory: () {},
                onSendMessage: () {},
                providerId: 'opencode',
                hasConfiguredProvider: hasConfiguredProvider,
                hasAvailableModels: hasAvailableModels,
                hasSelectedModel: hasSelectedModel,
                showFirstRunGuide: !hasCompletedFirstResponse,
                onOpenConnections: () => settingsOpenCount++,
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
    }

    await pumpSetup(hasConfiguredProvider: false);

    expect(find.textContaining('ChatGPT hesabınla oturum aç'), findsOneWidget);
    final paneContext = tester.element(find.byType(ConversationPane));
    final l10n = AppLocalizations.of(paneContext)!;
    expect(
      tester
          .getSemantics(find.bySemanticsLabel('1. ${l10n.connections}'))
          .getSemanticsData()
          .value,
      l10n.setupStepCurrent,
    );
    final connectionsAction = find.widgetWithText(FilledButton, 'Bağlantılar');
    expect(connectionsAction, findsOneWidget);
    await tester.tap(connectionsAction);
    await tester.pump();
    expect(settingsOpenCount, 1);

    await pumpSetup(hasConfiguredProvider: true);
    expect(find.text('Sağlayıcı bağlantısını kontrol et'), findsOneWidget);
    expect(
      find.textContaining('Kaydedilmiş oturum veya API anahtarı'),
      findsOneWidget,
    );
    expect(
      tester
          .getSemantics(
            find.bySemanticsLabel('2. ${l10n.providerCatalogVerification}'),
          )
          .getSemanticsData()
          .value,
      l10n.setupStepCurrent,
    );

    await pumpSetup(hasConfiguredProvider: true, hasAvailableModels: true);
    expect(find.text(l10n.emptyChatSelectModelTitle), findsOneWidget);
    expect(
      tester
          .getSemantics(find.bySemanticsLabel('3. ${l10n.modelSelection}'))
          .getSemanticsData()
          .value,
      l10n.setupStepCurrent,
    );

    await pumpSetup(
      hasConfiguredProvider: true,
      hasAvailableModels: true,
      hasSelectedModel: true,
    );
    expect(find.text(l10n.emptyChatWelcomeTitle), findsOneWidget);
    final firstMessageStep = find.bySemanticsLabel('4. ${l10n.send}');
    expect(firstMessageStep, findsOneWidget);
    expect(
      tester.getSemantics(firstMessageStep).getSemanticsData().value,
      l10n.setupStepCurrent,
    );

    await pumpSetup(
      hasConfiguredProvider: true,
      hasAvailableModels: true,
      hasSelectedModel: true,
      hasCompletedFirstResponse: true,
    );
    expect(find.bySemanticsLabel('1. ${l10n.connections}'), findsNothing);
  });

  testWidgets('shows temporary chat disclosure and end action', (tester) async {
    tester.view.physicalSize = const Size(1000, 760);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final messageController = TextEditingController();
    addTearDown(messageController.dispose);
    var endCount = 0;

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('tr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark.copyWith(platform: TargetPlatform.windows),
        builder: (context, child) => shad.ShadTheme(
          data: OpenChatTheme.shadFromTheme(Theme.of(context)),
          child: child ?? const SizedBox.shrink(),
        ),
        home: Scaffold(
          body: SizedBox(
            width: 900,
            height: 700,
            child: ConversationPane(
              messageController: messageController,
              showHistoryButton: false,
              onOpenHistory: () {},
              onSendMessage: () {},
              providerId: 'gemini',
              hasConfiguredProvider: true,
              isTemporaryConversation: true,
              temporaryProviderName: 'Gemini',
              onEndTemporaryConversation: () => endCount++,
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(
      find.text(
        AppLocalizations.of(tester.element(find.byType(ConversationPane)))!
            .temporaryChatNotice('Gemini'),
      ),
      findsOneWidget,
    );
    expect(find.textContaining('Assistant profile:'), findsNothing);
    await tester.tap(find.text('Bitir'));
    await tester.pump();
    expect(endCount, 1);
  });

  for (final locale in AppLocalizations.supportedLocales) {
    testWidgets(
      'keeps the first-response step accessible at narrow width in ${locale.languageCode}',
      (tester) async {
        tester.view.physicalSize = const Size(320, 800);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);

        final messageController = TextEditingController();
        addTearDown(messageController.dispose);

        await tester.pumpWidget(
          MaterialApp(
            locale: locale,
            localizationsDelegates: AppLocalizations.localizationsDelegates,
            supportedLocales: AppLocalizations.supportedLocales,
            theme: OpenChatTheme.dark.copyWith(
              platform: TargetPlatform.windows,
            ),
            builder: (context, child) => shad.ShadTheme(
              data: OpenChatTheme.shadFromTheme(Theme.of(context)),
              child: MediaQuery(
                data: MediaQuery.of(context)
                    .copyWith(textScaler: const TextScaler.linear(2)),
                child: child ?? const SizedBox.shrink(),
              ),
            ),
            home: Scaffold(
              body: ConversationPane(
                messageController: messageController,
                showHistoryButton: false,
                onOpenHistory: () {},
                onSendMessage: () {},
                providerId: 'opencode',
                hasConfiguredProvider: true,
                hasAvailableModels: true,
                hasSelectedModel: true,
                showFirstRunGuide: true,
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();

        final l10n = AppLocalizations.of(
          tester.element(find.byType(ConversationPane)),
        )!;
        final sendStep = find.bySemanticsLabel('4. ${l10n.send}');
        expect(sendStep, findsOneWidget);
        expect(
          tester.getSemantics(sendStep).getSemanticsData().value,
          l10n.setupStepCurrent,
        );
        expect(tester.takeException(), isNull);
      },
    );
  }
}
