import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/features/settings/domain/connector_catalog_entry.dart';
import 'package:openchat/features/settings/presentation/connector_catalog_settings_section.dart';
import 'package:openchat/features/settings/presentation/settings_screen.dart';
import 'package:openchat/features/chat/domain/history_storage_status.dart';

import 'support/shad_test_scope.dart';

import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:shared_preferences_platform_interface/in_memory_shared_preferences_async.dart';
import 'package:shared_preferences_platform_interface/shared_preferences_async_platform_interface.dart';

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

  const entry = ConnectorCatalogEntry(
    id: 'docs_server',
    name: 'Documentation server',
    publisher: 'Example publisher',
    version: '1.0.0',
    sourceUrl: 'https://example.com/source',
    requestedAccess: 'Reads documentation pages over HTTPS.',
    dataDisclosure: 'Selected search terms and page URLs.',
    transport: ConnectorCatalogTransport.streamableHttp,
    endpoint: 'https://mcp.example.com',
    authEnvironmentVariable: 'DOCS_TOKEN',
  );

  test('connector template round-trips without credential values', () async {
    final preferences = SettingsPreferences(SharedPreferencesAsync());

    await preferences.writeConnectorCatalogEntries(const [entry]);

    final reopened = await SettingsPreferences(SharedPreferencesAsync())
        .readConnectorCatalogEntries();
    expect(reopened.single.toJson(), entry.toJson());
    expect(entry.toJson().containsKey('secret'), isFalse);
  });

  test('catalog rejects duplicate IDs, unsafe URLs, and inline secrets', () {
    expect(
      () => validateConnectorCatalog(const [entry, entry]),
      throwsArgumentError,
    );
    expect(
      () => ConnectorCatalogEntry.fromJson(<String, Object?>{
        ...entry.toJson(),
        'endpoint': 'https://user:password@mcp.example.com',
      }),
      throwsFormatException,
    );
    expect(
      () => ConnectorCatalogEntry.fromJson(<String, Object?>{
        ...entry.toJson(),
        'transport': 'stdio',
        'endpoint': null,
        'authEnvironmentVariable': null,
        'program': r'C:\Tools\mcp.exe',
        'arguments': const ['--api-key=secret-value'],
      }),
      throwsFormatException,
    );
  });

  testWidgets('install adds a disabled project server with Ask permission', (
    tester,
  ) async {
    final preferences = SettingsPreferences(SharedPreferencesAsync());
    await preferences.writeConnectorCatalogEntries(const [entry]);
    Map<String, Object?>? savedCatalog;
    final serviceClient = _FakeServiceClient((method, params) async {
      if (method == 'project.mcp.catalog.get') {
        return <String, Object?>{'version': 1, 'servers': <Object?>[]};
      }
      if (method == 'project.mcp.catalog.save') {
        savedCatalog = params['catalog'] as Map<String, Object?>;
        return <String, Object?>{};
      }
      throw StateError('Unexpected service method: $method');
    });

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark,
        builder: (context, child) => openChatShadTestScope(
          Theme.of(context),
          child ?? const SizedBox.shrink(),
        ),
        home: Scaffold(
          body: SingleChildScrollView(
            child: ConnectorCatalogSettingsSection(
              settingsPreferences: preferences,
              serviceClient: serviceClient,
              projectId: 'project-1',
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.enterText(find.byType(TextField), 'unknown connector');
    await tester.pumpAndSettle();
    expect(find.text('No templates match this search.'), findsOneWidget);
    await tester.enterText(find.byType(TextField), 'Documentation');
    await tester.pumpAndSettle();
    expect(find.text('Documentation server'), findsOneWidget);

    await tester.ensureVisible(find.text('Add to active project').first);
    await tester.tap(find.text('Add to active project').first);
    await tester.pumpAndSettle();
    expect(find.textContaining('Tool calls may send'), findsOneWidget);
    expect(
      find.textContaining(
        'If configured, a bearer credential is scoped to this project and server',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining(
        'Removing this template does not remove project servers or their saved credentials',
      ),
      findsOneWidget,
    );
    await tester.tap(find.text('Add to active project').last);
    await tester.pumpAndSettle();

    final servers = savedCatalog?['servers'] as List<Object?>?;
    expect(servers, hasLength(1));
    expect((servers!.single as Map<String, Object?>)['enabled'], isFalse);
    final rules = await preferences.readProjectToolPermissionRules('project-1');
    expect(rules['mcp__docs_server__*'], ToolPermissionRule.ask);
  });

  testWidgets('failed project install restores the prior permission rules', (
    tester,
  ) async {
    final preferences = SettingsPreferences(SharedPreferencesAsync());
    await preferences.writeConnectorCatalogEntries(const [entry]);
    await preferences.writeProjectToolPermissionRules('project-1', const {
      'web_search': ToolPermissionRule.deny,
    });
    final serviceClient = _FakeServiceClient((method, params) async {
      if (method == 'project.mcp.catalog.get') {
        return <String, Object?>{'version': 1, 'servers': <Object?>[]};
      }
      if (method == 'project.mcp.catalog.save') {
        throw Exception('Catalog storage is unavailable.');
      }
      throw StateError('Unexpected service method: $method');
    });

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark,
        builder: (context, child) => openChatShadTestScope(
          Theme.of(context),
          child ?? const SizedBox.shrink(),
        ),
        home: Scaffold(
          body: SingleChildScrollView(
            child: ConnectorCatalogSettingsSection(
              settingsPreferences: preferences,
              serviceClient: serviceClient,
              projectId: 'project-1',
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('Add to active project').first);
    await tester.tap(find.text('Add to active project').first);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Add to active project').last);
    await tester.pumpAndSettle();

    expect(await preferences.readProjectToolPermissionRules('project-1'), {
      'web_search': ToolPermissionRule.deny,
    });
  });

  testWidgets('install disclosure is localized across supported locales', (
    tester,
  ) async {
    final preferences = SettingsPreferences(SharedPreferencesAsync());
    await preferences.writeConnectorCatalogEntries(const [entry]);
    final serviceClient = _FakeServiceClient((method, params) async {
      if (method == 'project.mcp.catalog.get') {
        return <String, Object?>{'version': 1, 'servers': <Object?>[]};
      }
      throw StateError('Unexpected service method: $method');
    });

    for (final locale in AppLocalizations.supportedLocales) {
      await tester.pumpWidget(
        MaterialApp(
          locale: locale,
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          theme: OpenChatTheme.dark,
          builder: (context, child) => openChatShadTestScope(
            Theme.of(context),
            child ?? const SizedBox.shrink(),
          ),
          home: Scaffold(
            body: SingleChildScrollView(
              child: ConnectorCatalogSettingsSection(
                settingsPreferences: preferences,
                serviceClient: serviceClient,
                projectId: 'project-1',
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      final l10n = AppLocalizations.of(
        tester.element(find.byType(ConnectorCatalogSettingsSection)),
      )!;
      await tester.ensureVisible(find.text(l10n.connectorCatalogInstall));
      await tester.tap(find.text(l10n.connectorCatalogInstall).first);
      await tester.pumpAndSettle();

      expect(find.text(l10n.connectorCatalogAuthentication), findsOneWidget);
      expect(
        find.text(l10n.connectorCatalogProjectBearerCredential),
        findsOneWidget,
      );
      expect(
        find.textContaining(l10n.connectorCatalogAuthVariable),
        findsOneWidget,
      );
      expect(find.text(l10n.connectorCatalogRemovalImpact), findsOneWidget);

      await tester.tap(find.text(l10n.cancel));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
    }
  });

  testWidgets('user can create a reusable remote connector template', (
    tester,
  ) async {
    final preferences = SettingsPreferences(SharedPreferencesAsync());
    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark,
        home: Scaffold(
          body: SingleChildScrollView(
            child: ConnectorCatalogSettingsSection(
              settingsPreferences: preferences,
              serviceClient: null,
              projectId: null,
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('Add template'));
    await tester.pumpAndSettle();

    Future<void> enterField(String label, String value) async {
      final field = find.widgetWithText(TextFormField, label);
      await tester.ensureVisible(field);
      await tester.enterText(field, value);
    }

    await enterField('Server ID', 'docs_server');
    await enterField('Template name', 'Documentation');
    await enterField('Publisher claim', 'Docs Team');
    await enterField('Declared version', '1.0.0');
    await enterField('Source URL', 'https://example.com/docs-server');
    await enterField(
      'Access requested',
      'Reads public documentation pages over HTTPS.',
    );
    await enterField(
      'Data this connector may receive',
      'Selected search terms and page URLs.',
    );
    await tester.ensureVisible(find.text('Local process (stdio)'));
    await tester.tap(find.text('Local process (stdio)'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Remote server (Streamable HTTP)').last);
    await tester.pumpAndSettle();
    await enterField('HTTPS endpoint', 'https://mcp.example.com');
    await tester.tap(find.text('Save template'));
    await tester.pumpAndSettle();

    final saved = await preferences.readConnectorCatalogEntries();
    expect(saved, hasLength(1));
    expect(saved.single.publisher, 'Docs Team');
    expect(saved.single.transport, ConnectorCatalogTransport.streamableHttp);
    expect(saved.single.endpoint, 'https://mcp.example.com');
  });

  testWidgets('install refuses to overwrite a project server with same ID', (
    tester,
  ) async {
    final preferences = SettingsPreferences(SharedPreferencesAsync());
    await preferences.writeConnectorCatalogEntries(const [entry]);
    var saveCalled = false;
    final serviceClient = _FakeServiceClient((method, params) async {
      if (method == 'project.mcp.catalog.get') {
        return <String, Object?>{
          'version': 1,
          'servers': <Object?>[
            <String, Object?>{'id': entry.id},
          ],
        };
      }
      if (method == 'project.mcp.catalog.save') {
        saveCalled = true;
        return <String, Object?>{};
      }
      throw StateError('Unexpected service method: $method');
    });

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark,
        home: Scaffold(
          body: SingleChildScrollView(
            child: ConnectorCatalogSettingsSection(
              settingsPreferences: preferences,
              serviceClient: serviceClient,
              projectId: 'project-1',
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.ensureVisible(find.text('Add to active project').first);
    await tester.tap(find.text('Add to active project').first);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Add to active project').last);
    await tester.pumpAndSettle();

    expect(saveCalled, isFalse);
    expect(
      find.text('A template or project server already uses this ID or name.'),
      findsOneWidget,
    );
  });

  testWidgets('connector catalog is reachable from settings navigation', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1400, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final preferences = SettingsPreferences(SharedPreferencesAsync());

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark,
        builder: (context, child) => openChatShadTestScope(
          Theme.of(context),
          child ?? const SizedBox.shrink(),
        ),
        home: Scaffold(
          body: SettingsScreen(
            themeMode: ThemeMode.dark,
            onThemeModeChanged: (_) async {},
            historyStorageStatus: HistoryStorageStatus.available,
            hasConversationHistory: false,
            settingsPreferences: preferences,
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('Connectors'));
    await tester.pumpAndSettle();

    expect(
      find.textContaining('Save reusable MCP connection details'),
      findsOneWidget,
    );
    expect(
      find.text('No connector templates have been added.'),
      findsOneWidget,
    );
  });

  testWidgets('connector template editor fits supported locales and layouts', (
    tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final preferences = SettingsPreferences(SharedPreferencesAsync());
    await preferences.writeConnectorCatalogEntries(const [entry]);
    const locales = <Locale>[
      Locale('de'),
      Locale('en'),
      Locale('es'),
      Locale('fr'),
      Locale('tr'),
    ];
    const widths = <double>[360, 768, 1280];
    const scales = <double>[1, 2];

    for (final locale in locales) {
      for (final width in widths) {
        for (final scale in scales) {
          tester.view.physicalSize = Size(width, 960);
          await tester.pumpWidget(
            MaterialApp(
              locale: locale,
              localizationsDelegates: AppLocalizations.localizationsDelegates,
              supportedLocales: AppLocalizations.supportedLocales,
              theme: OpenChatTheme.dark,
              builder: (context, child) {
                final media = MediaQuery.of(context);
                return openChatShadTestScope(
                  Theme.of(context),
                  MediaQuery(
                    data: media.copyWith(textScaler: TextScaler.linear(scale)),
                    child: child ?? const SizedBox.shrink(),
                  ),
                );
              },
              home: Scaffold(
                body: SingleChildScrollView(
                  child: ConnectorCatalogSettingsSection(
                    settingsPreferences: preferences,
                    serviceClient: null,
                    projectId: null,
                  ),
                ),
              ),
            ),
          );
          await tester.pumpAndSettle();
          final l10n = AppLocalizations.of(
            tester.element(find.byType(ConnectorCatalogSettingsSection)),
          )!;
          final editButton = find.byTooltip(l10n.connectorCatalogEdit);
          await tester.ensureVisible(editButton);
          await tester.tap(editButton);
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);
          await tester.tap(find.text(l10n.cancel));
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);
        }
      }
    }
  });
}
