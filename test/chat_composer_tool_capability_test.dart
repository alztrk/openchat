import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_select.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/chatgpt_connection.dart';
import 'package:openchat/features/chat/presentation/widgets/chat_composer.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:shadcn_ui/shadcn_ui.dart' as shad;

import 'support/shad_test_scope.dart';

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
          var permissionMode = ToolPermissionMode.requireApproval;
          ToolPermissionMode? selectedPermission;
          String? selectedReasoning;
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
                return openChatShadTestScope(
                  OpenChatTheme.dark,
                  MediaQuery(
                    data: MediaQuery.of(context)
                        .copyWith(textScaler: TextScaler.linear(2)),
                    child: child,
                  ),
                );
              },
              home: Scaffold(
                body: Center(
                  child: StatefulBuilder(
                    builder: (context, setState) => ChatComposer(
                      controller: controller,
                      onSendMessage: () => sent++,
                      canSendMessage: true,
                      showReasoningSelector: true,
                      reasoningOptions: const ['low', 'medium', 'high'],
                      onReasoningSelected: (level) =>
                          setState(() => selectedReasoning = level),
                      toolPermissionMode: permissionMode,
                      onToolPermissionModeChanged: (mode) => setState(() {
                        selectedPermission = mode;
                        permissionMode = mode;
                      }),
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
            ),
          );
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);
          final send = find.descendant(
            of: find.byTooltip(l10n.send),
            matching: find.byType(shad.ShadIconButton),
          );
          expect(tester.widget<shad.ShadIconButton>(send).onPressed, isNull);
          await tester.enterText(find.byType(EditableText), 'Test message');
          await tester.pump();
          expect(tester.widget<shad.ShadIconButton>(send).onPressed, isNotNull);
          await tester.tap(send);
          expect(sent, 1);
          final permission = find.text(l10n.toolPermissionRequireApproval).last;
          final permissionSelect = find.ancestor(
            of: permission,
            matching: find.byWidgetPredicate(
              (widget) => widget is shad.ShadSelect,
            ),
          );
          expect(
            tester.getSize(permissionSelect).height,
            greaterThanOrEqualTo(44),
          );
          await tester.tap(permission);
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);
          expect(find.text(l10n.toolPermissionPlan), findsOneWidget);
          final access = find.text(l10n.toolPermissionFullAccess);
          final optionsScrollable = find
              .ancestor(of: access, matching: find.byType(Scrollable))
              .first;
          await tester.scrollUntilVisible(
            access,
            100,
            scrollable: optionsScrollable,
          );
          expect(tester.takeException(), isNull);
          await tester.tap(access);
          await tester.pumpAndSettle();
          expect(selectedPermission, ToolPermissionMode.fullAccess);
          expect(find.byIcon(LucideIcons.shieldAlert), findsOneWidget);
          expect(tester.takeException(), isNull);
          await tester.tap(find.byTooltip(modelTitle).first);
          await tester.pumpAndSettle();
          expect(find.byIcon(LucideIcons.brain), findsOneWidget);
          expect(tester.takeException(), isNull);
          await tester.tap(find.byType(OpenChatSelect<String?>));
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);
          await tester.tap(find.text(l10n.reasoningHigh).last);
          await tester.pumpAndSettle();
          expect(selectedReasoning, 'high');
          expect(tester.takeException(), isNull);
          await tester.sendKeyEvent(LogicalKeyboardKey.escape);
          await tester.pumpAndSettle();
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
          home: openChatShadTestScope(
            OpenChatTheme.dark,
            Scaffold(
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
        ),
      );
      await tester.pumpAndSettle();
    }

    await pumpComposer();
    final field = find.byType(EditableText);
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
            of: find.text(l10n.toolPermissionRequireApproval).last,
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
            of: find.text(l10n.toolPermissionRequireApproval).last,
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
    final permissionButton = find.text(l10n.toolPermissionRequireApproval).last;
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
    home: openChatShadTestScope(
      OpenChatTheme.light,
      Scaffold(
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
    ),
  );
}
