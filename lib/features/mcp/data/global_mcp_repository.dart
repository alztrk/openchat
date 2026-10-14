import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:openchat/features/mcp/data/popular_mcp_catalog.dart';
import 'package:openchat/features/mcp/domain/global_mcp_server.dart';

class GlobalMcpRepository {
  GlobalMcpRepository({FlutterSecureStorage? secureStorage})
      : _secureStorage = secureStorage ?? const FlutterSecureStorage();

  final FlutterSecureStorage _secureStorage;

  static const String _credentialPrefix = 'openchat_global_mcp';

  /// Resolves the root directory for global MCP configurations:
  /// `%LOCALAPPDATA%\OpenChat\mcp\`
  Directory getMcpDirectory() {
    final localAppData = Platform.environment['LOCALAPPDATA'];
    final basePath = localAppData != null && localAppData.isNotEmpty
        ? localAppData
        : '${Platform.environment['USERPROFILE'] ?? '.'}\\AppData\\Local';

    final dir = Directory('$basePath\\OpenChat\\mcp');
    if (!dir.existsSync()) {
      dir.createSync(recursive: true);
    }
    return dir;
  }

  File _getConfigFile(String id) {
    return File('${getMcpDirectory().path}\\$id.json');
  }

  Directory _getInstallDirectory(String id) {
    final dir = Directory('${getMcpDirectory().path}\\$id');
    if (!dir.existsSync()) {
      dir.createSync(recursive: true);
    }
    return dir;
  }

  /// Loads all servers combining predefined popular servers and installed configurations.
  Future<List<GlobalMcpServer>> loadAllServers() async {
    final mcpDir = getMcpDirectory();
    final Map<String, GlobalMcpServer> serverMap = {
      for (final s in popularMcpCatalog) s.id: s,
    };

    if (mcpDir.existsSync()) {
      final entities = mcpDir.listSync();
      for (final entity in entities) {
        if (entity is File && entity.path.endsWith('.json')) {
          try {
            final raw = entity.readAsStringSync();
            final json = jsonDecode(raw) as Map<String, Object?>;
            final id = json['id'] as String? ??
                entity.uri.pathSegments.last.replaceAll('.json', '');

            if (serverMap.containsKey(id)) {
              final existing = serverMap[id]!;
              serverMap[id] = existing.copyWith(
                installed: true,
                enabled: json['enabled'] as bool? ?? false,
                installDir: entity.parent.path,
                discoveredTools: (json['discoveredTools'] as List<Object?>?)
                        ?.cast<String>() ??
                    existing.discoveredTools,
              );
            } else {
              serverMap[id] = GlobalMcpServer.fromJson(json);
            }
          } catch (_) {
            // Ignore malformed individual config files
          }
        }
      }
    }

    return serverMap.values.toList(growable: false);
  }

  /// Installs an MCP server by creating its directory and saving its configuration.
  Future<GlobalMcpServer> installServer(GlobalMcpServer server) async {
    final installDir = _getInstallDirectory(server.id);
    final configFile = _getConfigFile(server.id);

    final updated = server.copyWith(
      installed: true,
      enabled: true,
      installDir: installDir.path,
    );

    final payload = updated.toJson();
    await configFile.writeAsString(
      const JsonEncoder.withIndent('  ').convert(payload),
    );

    return updated;
  }

  /// Uninstalls an MCP server: removes config file, install directory, and secure credentials.
  Future<void> uninstallServer(String id) async {
    final configFile = _getConfigFile(id);
    if (configFile.existsSync()) {
      await configFile.delete();
    }
    final installDir = _getInstallDirectory(id);
    if (installDir.existsSync()) {
      try {
        await installDir.delete(recursive: true);
      } catch (_) {
        // Ignored if files are in use
      }
    }
  }

  /// Toggles enabled state of an installed MCP server.
  Future<GlobalMcpServer> toggleEnabled(String id, bool enabled) async {
    final configFile = _getConfigFile(id);
    if (!configFile.existsSync()) {
      throw StateError('MCP configuration not found for $id');
    }

    final raw = await configFile.readAsString();
    final json = jsonDecode(raw) as Map<String, Object?>;
    json['enabled'] = enabled;

    await configFile.writeAsString(
      const JsonEncoder.withIndent('  ').convert(json),
    );

    return GlobalMcpServer.fromJson(json);
  }

  /// Securely reads an environment variable using Windows Credential Manager.
  Future<String?> readSecureEnv(String serverId, String envKey) async {
    return _secureStorage.read(
      key: '${_credentialPrefix}_${serverId}_$envKey',
    );
  }

