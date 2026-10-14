import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/settings/data/chat_gpt_api_key_store.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:shared_preferences_platform_interface/in_memory_shared_preferences_async.dart';
import 'package:shared_preferences_platform_interface/shared_preferences_async_platform_interface.dart';
import 'package:openchat/features/settings/presentation/chat_gpt_connection_section.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

class _FakeServiceClient extends OpenChatServiceClient {
  _FakeServiceClient(this.handler);

  final Future<Map<String, Object?>> Function(
    String method,
    Map<String, Object?> params,
  )
  handler;

  @override
  Future<Map<String, Object?>> call(
    String method, {
    Map<String, Object?> params = const <String, Object?>{},
    Duration timeout = const Duration(seconds: 15),
  }) => handler(method, params);
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUp(() {
    SharedPreferencesAsyncPlatform.instance =
        InMemorySharedPreferencesAsync.empty();
  });

  tearDown(() {
    SharedPreferencesAsyncPlatform.instance = null;
  });

  setUpAll(() async {
    final iconFont = FontLoader('MaterialIcons')
      ..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'));
    await iconFont.load();
  });

  testWidgets('OAuth verification checks the selected account and workspace', (
    tester,
  ) async {
    final preferences = SettingsPreferences(SharedPreferencesAsync());
    final calls = <(String, Map<String, Object?>)>[];
    final service = _FakeServiceClient((method, params) async {
      calls.add((method, params));
      if (method == 'chatgpt.connections.list') {
        return <String, Object?>{
          'connections': <Object?>[
            <String, Object?>{
              'id': 'oauth-connection',
              'authStatus': 'active',
              'isSelected': true,
              'workspaces': <Object?>[
                <String, Object?>{
                  'id': 'oauth-workspace',
                  'externalId': 'workspace-external',
                  'isSelected': true,
                },
              ],
            },
          ],
        };
      }
      if (method == 'chatgpt.models.list') {
        return <String, Object?>{
          'freshness': 'current',
          'models': <Object?>[
            <String, Object?>{'id': 'gpt-model'},
          ],
        };
      }
      throw StateError('Unexpected service call: $method');
    });

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        home: Scaffold(
          body: SingleChildScrollView(
            child: ChatGptConnectionSection(
              apiKeyStore: null,
              settingsPreferences: preferences,
              serviceClient: service,
              onProviderStateChanged: null,
              onConnectionRemoved: null,
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Connection has not been verified yet.'), findsOneWidget);
    expect(find.text('Verify connection'), findsOneWidget);
    await tester.tap(find.text('Verify connection'));
    await tester.pumpAndSettle();

    expect(calls, hasLength(2));
    expect(calls.last.$1, 'chatgpt.models.list');
    expect(calls.last.$2['connectionId'], 'oauth-connection');
    expect(calls.last.$2['workspaceId'], 'oauth-workspace');
    expect(calls.last.$2['forceRefresh'], isTrue);
    expect(
      find.text('Connection verified; model catalog is available.'),
      findsOneWidget,
    );
    expect(
      (await preferences.readProviderVerificationRecordForRoute(
        providerId: 'chatgpt_oauth',
        connectionId: 'oauth-connection',
        workspaceId: 'oauth-workspace',
      ))?.outcome.name,
      'verified',
    );

    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        home: Scaffold(
          body: SingleChildScrollView(
            child: ChatGptConnectionSection(
              apiKeyStore: null,
              settingsPreferences: preferences,
              serviceClient: service,
              onProviderStateChanged: null,
              onConnectionRemoved: null,
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(
      find.text('Connection verified; model catalog is available.'),
      findsOneWidget,
    );
    expect(find.textContaining('Last checked'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'API-key verification refreshes the catalog without exposing key',
    (tester) async {
      final preferences = SettingsPreferences(SharedPreferencesAsync());
      FlutterSecureStorage.setMockInitialValues({});
      const apiKey = 'sk-test-only-verification-key';
      final store = ChatGptApiKeyStore(const FlutterSecureStorage());
      final connections = await store.saveApiKey(apiKey);
      final calls = <(String, Map<String, Object?>)>[];
      final service = _FakeServiceClient((method, params) async {
        calls.add((method, params));
        if (method == 'chatgpt.connections.list') {
          return <String, Object?>{'connections': <Object?>[]};
        }
        if (method == 'chatgpt.api.models.list') {
          return <String, Object?>{
            'freshness': 'current',
            'models': <Object?>[
              <String, Object?>{'id': 'gpt-model'},
            ],
          };
        }
        throw StateError('Unexpected service call: $method');
      });

      await tester.pumpWidget(
        MaterialApp(
          locale: const Locale('en'),
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          theme: OpenChatTheme.light,
          home: Scaffold(
            body: SingleChildScrollView(
              child: ChatGptConnectionSection(
                apiKeyStore: store,
                settingsPreferences: preferences,
                serviceClient: service,
                onProviderStateChanged: null,
                onConnectionRemoved: null,
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('Verify connection'), findsOneWidget);
      await tester.tap(find.text('Verify connection'));
      await tester.pumpAndSettle();

      expect(calls.last.$1, 'chatgpt.api.models.list');
      expect(calls.last.$2['apiKey'], apiKey);
      expect(calls.last.$2['forceRefresh'], isTrue);
      expect(
        find.text('Connection verified; model catalog is available.'),
        findsOneWidget,
      );
      expect(find.text(apiKey), findsNothing);
      expect(
        (await preferences.readProviderVerificationRecordForRoute(
          providerId: 'chatgpt_api',
          connectionId: connections.single.id,
        ))?.outcome.name,
        'verified',
      );
      expect(tester.takeException(), isNull);
      expect(connections, hasLength(1));
    },
  );
}
