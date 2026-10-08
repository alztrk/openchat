import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/presentation/widgets/agent_run_manager_dialog.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

void main() {
  testWidgets(
    'run manager stays readable and labelled across locales/scaling',
    (tester) async {
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final semantics = tester.ensureSemantics();
      try {
        for (final locale in AppLocalizations.supportedLocales) {
          final l10n = await AppLocalizations.delegate.load(locale);
          for (final width in <double>[480, 1200]) {
            for (final scale in <double>[1, 1.5, 2]) {
              for (final ratio in <double>[1, 1.25, 1.5, 2]) {
                tester.view.devicePixelRatio = ratio;
                tester.view.physicalSize = Size(width * ratio, 900 * ratio);
                final service = _FakeRunServiceClient(<Map<String, Object?>>[
                  <String, Object?>{
                    'runId': 'run-1',
                    'conversationId': 'conversation-1',
                    'conversationTitle': 'A long conversation title for an interrupted project run',
                    'status': 'interrupted',
                    'updatedAtUnixMs': 0,
                    'providerId': 'gemini',
                    'modelId': 'gemini-model',
                  },
                ]);
                await tester.pumpWidget(
                  MaterialApp(
                    key: ValueKey<String>('$locale-$width-$scale-$ratio'),
                    locale: locale,
                    localizationsDelegates:
                        AppLocalizations.localizationsDelegates,
                    supportedLocales: AppLocalizations.supportedLocales,
                    theme: OpenChatTheme.dark,
                    home: MediaQuery(
                      data: MediaQueryData(
                        size: Size(width, 900),
                        devicePixelRatio: ratio,
                        textScaler: TextScaler.linear(scale),
                        disableAnimations: true,
                      ),
                      child: Scaffold(
                        body: AgentRunManagerDialog(
                          serviceClient: service,
                          onOpenConversation: (_) {},
                        ),
                      ),
                    ),
                  ),
                );
                await tester.pumpAndSettle();

                expect(find.text(l10n.agentRunManagerTitle), findsOneWidget);
                expect(
                  find.text(l10n.agentRunStatusInterrupted),
                  findsOneWidget,
                );
                expect(
                  find.bySemanticsLabel(l10n.agentRunOpenConversation),
                  findsOneWidget,
                );
                expect(service.calls, <String>['chat.runs.list']);
                expect(tester.takeException(), isNull);
              }
            }
          }
        }
      } finally {
        semantics.dispose();
      }
    },
  );

  testWidgets('opening a run returns to its conversation', (tester) async {
    final service = _FakeRunServiceClient(<Map<String, Object?>>[
      <String, Object?>{
        'runId': 'run-1',
        'conversationId': 'conversation-1',
        'conversationTitle': 'Build feature',
        'status': 'paused',
        'updatedAtUnixMs': 0,
        'providerId': null,
        'modelId': null,
      },
    ]);
    String? openedConversationId;
    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        home: Scaffold(
          body: AgentRunManagerDialog(
            serviceClient: service,
            onOpenConversation: (conversationId) {
              openedConversationId = conversationId;
            },
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('Open chat'));
    await tester.pumpAndSettle();

    expect(openedConversationId, 'conversation-1');
    expect(find.byType(AgentRunManagerDialog), findsNothing);
  });

  testWidgets('invalid run summaries show a recoverable error', (tester) async {
    final service = _FakeRunServiceClient(<Map<String, Object?>>[
      <String, Object?>{'runId': 'malformed'},
    ]);
    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('tr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark,
        home: Scaffold(
          body: AgentRunManagerDialog(
            serviceClient: service,
            onOpenConversation: (_) {},
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Çalışma durumu yüklenemedi.'), findsOneWidget);
    expect(find.text('Çalışma listesini yenile'), findsOneWidget);
  });

  testWidgets('dialog closes from the keyboard', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark,
        home: Scaffold(
          body: Builder(
            builder: (context) => TextButton(
              onPressed: () => showDialog<void>(
                context: context,
                builder: (context) => AgentRunManagerDialog(
                  serviceClient: _FakeRunServiceClient(
                    const <Map<String, Object?>>[],
                  ),
                  onOpenConversation: (_) {},
                ),
              ),
              child: const Text('Open run manager'),
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.text('Open run manager'));
    await tester.pumpAndSettle();

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();

    expect(find.byType(AgentRunManagerDialog), findsNothing);
  });
}

class _FakeRunServiceClient extends OpenChatServiceClient {
  _FakeRunServiceClient(this.runs);

  final List<Map<String, Object?>> runs;
  final List<String> calls = <String>[];

  @override
  Future<Map<String, Object?>> call(
    String method, {
    Map<String, Object?> params = const <String, Object?>{},
    Duration timeout = const Duration(seconds: 15),
  }) async {
    calls.add(method);
    if (method != 'chat.runs.list') {
      throw StateError('Unexpected service method: $method');
    }
    return <String, Object?>{'runs': runs};
  }
}
