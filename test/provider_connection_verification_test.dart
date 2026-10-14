import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/settings/data/api_compatible_provider_key_store.dart';
import 'package:openchat/features/settings/data/open_code_api_key_store.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/features/settings/domain/provider_verification_record.dart';
import 'package:openchat/features/settings/domain/provider_request_outcome_record.dart';
import 'package:openchat/features/settings/presentation/compatible_provider_connection_section.dart';
import 'package:openchat/features/settings/presentation/open_code_connection_section.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:shared_preferences_platform_interface/in_memory_shared_preferences_async.dart';
import 'package:shared_preferences_platform_interface/shared_preferences_async_platform_interface.dart';

class _FakeKeyStore extends ApiCompatibleProviderKeyStore {
  _FakeKeyStore() : super(const FlutterSecureStorage());

  @override
  Future<ApiCompatibleProviderKeyStatus> readStatus(String providerId) async =>
      const ApiCompatibleProviderKeyStatus(
        isConfigured: true,
        keySuffix: '1234',
      );

  @override
  Future<String?> readApiKey(String providerId) async => 'test-only-key';

  @override
  Future<List<CustomCompatibleProvider>> readCustomProviders() async =>
      const <CustomCompatibleProvider>[];
}

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

class _FakeOpenCodeKeyStore extends OpenCodeApiKeyStore {
  _FakeOpenCodeKeyStore() : super(const FlutterSecureStorage());

  @override
  Future<String?> readApiKey() async => 'test-only-opencode-key';

