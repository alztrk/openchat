import 'package:flutter/foundation.dart';

@immutable
class GlobalMcpServer {
  const GlobalMcpServer({
    required this.id,
    required this.name,
    required this.category,
    required this.description,
    required this.command,
    this.args = const <String>[],
    this.requiredEnvKeys = const <String>[],
    this.optionalEnvKeys = const <String>[],
    this.installed = false,
    this.enabled = false,
    this.installDir = '',
    this.discoveredTools = const <String>[],
  });

  final String id;
  final String name;
  final String category;
  final String description;
  final String command;
  final List<String> args;
  final List<String> requiredEnvKeys;
  final List<String> optionalEnvKeys;
  final bool installed;
  final bool enabled;
  final String installDir;
  final List<String> discoveredTools;

  GlobalMcpServer copyWith({
    bool? installed,
    bool? enabled,
    String? installDir,
    List<String>? discoveredTools,
  }) {
    return GlobalMcpServer(
      id: id,
      name: name,
      category: category,
      description: description,
      command: command,
      args: args,
      requiredEnvKeys: requiredEnvKeys,
      optionalEnvKeys: optionalEnvKeys,
      installed: installed ?? this.installed,
      enabled: enabled ?? this.enabled,
      installDir: installDir ?? this.installDir,
      discoveredTools: discoveredTools ?? this.discoveredTools,
    );
  }

  Map<String, Object?> toJson() => <String, Object?>{
    'id': id,
    'name': name,
    'category': category,
    'description': description,
    'command': command,
    'args': args,
    'requiredEnvKeys': requiredEnvKeys,
    'optionalEnvKeys': optionalEnvKeys,
    'installed': installed,
    'enabled': enabled,
    'installDir': installDir,
    'discoveredTools': discoveredTools,
  };

  factory GlobalMcpServer.fromJson(Map<String, Object?> json) {
    return GlobalMcpServer(
      id: json['id'] as String? ?? '',
      name: json['name'] as String? ?? '',
      category: json['category'] as String? ?? 'General',
      description: json['description'] as String? ?? '',
      command: json['command'] as String? ?? '',
      args: (json['args'] as List<Object?>?)?.cast<String>() ?? const [],
      requiredEnvKeys:
          (json['requiredEnvKeys'] as List<Object?>?)?.cast<String>() ?? const [],
      optionalEnvKeys:
          (json['optionalEnvKeys'] as List<Object?>?)?.cast<String>() ?? const [],
      installed: json['installed'] as bool? ?? false,
      enabled: json['enabled'] as bool? ?? false,
      installDir: json['installDir'] as String? ?? '',
      discoveredTools:
          (json['discoveredTools'] as List<Object?>?)?.cast<String>() ?? const [],
    );
  }

  /// Builds the dynamic instruction string for this server when enabled.
  String toInstructionBlock() {
    final buffer = StringBuffer();
    buffer.writeln('### Active MCP Tool: $name ($id)');
    buffer.writeln(description);
    if (discoveredTools.isNotEmpty) {
      buffer.writeln('Available Tools:');
      for (final tool in discoveredTools) {
        buffer.writeln('- $tool');
      }
    }
    return buffer.toString().trim();
  }
}
