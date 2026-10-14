import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/history_storage_status.dart';
import 'package:openchat/features/settings/domain/support_diagnostics_bundle.dart';
import 'package:openchat/features/settings/presentation/support_diagnostics_section.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

class _FakeServiceClient extends OpenChatServiceClient {
  _FakeServiceClient(this.response);

  final Map<String, Object?> response;

  @override
  Future<Map<String, Object?>> call(
    String method, {
    Map<String, Object?> params = const <String, Object?>{},
    Duration timeout = const Duration(seconds: 15),
  }) async {
    expect(method, 'system.health');
    return response;
  }
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    final iconFont = FontLoader('MaterialIcons')
      ..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'));
    await iconFont.load();
  });

  test('support bundle serializes only reviewed diagnostic fields', () {
    final bundle = SupportDiagnosticsBundle.fromServiceHealth(
      value: <String, Object?>{
        'status': 'ready',
        'schema_version': 29,
        'database_path': r'C:\Users\private\chat.sqlite',
        'storage_root': r'C:\Users\private\OpenChat',
        'credential': 'must-not-be-exported',
        'prompt': 'must-not-be-exported',
      },
      generatedAt: DateTime.utc(2026, 10, 13),
      platform: 'windows',
      runtimeVersion: '3.13.1',
      historyStorageStatus: 'available',
    );

    final encoded = bundle.encode();
    expect(encoded, contains('"application": "OpenChat"'));
    expect(encoded, contains('"databaseSchemaVersion": 29'));
    expect(encoded, contains('"historyStatus": "available"'));
    expect(encoded, isNot(contains('database_path')));
    expect(encoded, isNot(contains('storage_root')));
    expect(encoded, isNot(contains('C:\\Users')));
    expect(encoded, isNot(contains('must-not-be-exported')));
  });

  test('support bundle rejects unhealthy and malformed service responses', () {
    for (final response in <Object?>[
      const <String, Object?>{'status': 'starting', 'schema_version': 29},
      const <String, Object?>{'status': 'ready', 'schema_version': '29'},
      const <String, Object?>{'status': 'ready', 'schema_version': 101},
      null,
    ]) {
      expect(
        () => SupportDiagnosticsBundle.fromServiceHealth(
          value: response,
          generatedAt: DateTime.utc(2026, 10, 13),
          platform: 'windows',
          runtimeVersion: '3.13.1',
          historyStorageStatus: 'available',
        ),
        throwsFormatException,
      );
    }
  });

  testWidgets('user reviews the exact support bundle before export', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1000, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    String? exportedContents;

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        home: Scaffold(
          body: Padding(
            padding: const EdgeInsets.all(24),
            child: SupportDiagnosticsSection(
              serviceClient: _FakeServiceClient(<String, Object?>{
                'status': 'ready',
                'schema_version': 29,
                'database_path': r'C:\private\chat.sqlite',
                'private': 'exclude this',
              }),
              historyStorageStatus: HistoryStorageStatus.available,
              exporter: (contents) async {
                exportedContents = contents;
                return true;
              },
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('Review export'));
    await tester.pumpAndSettle();
    expect(find.text('Review support diagnostics'), findsOneWidget);
    final preview = tester.widget<SelectableText>(find.byType(SelectableText));
    expect(preview.data, contains('"databaseSchemaVersion": 29'));
    expect(preview.data, isNot(contains('private')));
    expect(preview.data, isNot(contains('chat.sqlite')));
    expect(exportedContents, isNull);

    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    tester.view.physicalSize = const Size(420, 800);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Review export'));
    await tester.pumpAndSettle();
    expect(find.text('Review support diagnostics'), findsOneWidget);
    final narrowPreview = tester.widget<SelectableText>(
      find.byType(SelectableText),
    );
    expect(tester.takeException(), isNull);

    await tester.tap(find.text('Save JSON'));
    await tester.pumpAndSettle();
    expect(exportedContents, narrowPreview.data);
    expect(find.text('Support diagnostics saved.'), findsOneWidget);
    await tester.pump(const Duration(seconds: 5));
    expect(tester.takeException(), isNull);
  });

  testWidgets('service health failure shows recovery and does not export', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        home: Scaffold(
          body: SupportDiagnosticsSection(
            serviceClient: _FakeServiceClient(const <String, Object?>{
              'status': 'not-ready',
              'schema_version': 29,
            }),
            historyStorageStatus: HistoryStorageStatus.available,
            exporter: (_) async => fail('Invalid diagnostics must not export.'),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('Review export'));
    await tester.pumpAndSettle();

    expect(
      find.text('The local service returned unexpected diagnostic data.'),
      findsOneWidget,
    );
    expect(find.text('Review support diagnostics'), findsNothing);
    expect(find.text('Retry'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
}
