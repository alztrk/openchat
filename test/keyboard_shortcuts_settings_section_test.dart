import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/features/settings/presentation/keyboard_shortcuts_settings_section.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:shared_preferences_platform_interface/in_memory_shared_preferences_async.dart';
import 'package:shared_preferences_platform_interface/shared_preferences_async_platform_interface.dart';

import 'support/shad_test_scope.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUp(() {
    SharedPreferencesAsyncPlatform.instance =
        InMemorySharedPreferencesAsync.empty();
  });

  tearDown(() {
    SharedPreferencesAsyncPlatform.instance = null;
  });

  testWidgets('search filters the supported composer shortcuts', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        builder: openChatShadTestBuilder,
        home: Scaffold(
          body: KeyboardShortcutsSettingsSection(
            settingsPreferences: SettingsPreferences(SharedPreferencesAsync()),
          ),
        ),
      ),
    );

    expect(find.text('Send message'), findsOneWidget);
    expect(find.text('Insert a new line'), findsOneWidget);
    expect(find.text('Stop response'), findsOneWidget);
    expect(find.text('Enter'), findsOneWidget);
    expect(find.text('Shift + Enter'), findsOneWidget);
    expect(find.text('Esc'), findsOneWidget);

    await tester.enterText(
      find.byKey(const ValueKey<String>('keyboard-shortcuts-search')),
      'new line',
    );
    await tester.pumpAndSettle();

    expect(find.text('Insert a new line'), findsOneWidget);
    expect(find.text('Send message'), findsNothing);
    expect(find.text('Stop response'), findsNothing);
  });

  testWidgets('shortcut rows stack at narrow width and large text scale', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        builder: openChatShadTestBuilder,
        home: Scaffold(
          body: MediaQuery(
            data: const MediaQueryData(textScaler: TextScaler.linear(2)),
            child: SizedBox(
              width: 320,
              child: SingleChildScrollView(
                child: KeyboardShortcutsSettingsSection(
                  settingsPreferences: SettingsPreferences(
                    SharedPreferencesAsync(),
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Shift + Enter'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
}
