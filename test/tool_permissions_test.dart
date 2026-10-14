import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/presentation/widgets/project_tool_permissions_dialog.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/features/settings/presentation/tool_permissions_settings_section.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:shared_preferences_platform_interface/in_memory_shared_preferences_async.dart';
import 'package:shared_preferences_platform_interface/shared_preferences_async_platform_interface.dart';

import 'support/shad_test_scope.dart';

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

  testWidgets('global permission choice is saved and refreshed in chat', (
    tester,
  ) async {
    final preferences = SettingsPreferences(SharedPreferencesAsync());
    var savedCallbackCount = 0;
    var openedProjectPermissions = false;

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        home: Scaffold(
          body: SingleChildScrollView(
            child: ToolPermissionsSettingsSection(
              settingsPreferences: preferences,
              onSaved: () async => savedCallbackCount++,
              onOpenProjectPermissions: () => openedProjectPermissions = true,
            ),
          ),
        ),
      ),
    );

    await tester.pumpAndSettle();
    expect(find.byType(RadioListTile<ToolPermissionMode>), findsNWidgets(3));
    expect(
      await preferences.readToolPermissionMode(),
      ToolPermissionMode.requireApproval,
    );

    await tester.tap(find.text('Full access'));
    await tester.pumpAndSettle();

    expect(
      await preferences.readToolPermissionMode(),
      ToolPermissionMode.fullAccess,
    );
    expect(savedCallbackCount, 1);

    await tester.tap(find.text('Project tool permissions'));
    expect(openedProjectPermissions, isTrue);
  });

  testWidgets('summarizes the active project overrides accurately', (
    tester,
  ) async {
    final preferences = SettingsPreferences(SharedPreferencesAsync());
    await preferences.writeProjectToolPermissionRules('project-1', {
      'list_files': ToolPermissionRule.allow,
      'execute_command': ToolPermissionRule.ask,
      'write_file': ToolPermissionRule.deny,
    });

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        home: Scaffold(
          body: SingleChildScrollView(
            child: ToolPermissionsSettingsSection(
              settingsPreferences: preferences,
              projectId: 'project-1',
              onOpenProjectPermissions: () {},
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(
      find.text(
        'Project rules: 1 allow, 1 require approval, 1 deny. Tools without a rule use the global mode.',
      ),
      findsOneWidget,
    );
  });

  testWidgets('project rules can be changed and saved accessibly', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1100, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    Map<String, ToolPermissionRule>? savedRules;
    await tester.pumpWidget(
      MaterialApp(
        theme: OpenChatTheme.light,
        builder: openChatShadTestBuilder,
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: Scaffold(
          body: Builder(
            builder: (context) => TextButton(
              onPressed: () => showDialog<void>(
                context: context,
                builder: (context) => ProjectToolPermissionsDialog(
                  initialRules: const <String, ToolPermissionRule>{
                    'run_project_task__removed': ToolPermissionRule.deny,
                    'project_tool__removed': ToolPermissionRule.deny,
                  },
                  namedTaskIds: const <String>['verify'],
                  configuredToolIds: const <String>['lint'],
                  onSave: (rules) async {
                    savedRules = rules;
                  },
                ),
              ),
              child: const Text('Open'),
            ),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Open'));
    await tester.pumpAndSettle();
    expect(find.text('Project tool permissions'), findsOneWidget);
    expect(find.text('Use global setting'), findsWidgets);
    expect(
      tester
          .widget<ProjectToolPermissionsDialog>(
            find.byType(ProjectToolPermissionsDialog),
          )
          .namedTaskIds,
      <String>['verify'],
    );
    expect(
      tester
          .widget<ProjectToolPermissionsDialog>(
            find.byType(ProjectToolPermissionsDialog),
          )
          .configuredToolIds,
      <String>['lint'],
    );
    await tester.scrollUntilVisible(
      find.byKey(
        const ValueKey<String>('project-tool-rule-run_project_task__verify'),
      ),
      240,
      scrollable: find.byType(Scrollable).last,
    );
    await tester.pumpAndSettle();
    expect(find.text('Run project task: verify'), findsOneWidget);

    await tester.scrollUntilVisible(
      find.byKey(
        const ValueKey<String>('project-tool-rule-project_tool__lint'),
      ),
      240,
      scrollable: find.byType(Scrollable).last,
    );
    await tester.pumpAndSettle();
    expect(find.text('Project tool: lint'), findsOneWidget);
    await tester.tap(
      find.byKey(
        const ValueKey<String>('project-tool-rule-project_tool__lint'),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('Allow').last);
    await tester.pumpAndSettle();

    await tester.scrollUntilVisible(
      find.byKey(
        const ValueKey<String>('project-tool-rule-run_project_task__verify'),
      ),
      240,
      scrollable: find.byType(Scrollable).last,
    );
    await tester.pumpAndSettle();
    await tester.tap(
      find.byKey(
        const ValueKey<String>('project-tool-rule-run_project_task__verify'),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('Deny').last);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();

    expect(savedRules?['run_project_task__verify'], ToolPermissionRule.deny);
    expect(savedRules?['project_tool__lint'], ToolPermissionRule.allow);
    expect(savedRules?.containsKey('run_project_task__removed'), isFalse);
    expect(savedRules?.containsKey('project_tool__removed'), isFalse);
    expect(find.byType(ProjectToolPermissionsDialog), findsNothing);
  });
}
