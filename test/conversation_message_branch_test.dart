import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/presentation/widgets/conversation_pane.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

import 'support/shad_test_scope.dart';

void main() {
  testWidgets('edits the selected user message before starting a branch', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1200, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final controller = TextEditingController();
    addTearDown(controller.dispose);
    ChatMessage? branchedMessage;
    String? editedContent;
    const message = ChatMessage(
      id: 'user-branch-message',
      role: ChatMessageRole.user,
      content: 'Original question',
    );
    const locale = Locale('tr');
    final l10n = await AppLocalizations.delegate.load(locale);

    await tester.pumpWidget(
      MaterialApp(
        locale: locale,
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark,
        builder: openChatShadTestBuilder,
        home: Scaffold(
          body: ConversationPane(
            messageController: controller,
            showHistoryButton: false,
            onOpenHistory: () {},
            onSendMessage: () {},
            providerId: 'opencode',
            messages: const <ChatMessage>[message],
            onBranchMessage: (sourceMessage, content) async {
              branchedMessage = sourceMessage;
              editedContent = content;
            },
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byTooltip(l10n.conversationBranchEditTitle));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.descendant(
        of: find.byType(AlertDialog),
        matching: find.byType(TextField),
      ),
      'Revised question',
    );
    await tester.tap(find.text(l10n.conversationBranchStart));
    await tester.pumpAndSettle();

    expect(branchedMessage?.id, message.id);
    expect(editedContent, 'Revised question');
  });
}
