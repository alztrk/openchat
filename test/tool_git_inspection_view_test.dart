import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/presentation/widgets/tool_git_inspection_view.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

void main() {
  Widget buildTestWidget({required Locale locale, required Widget child}) {
    return MaterialApp(
      locale: locale,
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      theme: OpenChatTheme.dark,
      home: Scaffold(body: SingleChildScrollView(child: child)),
    );
  }

  testWidgets('shows Git branch and changed file states', (tester) async {
    const locale = Locale('tr');
    final l10n = await AppLocalizations.delegate.load(locale);
    final activity = ChatToolActivity(
      callId: 'git-status-1',
      name: 'git_status',
      status: ChatToolActivityStatus.completed,
      arguments: const <String, Object?>{},
      output: const <String, Object?>{
        'branch': 'feature/tooling',
        'upstream': 'origin/feature/tooling',
        'ahead': 2,
        'behind': 1,
        'truncated': false,
        'files': [
          {
            'path': 'lib/main.dart',
            'status': ' M',
            'staged': false,
            'unstaged': true,
            'originalPath': null,
          },
          {
            'path': 'new file.txt',
            'status': '??',
            'staged': false,
            'unstaged': true,
            'originalPath': null,
          },
        ],
      },
    );

    await tester.pumpWidget(
      buildTestWidget(
        locale: locale,
        child: ToolGitInspectionResult(
          activity: activity,
          palette: OpenChatPalette.dark,
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('${l10n.toolGitBranch}: '), findsOneWidget);
    expect(find.text('feature/tooling'), findsOneWidget);
    expect(find.text('lib/main.dart'), findsOneWidget);
    expect(find.text('new file.txt'), findsOneWidget);
    expect(find.text(l10n.toolGitUnstaged), findsAtLeastNWidgets(1));
    expect(find.text('${l10n.toolGitAhead} 2'), findsOneWidget);
  });

  testWidgets('shows bounded staged and unstaged diffs', (tester) async {
    const locale = Locale('en');
    final l10n = await AppLocalizations.delegate.load(locale);
    final activity = ChatToolActivity(
      callId: 'git-diff-1',
      name: 'git_diff',
      status: ChatToolActivityStatus.completed,
      arguments: const <String, Object?>{},
      output: const <String, Object?>{
        'staged': 'diff --git a/a.txt b/a.txt\n+staged',
        'stagedTruncated': false,
        'unstaged': 'diff --git a/b.txt b/b.txt\n+unstaged',
        'unstagedTruncated': true,
      },
    );

    await tester.pumpWidget(
      buildTestWidget(
        locale: locale,
        child: ToolGitInspectionResult(
          activity: activity,
          palette: OpenChatPalette.dark,
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text(l10n.toolGitStaged), findsOneWidget);
    expect(find.text(l10n.toolGitUnstaged), findsOneWidget);
    expect(find.textContaining('diff --git a/a.txt'), findsOneWidget);
    expect(find.textContaining('diff --git a/b.txt'), findsOneWidget);
    expect(find.text(l10n.toolOperationTruncated), findsOneWidget);
  });
}
