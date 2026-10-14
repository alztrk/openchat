import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:openchat/features/mcp/data/global_mcp_repository.dart';
import 'package:openchat/features/mcp/data/popular_mcp_catalog.dart';
import 'package:openchat/features/mcp/domain/global_mcp_server.dart';

void main() {
  group('PopularMcpCatalog', () {
    test('contains popular servers and explicitly excludes openmail', () {
      expect(popularMcpCatalog.isNotEmpty, isTrue);
      expect(
        popularMcpCatalog.any((s) => s.id.toLowerCase().contains('openmail')),
        isFalse,
      );
    });

    test('each catalog server has valid identifier and command', () {
      final ids = <String>{};
      for (final server in popularMcpCatalog) {
        expect(server.id.trim(), isNotEmpty);
        expect(ids.add(server.id), isTrue, reason: 'Duplicate id: ${server.id}');
        expect(server.name.trim(), isNotEmpty);
        expect(server.command.trim(), isNotEmpty);
        expect(server.category.trim(), isNotEmpty);
      }
    });
  });

  group('GlobalMcpServer', () {
    test('serializes and deserializes correctly', () {
      const server = GlobalMcpServer(
        id: 'test_mcp',
        name: 'Test MCP',
        category: 'Developer',
        description: 'Test description',
        command: 'npx',
        args: ['-y', 'test-pkg'],
        requiredEnvKeys: ['API_KEY'],
        optionalEnvKeys: ['DEBUG'],
        installed: true,
        enabled: true,
        installDir: r'C:\test\mcp',
        discoveredTools: ['tool_a: do something', 'tool_b: do other'],
      );

      final json = server.toJson();
      final revived = GlobalMcpServer.fromJson(json);

      expect(revived.id, equals(server.id));
      expect(revived.name, equals(server.name));
      expect(revived.category, equals(server.category));
      expect(revived.command, equals(server.command));
      expect(revived.args, equals(server.args));
      expect(revived.requiredEnvKeys, equals(server.requiredEnvKeys));
      expect(revived.optionalEnvKeys, equals(server.optionalEnvKeys));
      expect(revived.installed, isTrue);
      expect(revived.enabled, isTrue);
      expect(revived.installDir, equals(server.installDir));
      expect(revived.discoveredTools, equals(server.discoveredTools));
    });

    test('generates instruction block with discovered tools', () {
      const server = GlobalMcpServer(
        id: 'github',
        name: 'GitHub',
        category: 'Developer',
        description: 'GitHub integration',
        command: 'npx',
        args: ['-y', '@modelcontextprotocol/server-github'],
        installed: true,
        enabled: true,
        discoveredTools: ['create_issue', 'get_file_contents'],
      );

      final block = server.toInstructionBlock();
      expect(block, contains('### Active MCP Tool: GitHub (github)'));
      expect(block, contains('create_issue'));
      expect(block, contains('get_file_contents'));
    });
  });

  group('GlobalMcpRepository', () {
    late Directory tempDir;
    late GlobalMcpRepository repository;

    setUp(() {
      TestWidgetsFlutterBinding.ensureInitialized();
      FlutterSecureStorage.setMockInitialValues({});
      tempDir = Directory.systemTemp.createTempSync('openchat_mcp_test_');
      repository = GlobalMcpRepository();
    });

    tearDown(() {
      if (tempDir.existsSync()) {
        try {
          tempDir.deleteSync(recursive: true);
        } catch (_) {}
      }
    });

    test('secure storage handles environment credentials', () async {
      await repository.writeSecureEnv('test_srv', 'GITHUB_TOKEN', 'secret_token_123');
      final readVal = await repository.readSecureEnv('test_srv', 'GITHUB_TOKEN');
      expect(readVal, equals('secret_token_123'));

      // Clearing value deletes it
      await repository.writeSecureEnv('test_srv', 'GITHUB_TOKEN', '');
      final clearedVal = await repository.readSecureEnv('test_srv', 'GITHUB_TOKEN');
      expect(clearedVal, isNull);
    });

    test('buildActiveInstructions returns empty when no servers are installed/enabled', () async {
      final instructions = await repository.buildActiveInstructions();
      expect(instructions, isEmpty);
    });
  });
}
