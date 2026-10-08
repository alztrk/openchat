import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/presentation/widgets/project_worktrees_dialog.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

void main() {
  testWidgets(
    'worktree review actions remain accessible across locales and scaling',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.view.resetPhysicalSize);
      final semantics = tester.ensureSemantics();
      try {
        for (final locale in AppLocalizations.supportedLocales) {
          final l10n = await AppLocalizations.delegate.load(locale);
          for (final width in <double>[480, 1200]) {
            for (final scale in <double>[1, 1.5, 2]) {
              tester.view.physicalSize = Size(width, 900);
              final service = _FakeWorktreeServiceClient();
              await tester.pumpWidget(
                MaterialApp(
                  locale: locale,
                  localizationsDelegates:
                      AppLocalizations.localizationsDelegates,
                  supportedLocales: AppLocalizations.supportedLocales,
                  theme: OpenChatTheme.dark,
                  home: MediaQuery(
                    data: MediaQueryData(
                      size: Size(width, 900),
                      textScaler: TextScaler.linear(scale),
                    ),
                    child: Scaffold(
                      body: ProjectWorktreesDialog(
                        key: ValueKey<String>('$locale-$width-$scale'),
                        serviceClient: service,
                        projectId: 'project-1',
                        projectRoot: r'D:\projects\OpenChat',
                        permissionMode: ToolPermissionMode.requireApproval,
                        projectPermissionRules:
                            const <String, ToolPermissionRule>{},
                        onUseWorktree: (_, _) async {},
                      ),
                    ),
                  ),
                ),
              );
              await tester.pumpAndSettle();

              expect(find.text(l10n.projectWorktreesTitle), findsOneWidget);
              expect(service.calls, contains('project.worktrees.list'));
              expect(find.byType(Card), findsOneWidget);
              expect(
                find.bySemanticsLabel(l10n.projectWorktreeCreate),
                findsOneWidget,
              );
              expect(find.text(l10n.projectWorktreeReview), findsOneWidget);
              expect(tester.takeException(), isNull);
            }
          }
        }
      } finally {
        semantics.dispose();
      }
    },
  );

  testWidgets('named checks show the exact command before confirmation', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(480, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    for (final locale in AppLocalizations.supportedLocales) {
      final l10n = await AppLocalizations.delegate.load(locale);
      final service = _FakeWorktreeServiceClient();
      await tester.pumpWidget(
        MaterialApp(
          locale: locale,
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          theme: OpenChatTheme.dark,
          home: MediaQuery(
            data: const MediaQueryData(size: Size(480, 900)),
            child: Scaffold(
              body: ProjectWorktreesDialog(
                serviceClient: service,
                projectId: 'project-1',
                projectRoot: r'D:\projects\OpenChat',
                permissionMode: ToolPermissionMode.requireApproval,
                projectPermissionRules: const <String, ToolPermissionRule>{},
                onUseWorktree: (_, _) async {},
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text(l10n.projectWorktreeRunCheck));
      await tester.pumpAndSettle();

      expect(service.calls, contains('project.worktrees.tasks'));
      expect(find.text(l10n.projectWorktreeTaskPickerTitle), findsOneWidget);
      expect(find.text('verify'), findsOneWidget);
      await tester.tap(
        find.byKey(const ValueKey<String>('project-worktree-task-run')),
      );
      await tester.pumpAndSettle();

      expect(
        find.text(l10n.projectWorktreeTaskConfirmationTitle),
        findsOneWidget,
      );
      expect(find.text('echo verify'), findsOneWidget);
      expect(find.text(l10n.projectWorktreeTaskTimeout(30)), findsOneWidget);
      expect(service.calls, isNot(contains('project.worktrees.run_task')));
      await tester.tap(
        find.byKey(const ValueKey<String>('project-worktree-task-cancel')),
      );
      await tester.pumpAndSettle();
    }
  });
}

class _FakeWorktreeServiceClient extends OpenChatServiceClient {
  final List<String> calls = <String>[];

  @override
  Future<Map<String, Object?>> call(
    String method, {
    Map<String, Object?> params = const <String, Object?>{},
    Duration timeout = const Duration(seconds: 15),
  }) async {
    calls.add(method);
    if (method == 'project.worktrees.tasks') {
      return <String, Object?>{
        'tasks': <Map<String, Object?>>[
          <String, Object?>{
            'id': 'verify',
            'command': 'echo verify',
            'timeoutSeconds': 30,
          },
        ],
      };
    }
    if (method == 'project.worktrees.list') {
      return <String, Object?>{
        'worktrees': <Map<String, Object?>>[
          <String, Object?>{
            'id': '123e4567e89b12d3a456426614174000',
            'branch': 'openchat/123e4567e89b12d3a456426614174000',
            'path': r'D:\Users\User\AppData\Local\OpenChat\worktrees\project',
            'status': <String, Object?>{'files': <Object?>[]},
          },
        ],
        'truncated': false,
      };
    }
    throw StateError('Unexpected service method: $method');
  }
}
