import 'package:openchat/platform/windows/openchat_service_client.dart';

enum ConversationArchiveConflictPolicy {
  skipExisting('skip_existing'),
  importAsCopy('import_as_copy');

  const ConversationArchiveConflictPolicy(this.rpcValue);

  final String rpcValue;
}

class ConversationArchiveSummary {
  const ConversationArchiveSummary({
    required this.conversationCount,
    required this.messageCount,
    required this.attachmentCount,
    required this.attachmentBytes,
    required this.duplicateConversationCount,
    required this.createdAtUnixMs,
    required this.formatVersion,
  });

  final int conversationCount;
  final int messageCount;
  final int attachmentCount;
  final int attachmentBytes;
  final int duplicateConversationCount;
  final int createdAtUnixMs;
  final int formatVersion;

  factory ConversationArchiveSummary.fromJson(Map<String, Object?> json) {
    return ConversationArchiveSummary(
      conversationCount: _requiredNonNegativeInt(json, 'conversationCount'),
      messageCount: _requiredNonNegativeInt(json, 'messageCount'),
      attachmentCount: _requiredNonNegativeInt(json, 'attachmentCount'),
      attachmentBytes: _requiredNonNegativeInt(json, 'attachmentBytes'),
      duplicateConversationCount: _requiredNonNegativeInt(
        json,
        'duplicateConversationCount',
      ),
      createdAtUnixMs: _requiredNonNegativeInt(json, 'createdAtUnixMs'),
      formatVersion: _requiredPositiveInt(json, 'formatVersion'),
    );
  }
}

class ConversationArchiveResult {
  const ConversationArchiveResult({
    required this.conversationCount,
    required this.messageCount,
    required this.attachmentCount,
    required this.attachmentBytes,
    required this.duplicateConversationCount,
  });

  final int conversationCount;
  final int messageCount;
  final int attachmentCount;
  final int attachmentBytes;
  final int duplicateConversationCount;

  factory ConversationArchiveResult.fromJson(Map<String, Object?> json) {
    return ConversationArchiveResult(
      conversationCount: _requiredNonNegativeInt(
        json,
        'restoredConversationCount',
        alternative: 'conversationCount',
      ),
      messageCount: _requiredNonNegativeInt(
        json,
        'restoredMessageCount',
        alternative: 'messageCount',
      ),
      attachmentCount: _requiredNonNegativeInt(
        json,
        'restoredAttachmentCount',
        alternative: 'attachmentCount',
      ),
      attachmentBytes: _optionalNonNegativeInt(json, 'attachmentBytes') ?? 0,
      duplicateConversationCount:
          _optionalNonNegativeInt(json, 'duplicateConversationCount') ?? 0,
    );
  }
}

class ConversationArchiveRepository {
  const ConversationArchiveRepository(this._serviceClient);

  static const _archiveTimeout = Duration(minutes: 30);

  final OpenChatServiceClient _serviceClient;

  Future<ConversationArchiveResult> export({
    required List<String> conversationIds,
    required String path,
    required String passphrase,
  }) async {
    final result = await _serviceClient.call(
      'conversation.archive.export',
      params: <String, Object?>{
        'conversationIds': conversationIds,
        'path': path,
        'passphrase': passphrase,
      },
      timeout: _archiveTimeout,
    );
    return ConversationArchiveResult.fromJson(result);
  }

  Future<ConversationArchiveSummary> inspect({
    required String path,
    required String passphrase,
  }) async {
    final result = await _serviceClient.call(
      'conversation.archive.inspect',
      params: <String, Object?>{'path': path, 'passphrase': passphrase},
      timeout: _archiveTimeout,
    );
    return ConversationArchiveSummary.fromJson(result);
  }

  Future<ConversationArchiveResult> restore({
    required String path,
    required String passphrase,
    required ConversationArchiveConflictPolicy conflictPolicy,
  }) async {
    final result = await _serviceClient.call(
      'conversation.archive.restore',
      params: <String, Object?>{
        'path': path,
        'passphrase': passphrase,
        'conflictPolicy': conflictPolicy.rpcValue,
      },
      timeout: _archiveTimeout,
    );
    return ConversationArchiveResult.fromJson(result);
  }
}

int _requiredPositiveInt(Map<String, Object?> json, String key) {
  final value = _optionalNonNegativeInt(json, key);
  if (value == null || value == 0) {
    throw FormatException('The archive field "$key" is invalid.');
  }
  return value;
}

int _requiredNonNegativeInt(
  Map<String, Object?> json,
  String key, {
  String? alternative,
}) {
  final value =
      _optionalNonNegativeInt(json, key) ??
      (alternative == null ? null : _optionalNonNegativeInt(json, alternative));
  if (value == null) {
    throw FormatException('The archive field "$key" is invalid.');
  }
  return value;
}

int? _optionalNonNegativeInt(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value == null) return null;
  if (value is! int || value < 0) {
    throw FormatException('The archive field "$key" is invalid.');
  }
  return value;
}
