import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/features/chat/presentation/widgets/project_index_dialog.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

void main() {
  testWidgets('project index can be enabled, synchronized, and cleared', (
    tester,
  ) async {
    final service = _FakeProjectIndexService();
    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: Scaffold(
          body: ProjectIndexDialog(
            serviceClient: service,
            projectId: 'project-1',
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.textContaining('Files: 1'), findsOneWidget);
    await tester.tap(find.byType(SwitchListTile));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Sync'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Clear index'));
    await tester.pumpAndSettle();
    expect(service.calls, [
      'project.index.get',
      'project.index.set_enabled',
      'project.index.sync',
      'project.index.clear',
    ]);
    expect(tester.takeException(), isNull);
  });
}

class _FakeProjectIndexService extends OpenChatServiceClient {
  final List<String> calls = [];
  bool enabled = false;
  String status = 'disabled';

  @override
  Future<Map<String, Object?>> call(
    String method, {
    Map<String, Object?> params = const {},
    Duration timeout = const Duration(seconds: 15),
  }) async {
    calls.add(method);
    if (method == 'project.index.set_enabled') {
      enabled = params['enabled'] == true;
      status = enabled ? 'empty' : 'disabled';
    } else if (method == 'project.index.sync') {
      status = 'current';
    } else if (method == 'project.index.clear') {
      enabled = false;
      status = 'empty';
    }
    return {
      'index': {
        'enabled': enabled,
        'status': status,
        'indexedFiles': 1,
        'indexedBytes': 128,
      },
    };
  }
}
