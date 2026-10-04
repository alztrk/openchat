import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/presentation/widgets/chat_composer.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

void main() {
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
    expect(tooltip.message, l10n.selectedModelToolSupportUnknown);
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
    expect(tooltip.message, l10n.selectedModelDoesNotSupportToolCalls);
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
    expect(
      find.ancestor(of: permissionButton, matching: find.byType(Tooltip)),
      findsNothing,
    );
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
