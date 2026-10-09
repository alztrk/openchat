import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/presentation/widgets/project_skills_dialog.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';
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

  testWidgets('lists, previews, and saves selected project Skills', (
    tester,
  ) async {
    final service = _FakeProjectSkillsService();
    final preferences = SettingsPreferences(SharedPreferencesAsync());
    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        home: Scaffold(
          body: ProjectSkillsDialog(
            serviceClient: service,
            settingsPreferences: preferences,
            projectId: 'project-1',
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('rust-style'), findsOneWidget);
    await tester.tap(find.text('rust-style'));
    await tester.pumpAndSettle();
    expect(find.text('Use rustfmt for Rust changes.'), findsOneWidget);

    await tester.tap(find.byType(Checkbox).first);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Save instructions'));
    await tester.pumpAndSettle();

    expect(await preferences.readProjectSkillIds('project-1'), {'rust-style'});
    expect(service.calls, ['project.skills.list']);
  });

  testWidgets('stale saved Skills can be cleared', (tester) async {
    final service = _FakeProjectSkillsService(skills: const <Object?>[]);
    final preferences = SettingsPreferences(SharedPreferencesAsync());
    await preferences.writeProjectSkillIds('project-1', {'removed-skill'});

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        home: Scaffold(
          body: ProjectSkillsDialog(
            serviceClient: service,
            settingsPreferences: preferences,
            projectId: 'project-1',
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('removed-skill'), findsOneWidget);
    expect(find.textContaining('no longer available'), findsOneWidget);
    await tester.tap(find.byType(CheckboxListTile).first);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Save instructions'));
    await tester.pumpAndSettle();

    expect(await preferences.readProjectSkillIds('project-1'), isEmpty);
  });

  testWidgets('invalid catalogs can be recovered by clearing the selection', (
    tester,
  ) async {
    final service = _FakeProjectSkillsService()..fail = true;
    final preferences = SettingsPreferences(SharedPreferencesAsync());
    await preferences.writeProjectSkillIds('project-1', {'rust-style'});
    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        home: Scaffold(
          body: ProjectSkillsDialog(
            serviceClient: service,
            settingsPreferences: preferences,
            projectId: 'project-1',
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(
      find.text(
        'Project Skills could not be loaded. Check the project files and try again.',
      ),
      findsOneWidget,
    );
    await tester.tap(find.text('Clear selection'));
    await tester.pumpAndSettle();
    expect(await preferences.readProjectSkillIds('project-1'), isEmpty);
  });

  testWidgets('project Skills dialog fits supported locales and text scales', (
    tester,
  ) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    for (final locale in AppLocalizations.supportedLocales) {
      for (final width in <double>[320, 900]) {
        for (final scale in <double>[1, 2]) {
          final l10n = await AppLocalizations.delegate.load(locale);
          tester.view.physicalSize = Size(width, 720);
          tester.view.devicePixelRatio = 1;
          await tester.pumpWidget(
            MaterialApp(
              locale: locale,
              localizationsDelegates: AppLocalizations.localizationsDelegates,
              supportedLocales: AppLocalizations.supportedLocales,
              theme: OpenChatTheme.light,
              home: MediaQuery(
                data: MediaQueryData(
                  size: Size(width, 720),
                  textScaler: TextScaler.linear(scale),
                ),
                child: Scaffold(
                  body: ProjectSkillsDialog(
                    key: ValueKey<String>(
                      '${locale.languageCode}-$width-$scale',
                    ),
                    serviceClient: _FakeProjectSkillsService(),
                    settingsPreferences: SettingsPreferences(
                      SharedPreferencesAsync(),
                    ),
                    projectId: 'project-1',
                  ),
                ),
              ),
            ),
          );
          await tester.pumpAndSettle();
          expect(find.text(l10n.projectSkillsTitle), findsOneWidget);
          expect(tester.takeException(), isNull);
        }
      }
    }
  });
}

class _FakeProjectSkillsService extends OpenChatServiceClient {
  _FakeProjectSkillsService({
    this.skills = const <Object?>[
      <String, Object?>{
        'id': 'rust-style',
        'content': 'Use rustfmt for Rust changes.',
      },
    ],
  });

  final List<Object?> skills;
  final List<String> calls = <String>[];
  bool fail = false;

  @override
  Future<Map<String, Object?>> call(
    String method, {
    Map<String, Object?> params = const <String, Object?>{},
    Duration timeout = const Duration(seconds: 15),
  }) async {
    calls.add(method);
    if (method == 'project.skills.list') {
      if (fail) throw Exception('Skill catalog unavailable.');
      return <String, Object?>{'skills': skills};
    }
    throw StateError('Unexpected service method: $method');
  }
}
