import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/chatgpt_connection.dart';
import 'package:openchat/features/chat/presentation/widgets/chat_composer.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

void main() {
  for (final locale in <Locale>[
    const Locale('tr'),
    const Locale('de'),
    const Locale('fr'),
  ]) {
    for (final platform in <TargetPlatform>[
      TargetPlatform.windows,
      TargetPlatform.android,
    ]) {
      testWidgets(
        'composer menus fit enlarged ${locale.languageCode} text on ${platform.name}',
        (tester) async {
          tester.view.physicalSize = const Size(320, 600);
          tester.view.devicePixelRatio = 1;
          addTearDown(tester.view.resetPhysicalSize);
          addTearDown(tester.view.resetDevicePixelRatio);
          final controller = TextEditingController();
          addTearDown(controller.dispose);
          final l10n = await AppLocalizations.delegate.load(locale);
          var sent = 0;
          var selected = 0;
          const modelTitle = 'Long model name for readable selector testing';
          await tester.pumpWidget(
            MaterialApp(
              locale: locale,
              localizationsDelegates: AppLocalizations.localizationsDelegates,
              supportedLocales: AppLocalizations.supportedLocales,
              theme: OpenChatTheme.dark.copyWith(platform: platform),
              builder: (context, child) {
                if (child == null) {
                  throw StateError('Composer test route is missing.');
                }
                return MediaQuery(
                  data: MediaQuery.of(context)
                      .copyWith(textScaler: TextScaler.linear(2)),
                  child: child,
                );
              },
              home: Scaffold(
                body: Center(
                  child: ChatComposer(
                    controller: controller,
                    onSendMessage: () => sent++,
                    canSendMessage: true,
                    showReasoningSelector: true,
                    reasoningOptions: const ['low', 'medium', 'high'],
                    onReasoningSelected: (_) {},
                    onToolPermissionModeChanged: (_) {},
                    providerId: 'opencode',
                    modelLabel: modelTitle,
                    selectedModelId: 'test-model',
                    onModelSelected: (_) => selected++,
                    onProviderSelected: (_) {},
                    models: const [
                      ChatGptModel(
                        id: 'test-model',
                        displayName: modelTitle,
                        isAvailable: true,
                        reasoningLevels: [],
                        providerId: 'opencode',
                        groupId: 'free',
                        contextWindow: 200000,
                      ),
                    ],
                  ),
                ),
              ),
            ),
          );
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);
          final send = find.descendant(
            of: find.byTooltip(l10n.send),
            matching: find.byType(IconButton),
          );
          expect(tester.widget<IconButton>(send).onPressed, isNull);
          await tester.enterText(find.byType(TextField), 'Test message');
          await tester.pump();
          expect(tester.widget<IconButton>(send).onPressed, isNotNull);
          await tester.tap(send);
          expect(sent, 1);
          final permission = find.widgetWithText(
            OutlinedButton,
            l10n.toolPermissionRequireApproval,
          );
          expect(tester.getSize(permission).height, greaterThanOrEqualTo(44));
          await tester.tap(permission);
          await tester.pumpAndSettle();
          final access = find.widgetWithText(
            MenuItemButton,
            l10n.toolPermissionFullAccess,
          );
          await tester.ensureVisible(access);
          await tester.tap(access);
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);
          final reasoning = find.widgetWithText(
            OutlinedButton,
            l10n.reasoningDefault,
          );
          await tester.tap(reasoning);
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);
          await tester.sendKeyEvent(LogicalKeyboardKey.escape);
          await tester.pumpAndSettle();
          expect(find.byType(MenuItemButton), findsNothing);
          await tester.tap(find.byTooltip(modelTitle).first);
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);
          final option = find.text(modelTitle).last;
          await tester.ensureVisible(option);
          await tester.tap(option);
          await tester.pumpAndSettle();
          expect(selected, 1);
          expect(tester.takeException(), isNull);
        },
      );
    }
  }

  testWidgets('composer keeps send, newline, stop, and attachment actions', (
    tester,
  ) async {
    final controller = TextEditingController();
    addTearDown(controller.dispose);
    final l10n = await AppLocalizations.delegate.load(const Locale('tr'));
    var sent = 0;
    var stopped = 0;
    var attached = 0;

    Future<void> pumpComposer({bool isSending = false}) async {
      await tester.pumpWidget(
        MaterialApp(
          locale: const Locale('tr'),
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          theme: OpenChatTheme.dark,
          home: Scaffold(
            body: Center(
              child: SizedBox(
                width: 720,
                child: ChatComposer(
                  controller: controller,
                  onSendMessage: () => sent++,
                  canSendMessage: true,
                  isSending: isSending,
                  onStopMessage: () => stopped++,
                  attachmentsEnabled: true,
                  onAddAttachments: () => attached++,
                  showReasoningSelector: false,
                  providerId: 'opencode',
                ),
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
    }

    await pumpComposer();
    final field = find.byType(TextField);
    await tester.tap(field);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    expect(sent, 0);

    await tester.enterText(field, 'Test message');
    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
    expect(sent, 0);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    expect(sent, 1);

    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    expect(attached, 1);
    expect(sent, 1);

    await tester.tap(find.byTooltip(l10n.send));
    expect(sent, 2);
    await tester.tap(find.byTooltip(l10n.attachFile));
    expect(attached, 2);

    await pumpComposer(isSending: true);
    await tester.tap(field);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    expect(stopped, 1);
    await tester.tap(find.byTooltip(l10n.stop));
    expect(stopped, 2);
    expect(tester.takeException(), isNull);
  });

  testWidgets('explains when the selected model does not report tool support', (
    tester,
  ) async {
    const locale = Locale('tr');
    final l10n = await AppLocalizations.delegate.load(locale);
    final controller = TextEditingController();
    addTearDown(controller.dispose);

    await tester.pumpWidget(
      _composerApp(
        controller: controller,
        selectedModelId: 'unknown-tools-model',
        supportsToolCalls: null,
      ),
    );

    final tooltip = tester.widget<Tooltip>(
      find
          .ancestor(
            of: find.widgetWithText(
              OutlinedButton,
              l10n.toolPermissionRequireApproval,
            ),
            matching: find.byType(Tooltip),
          )
          .first,
    );
    expect(tooltip.message, contains(l10n.selectedModelToolSupportUnknown));
    expect(
      tooltip.message,
      contains(l10n.toolPermissionRequireApprovalDescription),
    );
  });

  testWidgets('explains when the selected model does not support tool calls', (
    tester,
  ) async {
    const locale = Locale('tr');
    final l10n = await AppLocalizations.delegate.load(locale);
    final controller = TextEditingController();
    addTearDown(controller.dispose);

    await tester.pumpWidget(
      _composerApp(
        controller: controller,
        selectedModelId: 'no-tools-model',
        supportsToolCalls: false,
      ),
    );

    final tooltip = tester.widget<Tooltip>(
      find
          .ancestor(
            of: find.widgetWithText(
              OutlinedButton,
              l10n.toolPermissionRequireApproval,
            ),
            matching: find.byType(Tooltip),
          )
          .first,
    );
    expect(
      tooltip.message,
      contains(l10n.selectedModelDoesNotSupportToolCalls),
    );
    expect(
      tooltip.message,
      contains(l10n.toolPermissionRequireApprovalDescription),
    );
  });

  testWidgets('does not warn when the selected model reports tool support', (
    tester,
  ) async {
    const locale = Locale('tr');
    final l10n = await AppLocalizations.delegate.load(locale);
    final controller = TextEditingController();
    addTearDown(controller.dispose);

    await tester.pumpWidget(
      _composerApp(
        controller: controller,
        selectedModelId: 'tools-model',
        supportsToolCalls: true,
      ),
    );

    final permissionButton = find.widgetWithText(
      OutlinedButton,
      l10n.toolPermissionRequireApproval,
    );
    expect(permissionButton, findsOneWidget);
    final tooltip = tester.widget<Tooltip>(
      find.ancestor(of: permissionButton, matching: find.byType(Tooltip)),
    );
    expect(tooltip.message, l10n.toolPermissionRequireApprovalDescription);
  });
}

Widget _composerApp({
  required TextEditingController controller,
  required String selectedModelId,
  required bool? supportsToolCalls,
}) {
  return MaterialApp(
    locale: const Locale('tr'),
    localizationsDelegates: AppLocalizations.localizationsDelegates,
    supportedLocales: AppLocalizations.supportedLocales,
    theme: OpenChatTheme.light,
    home: Scaffold(
      body: Center(
        child: SizedBox(
          width: 1000,
          child: ChatComposer(
            controller: controller,
            onSendMessage: () {},
            canSendMessage: false,
            showReasoningSelector: false,
            providerId: 'opencode',
            selectedModelId: selectedModelId,
            contextSupportsTools: supportsToolCalls,
          ),
        ),
      ),
    ),
  );
}
