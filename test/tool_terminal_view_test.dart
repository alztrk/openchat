import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/presentation/widgets/tool_activity.dart';
import 'package:openchat/features/chat/presentation/widgets/tool_terminal_view.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

void main() {
  Widget buildTestableWidget({
    required Widget child,
    Locale locale = const Locale('tr'),
  }) {
    return MaterialApp(
      locale: locale,
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      theme: OpenChatTheme.light,
      home: Scaffold(body: SingleChildScrollView(child: child)),
    );
  }

  test('cleans ANSI escape codes properly', () {
    const raw = '\u001b[31;1mError occurred\u001b[0m\r\n';
    expect(cleanAnsiCodes(raw).trim(), 'Error occurred');
  });

  testWidgets('renders command, output and success exit code', (tester) async {
    const locale = Locale('tr');
    final l10n = await AppLocalizations.delegate.load(locale);

    final activity = ChatToolActivity(
      callId: 'call_term_1',
      name: 'execute_command',
      status: ChatToolActivityStatus.completed,
      arguments: const <String, Object?>{'command': 'git status'},
      output: const <String, Object?>{
        'command': 'git status',
        'output': 'On branch main\nnothing to commit',
        'exit_code': 0,
        'is_running': false,
      },
    );

    await tester.pumpWidget(
      buildTestableWidget(
        locale: locale,
        child: ToolActivityAccordion(
          activity: activity,
          palette: OpenChatPalette.light,
        ),
      ),
    );
    await tester.pumpAndSettle();

    // Expand the tile if not already expanded
    await tester.tap(find.text(l10n.toolExecuteCommand));
    await tester.pumpAndSettle();

    expect(find.text('git status'), findsAtLeast(1));
    expect(find.text('On branch main\nnothing to commit'), findsOneWidget);
    expect(find.text(l10n.toolTerminalExitCode(0)), findsOneWidget);
  });

  testWidgets('renders running indicator and input for send_terminal_input', (
    tester,
  ) async {
    const locale = Locale('tr');
    final l10n = await AppLocalizations.delegate.load(locale);

    final activity = ChatToolActivity(
      callId: 'call_term_2',
      name: 'send_terminal_input',
      status: ChatToolActivityStatus.running,
      arguments: const <String, Object?>{
        'terminal_id': 'term_interactive_1',
        'input': 'confirm_yes\n',
      },
      output: const <String, Object?>{
        'terminal_id': 'term_interactive_1',
        'output': 'Processing confirmation...',
        'is_running': true,
        'waiting_for_input': true,
      },
    );

    await tester.pumpWidget(
      buildTestableWidget(
        locale: locale,
        child: ToolActivityAccordion(
          activity: activity,
          palette: OpenChatPalette.light,
        ),
      ),
    );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));

    expect(find.text('terminal: term_interactive_1'), findsOneWidget);
    expect(find.text('confirm_yes'), findsOneWidget);
    expect(find.text('Processing confirmation...'), findsOneWidget);
    expect(find.text(l10n.toolTerminalWaitingForInput), findsOneWidget);
  });

  testWidgets('renders error exit code for failing commands', (tester) async {
    const locale = Locale('en');
    final l10n = await AppLocalizations.delegate.load(locale);

    final activity = ChatToolActivity(
      callId: 'call_term_3',
      name: 'bash',
      status: ChatToolActivityStatus.failed,
      arguments: const <String, Object?>{'command': 'cargo check'},
      output: const <String, Object?>{
        'command': 'cargo check',
        'output': 'error[E0425]: cannot find value',
        'exit_code': 101,
        'is_running': false,
      },
    );

    await tester.pumpWidget(
      buildTestableWidget(
        locale: locale,
        child: ToolActivityAccordion(
          activity: activity,
          palette: OpenChatPalette.dark,
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text(l10n.toolExecuteCommand));
    await tester.pumpAndSettle();

    expect(find.text('cargo check'), findsAtLeast(1));
    expect(find.text('error[E0425]: cannot find value'), findsOneWidget);
    expect(find.text(l10n.toolTerminalExitCode(101)), findsOneWidget);
  });

  testWidgets('renders terminated badge when session is killed', (
    tester,
  ) async {
    const locale = Locale('tr');
    final l10n = await AppLocalizations.delegate.load(locale);

    const data = ToolTerminalData(
      terminalId: 'term_killed_1',
      isTerminated: true,
      output: 'Session ended by user.',
    );

    await tester.pumpWidget(
      buildTestableWidget(
        locale: locale,
        child: const ToolTerminalResult(
          data: data,
          palette: OpenChatPalette.light,
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('terminal: term_killed_1'), findsOneWidget);
    expect(find.text(l10n.toolTerminalTerminated), findsOneWidget);
    expect(find.text('Session ended by user.'), findsOneWidget);
  });
}
