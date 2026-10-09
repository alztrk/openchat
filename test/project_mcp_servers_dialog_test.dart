import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/presentation/widgets/project_mcp_servers_dialog.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

import 'support/shad_test_scope.dart';

import 'package:openchat/platform/windows/openchat_service_client.dart';

void main() {
  testWidgets('MCP settings remain labeled across locales and layouts', (
    tester,
  ) async {
    final semantics = tester.ensureSemantics();
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    try {
      for (final locale in AppLocalizations.supportedLocales) {
        final l10n = await AppLocalizations.delegate.load(locale);
        for (final width in <double>[480, 1200]) {
          for (final scale in <double>[1, 2]) {
            tester.view.physicalSize = Size(width, 900);
            tester.view.devicePixelRatio = 1;
            final service = _FakeMcpServiceClient();
            await tester.pumpWidget(
              MaterialApp(
                locale: locale,
                localizationsDelegates: AppLocalizations.localizationsDelegates,
                supportedLocales: AppLocalizations.supportedLocales,
                theme: OpenChatTheme.dark,
                builder: openChatShadTestBuilder,
                home: MediaQuery(
                  data: MediaQueryData(
                    size: Size(width, 900),
                    textScaler: TextScaler.linear(scale),
                  ),
                  child: Scaffold(
                    body: ProjectMcpServersDialog(
                      key: ValueKey<String>('$locale-$width-$scale'),
                      serviceClient: service,
                      projectId: 'project-1',
                      initialPermissionRules:
                          const <String, ToolPermissionRule>{},
                      onSavePermissionRules: (_) async {},
                    ),
                  ),
                ),
              ),
            );
            await tester.pumpAndSettle();

            expect(find.text(l10n.projectMcpTitle), findsOneWidget);
            expect(find.text(l10n.projectMcpEmpty), findsOneWidget);
            expect(find.text(l10n.projectMcpAdd), findsOneWidget);
            expect(
              find.bySemanticsLabel(l10n.projectMcpAdd),
              findsOneWidget,
              reason: 'Add action semantics for ${locale.languageCode}',
            );
            await tester.scrollUntilVisible(
              find.text(l10n.projectMcpNoCredentials),
              80,
              scrollable: find.byType(Scrollable).last,
            );
            expect(find.text(l10n.projectMcpNoCredentials), findsOneWidget);
            expect(service.calls, contains('project.mcp.catalog.get'));
            expect(tester.takeException(), isNull);
          }
        }
      }
    } finally {
      semantics.dispose();
    }
  });

  testWidgets('server settings are validated and saved through the service', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1100, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final service = _FakeMcpServiceClient();
    Map<String, ToolPermissionRule>? savedPermissionRules;

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        builder: openChatShadTestBuilder,
        home: Scaffold(
          body: ProjectMcpServersDialog(
            serviceClient: service,
            projectId: 'project-1',
            initialPermissionRules: const <String, ToolPermissionRule>{},
            onSavePermissionRules: (rules) async {
              savedPermissionRules = rules;
            },
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('Add server'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextFormField).at(0), 'local_docs');
    await tester.enterText(
      find.byType(TextFormField).at(1),
      r'C:\tools\mcp-server.exe',
    );
    await tester.enterText(find.byType(TextFormField).at(2), '--read-only\n');
    await tester.enterText(find.byType(TextFormField).at(3), 'API_TOKEN');
    await tester.tap(
      find.byKey(const ValueKey<String>('project-mcp-permission-rule')),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('Allow').last);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Save').last);
    await tester.pumpAndSettle();

    expect(find.text('local_docs'), findsOneWidget);
    expect(service.calls, isNot(contains('project.mcp.catalog.save')));
    expect(service.calls, isNot(contains('project.mcp.server.check')));
    await tester.tap(find.byTooltip('Check connection'));
    await tester.pumpAndSettle();
    expect(find.text('Connected; discovered 2 tools'), findsOneWidget);
    expect(
      find.byKey(
        const ValueKey<String>('project-mcp-tool-rule-local_docs-read-file'),
      ),
      findsOneWidget,
    );
    await tester.tap(
      find.byKey(
        const ValueKey<String>('project-mcp-tool-rule-local_docs-read-file'),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('Deny').last);
    await tester.pumpAndSettle();
    expect(service.checkedServer?['id'], 'local_docs');
    expect(service.checkedServer?['program'], r'C:\tools\mcp-server.exe');
    expect(service.checkedServer?['arguments'], <String>['--read-only']);
    await tester.tap(find.text('Save').last);
    await tester.pumpAndSettle();

    expect(service.calls, contains('project.mcp.catalog.save'));
    expect(service.savedCatalog?['servers'], <Object?>[
      <String, Object?>{
        'id': 'local_docs',
        'enabled': true,
        'transport': 'stdio',
        'program': r'C:\tools\mcp-server.exe',
        'arguments': <String>['--read-only'],
        'environmentVariables': <String>['API_TOKEN'],
      },
    ]);
    expect(
      savedPermissionRules?['mcp__local_docs__*'],
      ToolPermissionRule.allow,
    );
    expect(
      savedPermissionRules?['mcp__local_docs__read-file'],
      ToolPermissionRule.deny,
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('remote MCP settings keep auth out of the project catalog', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1100, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final service = _FakeMcpServiceClient();

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        builder: openChatShadTestBuilder,
        home: Scaffold(
          body: ProjectMcpServersDialog(
            serviceClient: service,
            projectId: 'project-1',
            initialPermissionRules: const <String, ToolPermissionRule>{},
            onSavePermissionRules: (_) async {},
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('Add server'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextFormField).at(0), 'remote_docs');
    await tester.tap(
      find.byKey(const ValueKey<String>('project-mcp-transport')),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('Remote server (Streamable HTTP)').last);
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byType(TextFormField).at(1),
      'https://mcp.example.org/mcp',
    );
    await tester.enterText(find.byType(TextFormField).at(2), 'MCP_TOKEN');
    await tester.tap(find.text('Save').last);
    await tester.pumpAndSettle();

    await tester.tap(find.byTooltip('Check connection'));
    await tester.pumpAndSettle();
    expect(service.checkedServer?['transport'], 'streamableHttp');
    expect(service.checkedServer?['endpoint'], 'https://mcp.example.org/mcp');
    expect(service.checkedServer?['authEnvironmentVariable'], 'MCP_TOKEN');
    await tester.tap(find.byTooltip('MCP credentials'));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const ValueKey<String>('project-mcp-secret-MCP_TOKEN')),
      'remote-secret',
    );
    await tester.tap(find.text('Save securely'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Close'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Save').last);
    await tester.pumpAndSettle();

    expect(service.lastCredentialParams?['secret'], 'remote-secret');
    expect(service.savedCatalog?['servers'], <Object?>[
      <String, Object?>{
        'id': 'remote_docs',
        'enabled': true,
        'transport': 'streamableHttp',
        'arguments': <String>[],
        'environmentVariables': <String>[],
        'endpoint': 'https://mcp.example.org/mcp',
        'authEnvironmentVariable': 'MCP_TOKEN',
      },
    ]);
    expect(service.savedCatalog.toString(), isNot(contains('remote-secret')));
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'MCP credentials are stored through the secure service boundary',
    (tester) async {
      tester.view.physicalSize = const Size(1100, 900);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final service = _FakeMcpServiceClient();

      await tester.pumpWidget(
        MaterialApp(
          locale: const Locale('en'),
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          theme: OpenChatTheme.light,
          builder: openChatShadTestBuilder,
          home: Scaffold(
            body: ProjectMcpServersDialog(
              serviceClient: service,
              projectId: 'project-1',
              initialPermissionRules: const <String, ToolPermissionRule>{},
              onSavePermissionRules: (_) async {},
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text('Add server'));
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextFormField).at(0), 'private_api');
      await tester.enterText(
        find.byType(TextFormField).at(1),
        r'C:\tools\mcp-server.exe',
      );
      await tester.enterText(find.byType(TextFormField).at(3), 'API_TOKEN');
      await tester.tap(find.text('Save').last);
      await tester.pumpAndSettle();
      await tester.tap(find.byTooltip('MCP credentials'));
      await tester.pumpAndSettle();
      await tester.enterText(
        find.byKey(const ValueKey<String>('project-mcp-secret-API_TOKEN')),
        'sensitive-value',
      );
      await tester.tap(find.text('Save securely'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Close'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Save').last);
      await tester.pumpAndSettle();

      expect(service.calls, contains('project.mcp.credential.set'));
      expect(service.lastCredentialParams?['secret'], 'sensitive-value');
      expect(
        service.savedCatalog.toString(),
        isNot(contains('sensitive-value')),
      );
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets('removing a configured variable deletes its stored credential', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1100, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final service = _FakeMcpServiceClient()
      ..initialCatalog = <String, Object?>{
        'version': 1,
        'servers': <Object?>[
          <String, Object?>{
            'id': 'saved_server',
            'enabled': true,
            'program': r'C:\tools\mcp-server.exe',
            'arguments': <String>[],
            'environmentVariables': <String>['API_TOKEN'],
          },
        ],
      };

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        builder: openChatShadTestBuilder,
        home: Scaffold(
          body: ProjectMcpServersDialog(
            serviceClient: service,
            projectId: 'project-1',
            initialPermissionRules: const <String, ToolPermissionRule>{},
            onSavePermissionRules: (_) async {},
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byTooltip('Edit server'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextFormField).at(3), '');
    await tester.tap(find.text('Save').last);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Save').last);
    await tester.pumpAndSettle();

    expect(service.calls, contains('project.mcp.credential.remove'));
    expect(service.savedCatalog?['servers'], <Object?>[
      <String, Object?>{
        'id': 'saved_server',
        'enabled': true,
        'transport': 'stdio',
        'program': r'C:\tools\mcp-server.exe',
        'arguments': <String>[],
        'environmentVariables': <String>[],
      },
    ]);
    expect(tester.takeException(), isNull);
  });

  testWidgets('a failed catalog load cannot be overwritten as an empty list', (
    tester,
  ) async {
    final service = _FakeMcpServiceClient()..failCatalogLoad = true;
    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        builder: openChatShadTestBuilder,
        home: Scaffold(
          body: ProjectMcpServersDialog(
            serviceClient: service,
            projectId: 'project-1',
            initialPermissionRules: const <String, ToolPermissionRule>{},
            onSavePermissionRules: (_) async {},
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(
      find.text(
        'The project MCP catalog could not be loaded. Check `.openchat/mcp.json` for invalid or unsafe entries.',
      ),
      findsOneWidget,
    );
    expect(find.text('Retry'), findsOneWidget);
    expect(find.text('Save'), findsNothing);
    expect(find.text('Add server'), findsNothing);
    expect(service.calls, isNot(contains('project.mcp.catalog.save')));
  });
}

class _FakeMcpServiceClient extends OpenChatServiceClient {
  final List<String> calls = <String>[];
  bool failCatalogLoad = false;
  Map<String, Object?>? initialCatalog;
  Map<String, Object?>? savedCatalog;
  Map<String, Object?>? checkedServer;
  Map<String, Object?>? lastCredentialParams;

  @override
  Future<Map<String, Object?>> call(
    String method, {
    Map<String, Object?> params = const <String, Object?>{},
    Duration timeout = const Duration(seconds: 15),
  }) async {
    calls.add(method);
    if (method == 'project.mcp.credential.status') {
      return <String, Object?>{'stored': false};
    }
    if (method == 'project.mcp.credential.set') {
      lastCredentialParams = params;
      return <String, Object?>{'stored': true};
    }
    if (method == 'project.mcp.credential.remove') {
      return <String, Object?>{'removed': true};
    }
    if (method == 'project.mcp.server.check') {
      final server = params['server'];
      if (server is Map<String, Object?>) {
        checkedServer = server;
      }
    }
    if (method == 'project.mcp.catalog.get') {
      if (failCatalogLoad) throw Exception('Catalog unavailable.');
      return initialCatalog ??
          <String, Object?>{'version': 1, 'servers': <Object?>[]};
    }
    if (method == 'project.mcp.server.check') {
      return <String, Object?>{
        'status': 'connected',
        'toolCount': 2,
        'toolNames': <String>['read-file', 'search_files'],
      };
    }
    if (method == 'project.mcp.catalog.save') {
      final catalog = params['catalog'];
      if (catalog is Map<String, Object?>) {
        savedCatalog = catalog;
        return catalog;
      }
      throw StateError('Missing MCP catalog.');
    }
    throw StateError('Unexpected service method: $method');
  }
}
