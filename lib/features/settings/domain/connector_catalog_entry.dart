import 'dart:convert';

import 'package:flutter/foundation.dart';

enum ConnectorCatalogTransport {
  stdio,
  streamableHttp;

  String get storageValue => switch (this) {
    ConnectorCatalogTransport.stdio => 'stdio',
    ConnectorCatalogTransport.streamableHttp => 'streamableHttp',
  };

  static ConnectorCatalogTransport fromStorageValue(Object? value) =>
      switch (value) {
        'stdio' => ConnectorCatalogTransport.stdio,
        'streamableHttp' => ConnectorCatalogTransport.streamableHttp,
        _ => throw const FormatException('Connector transport is invalid.'),
      };
}

@immutable
class ConnectorCatalogEntry {
  const ConnectorCatalogEntry({
    required this.id,
    required this.name,
    required this.publisher,
    required this.version,
    required this.sourceUrl,
    required this.requestedAccess,
    required this.dataDisclosure,
    required this.transport,
    this.program,
    this.arguments = const <String>[],
    this.environmentVariables = const <String>[],
    this.endpoint,
    this.authEnvironmentVariable,
  });

  final String id;
  final String name;
  final String publisher;
  final String version;
  final String sourceUrl;
  final String requestedAccess;
  final String dataDisclosure;
  final ConnectorCatalogTransport transport;
  final String? program;
  final List<String> arguments;
  final List<String> environmentVariables;
  final String? endpoint;
  final String? authEnvironmentVariable;

  Map<String, Object?> toJson() => <String, Object?>{
    'id': id,
    'name': name,
    'publisher': publisher,
    'version': version,
    'sourceUrl': sourceUrl,
    'requestedAccess': requestedAccess,
    'dataDisclosure': dataDisclosure,
    'transport': transport.storageValue,
    if (program != null) 'program': program,
    'arguments': arguments,
    'environmentVariables': environmentVariables,
    if (endpoint != null) 'endpoint': endpoint,
    if (authEnvironmentVariable != null)
      'authEnvironmentVariable': authEnvironmentVariable,
  };

  factory ConnectorCatalogEntry.fromJson(Object? value) {
    if (value is! Map<String, Object?>) {
      throw const FormatException('Connector catalog entry is invalid.');
    }
    final id = _requiredString(value, 'id', 24);
    final name = _requiredString(value, 'name', 80);
    final publisher = _requiredString(value, 'publisher', 120);
    final version = _requiredString(value, 'version', 64);
    final sourceUrl = _requiredHttpsUrl(value, 'sourceUrl');
    final requestedAccess = _requiredString(value, 'requestedAccess', 500);
    final dataDisclosure = _requiredString(value, 'dataDisclosure', 500);
    final transport = ConnectorCatalogTransport.fromStorageValue(
      value['transport'],
    );
    final program = _optionalString(value, 'program', 4096);
    final arguments = _stringList(value, 'arguments', 64, 4096);
    final environmentVariables = _stringList(
      value,
      'environmentVariables',
      32,
      128,
    );
    final endpoint = value['endpoint'] == null
        ? null
        : _requiredHttpsUrl(value, 'endpoint');
    final authEnvironmentVariable = _optionalString(
      value,
      'authEnvironmentVariable',
      128,
    );

    if (!RegExp(r'^[a-z0-9_]{1,24}$').hasMatch(id) ||
        name.trim().isEmpty ||
        publisher.trim().isEmpty ||
        version.trim().isEmpty ||
        requestedAccess.trim().isEmpty ||
        dataDisclosure.trim().isEmpty ||
        sourceUrl != sourceUrl.trim() ||
        (transport == ConnectorCatalogTransport.stdio &&
            (program == null || !_isAbsoluteWindowsPath(program))) ||
        (transport == ConnectorCatalogTransport.streamableHttp &&
            (endpoint == null ||
                program != null ||
                arguments.isNotEmpty ||
                environmentVariables.isNotEmpty)) ||
        (transport == ConnectorCatalogTransport.stdio &&
            (endpoint != null ||
                authEnvironmentVariable != null ||
                arguments.any(_looksLikeEmbeddedSecret)))) {
      throw const FormatException('Connector catalog entry is invalid.');
    }

    final envNames = <String>{};
    for (final variable in <String>[
      ...environmentVariables,
      ?authEnvironmentVariable,
    ]) {
      if (!_isSafeEnvironmentName(variable) || !envNames.add(variable)) {
        throw const FormatException('Connector credentials are invalid.');
      }
    }

    return ConnectorCatalogEntry(
      id: id,
      name: name.trim(),
      publisher: publisher.trim(),
      version: version.trim(),
      sourceUrl: sourceUrl,
      requestedAccess: requestedAccess.trim(),
      dataDisclosure: dataDisclosure.trim(),
      transport: transport,
      program: program?.trim(),
      arguments: List<String>.unmodifiable(arguments),
      environmentVariables: List<String>.unmodifiable(environmentVariables),
      endpoint: endpoint,
      authEnvironmentVariable: authEnvironmentVariable,
    );
  }