  @override
  Future<String?> readKeySuffix() async => '1234';
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    final iconFont = FontLoader('MaterialIcons')
      ..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'));
    await iconFont.load();
  });

  setUp(() {
    SharedPreferencesAsyncPlatform.instance =
        InMemorySharedPreferencesAsync.empty();
  });

  tearDown(() {
    SharedPreferencesAsyncPlatform.instance = null;
  });

  Future<void> pumpSection(
    WidgetTester tester,
    OpenChatServiceClient serviceClient,
    SettingsPreferences preferences,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        home: Scaffold(
          body: SingleChildScrollView(
            child: CompatibleProviderConnectionSection(
              providerId: 'groq',
              apiKeyStore: _FakeKeyStore(),
              settingsPreferences: preferences,
              serviceClient: serviceClient,
              onChanged: null,
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
  }

  testWidgets('saved key stays unverified until catalog check succeeds', (
    tester,
  ) async {
    var callCount = 0;
    final preferences = SettingsPreferences(SharedPreferencesAsync());
    final service = _FakeServiceClient((method, params) async {
      callCount++;
      expect(method, 'compatible.models.list');
      expect(params['providerId'], 'groq');
      expect(params['apiKey'], 'test-only-key');
      expect(params.containsKey('baseUrl'), isFalse);
      expect(params.containsKey('protocol'), isFalse);
      return <String, Object?>{
        'freshness': 'current',
        'models': <Object?>[
          <String, Object?>{'id': 'model-1'},
        ],
      };
    });
    await pumpSection(tester, service, preferences);

    expect(
      find.text('API key ending in ••••1234 is saved securely on this device.'),
      findsOneWidget,
    );
    await tester.tap(find.text('Verify connection'));
    await tester.pumpAndSettle();

    expect(callCount, 1);
    expect(
      find.text('Connection verified; model catalog is available.'),
      findsOneWidget,
    );
    expect(
      find.text(
        'OpenChat does not currently show quota data for this provider.',
      ),
      findsOneWidget,
    );
    final savedRecord = await preferences.readProviderVerificationRecord(
      'groq',
    );
    expect(savedRecord?.outcome, ProviderVerificationOutcome.verified);
  });

  testWidgets('authentication failure provides a clear recovery state', (
    tester,
  ) async {
    final service = _FakeServiceClient((_, _) async {
      throw const OpenChatServiceException(
        code: 'authentication_required',
        message: 'The provider rejected the key.',
      );
    });
    await pumpSection(
      tester,
      service,
      SettingsPreferences(SharedPreferencesAsync()),
    );

    await tester.tap(find.text('Verify connection'));
    await tester.pumpAndSettle();

    expect(
      find.text('The provider rejected this key. Check it and try again.'),
      findsOneWidget,
    );
    expect(find.text('Verify connection'), findsOneWidget);
  });

  testWidgets('stale compatible catalog is not treated as verified', (
    tester,
  ) async {
    final service = _FakeServiceClient((_, _) async {
      return <String, Object?>{
        'freshness': 'stale',
        'models': <Object?>[<String, Object?>{}],
      };
    });
    await pumpSection(
      tester,
      service,
      SettingsPreferences(SharedPreferencesAsync()),
    );

    await tester.tap(find.text('Verify connection'));
    await tester.pumpAndSettle();

    expect(
      find.text(
        'Only cached models were returned; the provider connection could not be verified. Try again.',
      ),
      findsOneWidget,
    );
    expect(
      find.text('Connection verified; model catalog is available.'),
      findsNothing,
    );
  });

  testWidgets(
    'restores the last provider verification after reopening settings',
    (tester) async {
      final preferences = SettingsPreferences(SharedPreferencesAsync());
      final checkedAt = DateTime.utc(2026, 10, 13, 9, 30);
      await preferences.writeProviderVerificationRecord(
        'groq',
        ProviderVerificationRecord(
          outcome: ProviderVerificationOutcome.verified,
          checkedAt: checkedAt,
        ),
      );
      final service = _FakeServiceClient((_, _) async => <String, Object?>{});

      await pumpSection(tester, service, preferences);

      expect(
        find.text('Connection verified; model catalog is available.'),
        findsOneWidget,
      );
      expect(find.textContaining('Last checked:'), findsOneWidget);
      expect(find.text('Verify connection'), findsOneWidget);
    },
  );

  testWidgets('shows the last request outcome on the provider connection', (
    tester,
  ) async {
    final preferences = SettingsPreferences(SharedPreferencesAsync());
    await preferences.writeProviderRequestOutcomeForRoute(
      providerId: 'groq',
      connectionId: 'groq',
      record: ProviderRequestOutcomeRecord(
        outcome: ProviderRequestOutcome.completed,
        occurredAt: DateTime.utc(2026, 10, 13, 9, 30),
      ),
    );

    await pumpSection(
      tester,
      _FakeServiceClient((_, _) async => <String, Object?>{}),
      preferences,
    );

    expect(find.text('Last request: Completed'), findsOneWidget);
    expect(find.textContaining('Request time:'), findsOneWidget);
  });

  testWidgets('OpenCode verification bypasses cached catalog results', (
    tester,
  ) async {
    var callCount = 0;
    final preferences = SettingsPreferences(SharedPreferencesAsync());
    final service = _FakeServiceClient((method, params) async {
      callCount++;
      expect(method, 'opencode.models.list');
      expect(params['apiKey'], 'test-only-opencode-key');
      expect(params['forceRefresh'], isTrue);
      return <String, Object?>{
        'freshness': 'current',
        'models': <Object?>[<String, Object?>{}],
      };
    });
    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        home: Scaffold(
          body: SingleChildScrollView(
            child: OpenCodeConnectionSection(
              apiKeyStore: _FakeOpenCodeKeyStore(),
              settingsPreferences: preferences,
              serviceClient: service,
              onChanged: null,
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(
      find.text(
        'Console API key ending in ••••1234 is saved securely on this device.',
      ),
      findsOneWidget,
    );
    await tester.tap(find.text('Verify connection'));
    await tester.pumpAndSettle();

    expect(callCount, 1);
    expect(
      find.text('Connection verified; model catalog is available.'),
      findsOneWidget,
    );
    expect(
      (await preferences.readProviderVerificationRecord('opencode'))?.outcome,
      ProviderVerificationOutcome.verified,
    );
  });

  testWidgets('stale catalog cannot be reported as a verified connection', (
    tester,
  ) async {
    final service = _FakeServiceClient((_, _) async {
      return <String, Object?>{
        'freshness': 'stale',
        'models': <Object?>[<String, Object?>{}],
      };
    });
    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        home: Scaffold(
          body: SingleChildScrollView(
            child: OpenCodeConnectionSection(
              apiKeyStore: _FakeOpenCodeKeyStore(),
              settingsPreferences: SettingsPreferences(
                SharedPreferencesAsync(),
              ),
              serviceClient: service,
              onChanged: null,
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('Verify connection'));
    await tester.pumpAndSettle();

    expect(
      find.text(
        'Only cached models were returned; the provider connection could not be verified. Try again.',
      ),
      findsOneWidget,
    );
    expect(
      find.text('Connection verified; model catalog is available.'),
      findsNothing,
    );
  });
}