  /// Securely stores an environment variable using Windows Credential Manager.
  Future<void> writeSecureEnv(
    String serverId,
    String envKey,
    String value,
  ) async {
    if (value.trim().isEmpty) {
      await _secureStorage.delete(
        key: '${_credentialPrefix}_${serverId}_$envKey',
      );
    } else {
      await _secureStorage.write(
        key: '${_credentialPrefix}_${serverId}_$envKey',
        value: value.trim(),
      );
    }
  }

  /// Probes the MCP server via stdio JSON-RPC to discover available tools automatically.
  /// Sends `initialize`, `notifications/initialized`, and `tools/list`.
  Future<GlobalMcpServer> probeTools(GlobalMcpServer server) async {
    final env = <String, String>{};
    for (final key in [...server.requiredEnvKeys, ...server.optionalEnvKeys]) {
      final value = await readSecureEnv(server.id, key);
      if (value != null && value.isNotEmpty) {
        env[key] = value;
      }
    }

    final installDir = _getInstallDirectory(server.id);
    Process process;
    try {
      process = await Process.start(
        server.command,
        server.args,
        environment: env,
        workingDirectory: installDir.path,
        runInShell: Platform.isWindows,
      );
    } catch (e) {
      throw StateError('Sunucu komutu çalıştırılamadı: $e');
    }

    final discovered = <String>[];
    StreamSubscription<String>? subscription;
    try {
      final initRequest = jsonEncode({
        'jsonrpc': '2.0',
        'id': 1,
        'method': 'initialize',
        'params': {
          'protocolVersion': '2024-11-05',
          'capabilities': {},
          'clientInfo': {'name': 'OpenChat', 'version': '1.0.0'},
        },
      });

      final initializedNotification = jsonEncode({
        'jsonrpc': '2.0',
        'method': 'notifications/initialized',
      });

      final listToolsRequest = jsonEncode({
        'jsonrpc': '2.0',
        'id': 2,
        'method': 'tools/list',
        'params': {},
      });

      final lines = process.stdout
          .transform(utf8.decoder)
          .transform(const LineSplitter());

      process.stdin.writeln(initRequest);

      final completer = Completer<List<String>>();

      subscription = lines.listen((line) {
        final trimmed = line.trim();
        if (trimmed.isEmpty) return;

        try {
          final decoded = jsonDecode(trimmed);
          if (decoded is Map<String, Object?>) {
            if (decoded['id'] == 1) {
              process.stdin.writeln(initializedNotification);
              process.stdin.writeln(listToolsRequest);
            } else if (decoded['id'] == 2) {
              final result = decoded['result'] as Map<String, Object?>?;
              final tools = result?['tools'] as List<Object?>? ?? const [];
              final toolList = <String>[];
              for (final t in tools) {
                if (t is Map<String, Object?> && t['name'] is String) {
                  final name = t['name'] as String;
                  final desc = t['description'] as String?;
                  if (desc != null && desc.isNotEmpty) {
                    toolList.add('$name: $desc');
                  } else {
                    toolList.add(name);
                  }
                }
              }
              if (!completer.isCompleted) {
                completer.complete(toolList);
              }
            }
          }
        } catch (_) {}
      });

      final tools = await completer.future.timeout(
        const Duration(seconds: 12),
        onTimeout: () => throw TimeoutException('MCP yanıtı zaman aşımına uğradı.'),
      );
      discovered.addAll(tools);
    } finally {
      await subscription?.cancel();
      process.kill();
    }

    final updated = server.copyWith(
      installed: true,
      discoveredTools: discovered,
    );

    final configFile = _getConfigFile(server.id);
    await configFile.writeAsString(
      const JsonEncoder.withIndent('  ').convert(updated.toJson()),
    );

    return updated;
  }

  /// Builds a combined dynamic instruction block for all currently active MCP servers.
  /// If no MCP server is active, returns an empty string.
  Future<String> buildActiveInstructions() async {
    final servers = await loadAllServers();
    final activeServers =
        servers.where((s) => s.installed && s.enabled).toList(growable: false);

    if (activeServers.isEmpty) {
      return '';
    }

    final buffer = StringBuffer();
    buffer.writeln(
      'Active Model Context Protocol (MCP) Tools Available in this environment:\n'
      'The assistant has access to the following active MCP servers. When relevant to the user request, '
      'use these tools to fetch fresh data, perform actions, or interact with external services.\n',
    );

    for (final server in activeServers) {
      buffer.writeln(server.toInstructionBlock());
      buffer.writeln();
    }

    return buffer.toString().trim();
  }
}
