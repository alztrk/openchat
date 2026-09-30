import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/features/settings/presentation/models_settings_section.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:shared_preferences_platform_interface/in_memory_shared_preferences_async.dart';
import 'package:shared_preferences_platform_interface/shared_preferences_async_platform_interface.dart';

class _FakeServiceClient extends OpenChatServiceClient {
  _FakeServiceClient(this.handler);

  final Future<Map<String, Object?>> Function(String method, Map<String, Object?> params) handler;

  @override
  Future<Map<String, Object?>> call(
    String method, {
    Map<String, Object?> params = const <String, Object?>{},
  }) =>
      handler(method, params);
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

  testWidgets('ModelsSettingsSection lists models, sets default, and toggles hide', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1000, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final preferences = SettingsPreferences(SharedPreferencesAsync());
    final fakeService = _FakeServiceClient((method, params) async {
      if (method == 'opencode.models.list') {
        return {
          'freshness': 'current',
          'models': [
            {
              'id': 'big-pickle',
              'displayName': 'Big Pickle',
              'groupId': 'free',
              'isAvailable': true,
              'reasoningLevels': <String>[],
              'contextWindow': 200000,
            },
            {
              'id': 'space-bunny-free',
              'displayName': 'Space Bunny Free',
              'groupId': 'free',
              'isAvailable': true,
              'reasoningLevels': <String>[],
              'contextWindow': 128000,
            },
          ],
        };
      }
      if (method == 'chatgpt.connections.list') {
        return {'connections': <Object?>[]};
      }
      return {};
    });

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('tr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        home: Scaffold(
          body: SingleChildScrollView(
            child: ModelsSettingsSection(
              serviceClient: fakeService,
              chatGptApiKeyStore: null,
              apiCompatibleProviderKeyStore: null,
              openCodeApiKeyStore: null,
              settingsPreferences: preferences,
            ),
          ),
        ),
      ),
    );

    await tester.pumpAndSettle();

    // Verify OpenCode models appear
    expect(find.text('Big Pickle'), findsOneWidget);
    expect(find.text('Space Bunny Free'), findsOneWidget);
    expect(find.text('200k context'), findsOneWidget);

    // Set Big Pickle as default
    final setDefaultButtons = find.text('Varsayılan Yap');
    expect(setDefaultButtons, findsNWidgets(2));
    await tester.tap(setDefaultButtons.first);
    await tester.pumpAndSettle();

    // Verify default badge appeared
    expect(find.text('Varsayılan'), findsAtLeastNWidgets(1));
    final defaultModel = await preferences.readDefaultModel();
    expect(defaultModel?.modelId, 'big-pickle');

    // Hide Space Bunny Free
    final hideButtons = find.byIcon(Icons.visibility_outlined);
    expect(hideButtons, findsNWidgets(2));
    await tester.tap(hideButtons.last);
    await tester.pumpAndSettle();

    expect(find.text('Gizli'), findsOneWidget);
    final hiddenKeys = await preferences.readHiddenModelKeys();
    expect(hiddenKeys, contains('opencode:::space-bunny-free'));

    // Search filter
    await tester.enterText(find.byType(TextField), 'pickle');
    await tester.pumpAndSettle();
    expect(find.text('Big Pickle'), findsOneWidget);
    expect(find.text('Space Bunny Free'), findsNothing);

    // Consume any active toastification timers
    await tester.pump(const Duration(seconds: 5));
    await tester.pumpAndSettle();
  });
}
