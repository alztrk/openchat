import 'package:openchat/platform/windows/openchat_service_client.dart';

class ProfileArchiveResult {
  const ProfileArchiveResult({
    required this.conversationCount,
    required this.messageCount,
    required this.attachmentCount,
    required this.attachmentBytes,
    required this.requiresRestart,
  });

  final int conversationCount;
  final int messageCount;
  final int attachmentCount;
  final int attachmentBytes;
  final bool requiresRestart;

  factory ProfileArchiveResult.fromJson(Map<String, Object?> json) {
    return ProfileArchiveResult(
      conversationCount: _requiredNonNegativeInt(json, 'conversationCount'),
      messageCount: _requiredNonNegativeInt(json, 'messageCount'),
      attachmentCount: _requiredNonNegativeInt(json, 'attachmentCount'),
      attachmentBytes: _requiredNonNegativeInt(json, 'attachmentBytes'),
      requiresRestart: _requiredBool(json, 'requiresRestart'),
    );
  }
}

class ProfileArchiveRepository {
  const ProfileArchiveRepository(this._serviceClient);

  static const _archiveTimeout = Duration(hours: 3);

  final OpenChatServiceClient _serviceClient;

  Future<ProfileArchiveResult> export({
    required String path,
    required String passphrase,
  }) async {
    final result = await _serviceClient.call(
      'profile.archive.export',
      params: <String, Object?>{'path': path, 'passphrase': passphrase},
      timeout: _archiveTimeout,
    );
    return ProfileArchiveResult.fromJson(result);
  }

  Future<ProfileArchiveResult> prepareRestore({
    required String path,
    required String passphrase,
    required int chatSchemaVersion,
  }) async {
    final result = await _serviceClient.call(
      'profile.archive.prepare_restore',
      params: <String, Object?>{
        'path': path,
        'passphrase': passphrase,
        'chatSchemaVersion': chatSchemaVersion,
      },
      timeout: _archiveTimeout,
    );
    return ProfileArchiveResult.fromJson(result);
  }
}

int _requiredNonNegativeInt(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value is! int || value < 0) {
    throw FormatException('The profile backup field "$key" is invalid.');
  }
  return value;
}

bool _requiredBool(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value is! bool) {
    throw FormatException('The profile backup field "$key" is invalid.');
  }
  return value;
}