  Map<String, Object?> toProjectServer() => <String, Object?>{
    'id': id,
    'enabled': false,
    'transport': transport.storageValue,
    if (program != null) 'program': program,
    'arguments': arguments,
    'environmentVariables': environmentVariables,
    if (endpoint != null) 'endpoint': endpoint,
    if (authEnvironmentVariable != null)
      'authEnvironmentVariable': authEnvironmentVariable,
  };
}

String _requiredString(Map<String, Object?> value, String key, int maxLength) {
  final item = value[key];
  if (item is! String || item.isEmpty || item.length > maxLength) {
    throw FormatException('Connector $key is invalid.');
  }
  return item;
}

String? _optionalString(Map<String, Object?> value, String key, int maxLength) {
  final item = value[key];
  if (item == null) return null;
  if (item is! String || item.isEmpty || item.length > maxLength) {
    throw FormatException('Connector $key is invalid.');
  }
  return item;
}

String _requiredHttpsUrl(Map<String, Object?> value, String key) {
  final raw = _requiredString(value, key, 2048);
  final uri = Uri.tryParse(raw);
  if (uri == null ||
      uri.scheme != 'https' ||
      uri.host.isEmpty ||
      uri.userInfo.isNotEmpty ||
      uri.hasFragment ||
      uri.hasQuery) {
    throw FormatException('Connector $key must be a plain HTTPS URL.');
  }
  return uri.toString();
}

List<String> _stringList(
  Map<String, Object?> value,
  String key,
  int maxCount,
  int maxLength,
) {
  final raw = value[key];
  if (raw == null) return const <String>[];
  if (raw is! List<Object?> ||
      raw.length > maxCount ||
      raw.any((item) => item is! String || item.length > maxLength)) {
    throw FormatException('Connector $key is invalid.');
  }
  return raw.cast<String>();
}

bool _isAbsoluteWindowsPath(String value) =>
    RegExp(r'^(?:[A-Za-z]:[\\/]|\\\\[^\\]+\\[^\\]+)').hasMatch(value);

bool _isSafeEnvironmentName(String value) {
  if (!RegExp(r'^[A-Za-z_][A-Za-z0-9_]{0,127}$').hasMatch(value)) return false;
  return !<String>{
    'PATH',
    'SYSTEMROOT',
    'WINDIR',
    'PATHEXT',
    'COMSPEC',
    'USERPROFILE',
    'APPDATA',
    'LOCALAPPDATA',
    'TEMP',
    'TMP',
  }.contains(value.toUpperCase());
}

bool _looksLikeEmbeddedSecret(String argument) => RegExp(
  r'(?:api[_-]?key|access[_-]?token|refresh[_-]?token|password|secret|authorization|bearer)\s*[:=]\s*\S+',
  caseSensitive: false,
).hasMatch(argument);

void validateConnectorCatalog(List<ConnectorCatalogEntry> entries) {
  if (entries.length > 50) {
    throw ArgumentError.value(entries.length, 'entries', 'Too many entries.');
  }
  final ids = <String>{};
  final names = <String>{};
  for (final entry in entries) {
    final parsed = ConnectorCatalogEntry.fromJson(entry.toJson());
    if (!ids.add(parsed.id) || !names.add(parsed.name.toLowerCase())) {
      throw ArgumentError.value(entries, 'entries', 'Entries must be unique.');
    }
  }
  if (utf8
          .encode(jsonEncode(entries.map((entry) => entry.toJson()).toList()))
          .length >
      64 * 1024) {
    throw ArgumentError.value(entries, 'entries', 'Catalog is too large.');
  }
}
