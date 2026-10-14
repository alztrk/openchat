import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/settings/data/personal_memory_fact.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/features/settings/presentation/personal_memory_settings_section.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:shared_preferences_platform_interface/in_memory_shared_preferences_async.dart';
import 'package:shared_preferences_platform_interface/shared_preferences_async_platform_interface.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUp(() {
    SharedPreferencesAsyncPlatform.instance =
        InMemorySharedPreferencesAsync.empty();
  });

  tearDown(() {
    SharedPreferencesAsyncPlatform.instance = null;
  });

  testWidgets('pauses, edits, and removes user-entered memories', (
    tester,
  ) async {
    final preferences = SettingsPreferences(SharedPreferencesAsync());
    final fact = PersonalMemoryFact.create('Prefers concise answers');
    await preferences.writePersonalMemoryFacts(<PersonalMemoryFact>[fact]);

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        home: Scaffold(
          body: SingleChildScrollView(
            child: PersonalMemorySettingsSection(
              settingsPreferences: preferences,
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Prefers concise answers'), findsOneWidget);
    expect(await preferences.readPersonalMemoryEnabled(), isFalse);

    await tester.tap(find.byType(SwitchListTile));
    await tester.pumpAndSettle();
    expect(await preferences.readPersonalMemoryEnabled(), isTrue);

    await tester.tap(find.byTooltip('Edit fact'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField), 'Prefers short answers');
    await tester.tap(find.widgetWithText(FilledButton, 'Save'));
    await tester.pumpAndSettle();

    expect(find.text('Prefers short answers'), findsOneWidget);
    expect(
      (await preferences.readPersonalMemoryFacts()).single.id,
      isNot(fact.id),
    );
    expect(
      (await preferences.readPersonalMemoryFacts()).single.content,
      'Prefers short answers',
    );

    await tester.tap(find.byTooltip('Delete fact'));
    await tester.pumpAndSettle();
    expect(find.text('Delete this saved fact?'), findsOneWidget);
    await tester.tap(find.widgetWithText(FilledButton, 'Delete'));
    await tester.pumpAndSettle();
    expect(find.text('You have not saved any facts.'), findsOneWidget);
    expect(await preferences.readPersonalMemoryFacts(), isEmpty);
  });

  testWidgets('offers a confirmed recovery action for unreadable memory', (
    tester,
  ) async {
    final previousErrorHandler = FlutterError.onError;
    final reportedErrors = <FlutterErrorDetails>[];
    addTearDown(() => FlutterError.onError = previousErrorHandler);
    FlutterError.onError = reportedErrors.add;

    final asyncPreferences = SharedPreferencesAsync();
    await asyncPreferences.setBool('privacy.personal_memory_enabled', true);
    await asyncPreferences.setString(
      'privacy.personal_memory_facts',
      'unreadable',
    );
    final preferences = SettingsPreferences(asyncPreferences);

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        home: Scaffold(
          body: SingleChildScrollView(
            child: PersonalMemorySettingsSection(
              settingsPreferences: preferences,
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Saved facts could not be loaded.'), findsOneWidget);
    await tester.tap(find.text('Clear saved memories'));
    await tester.pumpAndSettle();
    expect(find.text('Clear all saved memories?'), findsOneWidget);
    await tester.tap(find.widgetWithText(FilledButton, 'Clear memories'));
    await tester.pumpAndSettle();

    expect(await preferences.readPersonalMemoryEnabled(), isFalse);
    expect(await preferences.readPersonalMemoryFacts(), isEmpty);
    expect(find.text('Saved facts could not be loaded.'), findsNothing);
    expect(reportedErrors, hasLength(1));
  });
}
