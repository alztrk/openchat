import 'dart:convert';

final class SupportDiagnosticsBundle {
  const SupportDiagnosticsBundle({
    required this.generatedAt,
    required this.platform,
    required this.runtimeVersion,
    required this.historyStorageStatus,
    required this.serviceStatus,
    required this.databaseSchemaVersion,
  });

  final DateTime generatedAt;
  final String platform;
  final String runtimeVersion;
  final String historyStorageStatus;
  final String serviceStatus;
  final int databaseSchemaVersion;

  factory SupportDiagnosticsBundle.fromServiceHealth({
    required Object? value,
    required DateTime generatedAt,
    required String platform,
    required String runtimeVersion,
    required String historyStorageStatus,
  }) {
    if (value is! Map ||
        value['status'] != 'ready' ||
        value['schema_version'] is! int) {
      throw const FormatException(
        'The local service health response was invalid.',
      );
    }
    final schemaVersion = value['schema_version'] as int;
    if (schemaVersion < 1 || schemaVersion > 100) {
      throw const FormatException(
        'The local service health response was invalid.',
      );
    }
    if (!const <String>{
          'windows',
          'macos',
          'linux',
          'android',
          'ios',
        }.contains(platform) ||
        !RegExp(r'^\d+\.\d+\.\d+').hasMatch(runtimeVersion) ||
        !const <String>{
          'loading',
          'available',
          'unavailable',
          'corrupt',
          'backupUnavailable',
        }.contains(historyStorageStatus)) {
      throw ArgumentError('Support diagnostics metadata was invalid.');
    }
    return SupportDiagnosticsBundle(
      generatedAt: generatedAt.toUtc(),
      platform: platform,
      runtimeVersion: runtimeVersion,
      historyStorageStatus: historyStorageStatus,
      serviceStatus: 'ready',
      databaseSchemaVersion: schemaVersion,
    );
  }

  Map<String, Object?> toJson() => <String, Object?>{
    'formatVersion': 1,
    'application': 'OpenChat',
    'generatedAtUtc': generatedAt.toIso8601String(),
    'runtime': <String, Object?>{
      'platform': platform,
      'dartVersion': runtimeVersion,
    },
    'storage': <String, Object?>{
      'historyStatus': historyStorageStatus,
      'serviceStatus': serviceStatus,
      'databaseSchemaVersion': databaseSchemaVersion,
    },
  };

  String encode() => const JsonEncoder.withIndent('  ').convert(toJson());
}
