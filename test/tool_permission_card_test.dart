import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/tool_permission_request.dart';
import 'package:openchat/features/chat/presentation/widgets/chat_composer.dart';
import 'package:openchat/features/chat/presentation/widgets/conversation_pane.dart';
import 'package:openchat/features/chat/presentation/widgets/tool_permission_card.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

void main() {
  test('parses only complete permission requests', () {
    expect(
      ToolPermissionRequest.fromEvent(const <String, Object?>{
        'approvalRequestId': 'approval-1',
        'toolName': 'list_files',
        'targetPath': r'C:\project',
        'arguments': <String, Object?>{'path': r'C:\project'},
      }),
      isA<ToolPermissionRequest>(),
    );
    expect(
      ToolPermissionRequest.fromEvent(const <String, Object?>{
        'approvalRequestId': 'approval-1',
        'toolName': 'list_files',
        'targetPath': r'C:\project',
      }),
      isNull,
    );
  });

  testWidgets('shows the pending request above the composer and sends choices', (
    tester,
  ) async {
    const locale = Locale('tr');
    final l10n = await AppLocalizations.delegate.load(locale);
    final controller = TextEditingController();
    addTearDown(controller.dispose);
    var approved = false;
    var denied = false;
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
          body: ConversationPane(
            messageController: controller,
            showHistoryButton: false,
            onOpenHistory: () {},
            onSendMessage: () {},
            providerId: 'opencode',
            selectedModelLabel: 'Test model',
            toolPermissionRequest: const ToolPermissionRequest(
              id: 'approval-1',
              toolName: 'list_files',
              targetPath: r'C:\project',
              arguments: <String, Object?>{'path': r'C:\project'},
            ),
            onApproveToolPermission: () => approved = true,
            onDenyToolPermission: () => denied = true,
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    final card = find.byKey(const ValueKey<String>('tool-permission-card'));
    final composer = find.byType(ChatComposer);
    expect(card, findsOneWidget);
    expect(find.text(l10n.toolListFiles), findsOneWidget);
    expect(find.text(r'C:\project'), findsOneWidget);
    expect(tester.getTopLeft(card).dy, lessThan(tester.getTopLeft(composer).dy));

    await tester.tap(find.text(l10n.toolPermissionAllowOnce));
    await tester.pump();
    expect(approved, isTrue);

    await tester.tap(find.text(l10n.toolPermissionDeny));
    await tester.pump();
    expect(denied, isTrue);
  });

  testWidgets('disables both choices while a decision is being sent', (
    tester,
  ) async {
    const locale = Locale('en');
    final l10n = await AppLocalizations.delegate.load(locale);

    await tester.pumpWidget(
      MaterialApp(
        locale: locale,
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        home: const Scaffold(
          body: ToolPermissionCard(
            request: ToolPermissionRequest(
              id: 'approval-1',
              toolName: 'read_file',
              targetPath: r'C:\project\notes.md',
              arguments: <String, Object?>{
                'path': r'C:\project\notes.md',
              },
            ),
            isResponding: true,
          ),
        ),
      ),
    );

    expect(
      tester.widget<TextButton>(
        find.ancestor(
          of: find.text(l10n.toolPermissionDeny),
          matching: find.byType(TextButton),
        ),
      ).onPressed,
      isNull,
    );
    expect(
      tester.widget<FilledButton>(
        find.ancestor(
          of: find.byType(CircularProgressIndicator),
          matching: find.byType(FilledButton),
        ),
      ).onPressed,
      isNull,
    );
  });
}
