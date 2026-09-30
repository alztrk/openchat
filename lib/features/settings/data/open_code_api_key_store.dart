import 'package:flutter_secure_storage/flutter_secure_storage.dart';

import 'package:openchat/features/settings/data/api_key_format.dart';

class OpenCodeApiKeyStore {
  OpenCodeApiKeyStore(this._secureStorage);

  static const _storageKey = 'openchat.opencode.console_api_key';

  final FlutterSecureStorage _secureStorage;

  Future<String?> readApiKey() async {
    try {
      final value = await _secureStorage.read(key: _storageKey);
      return value?.trim().isEmpty == true ? null : value;
    } on Exception {
      throw const OpenCodeApiKeyStorageException();
    }
  }

  Future<String?> readKeySuffix() async {
    final key = await readApiKey();
    return key == null || key.length < 4 ? null : key.substring(key.length - 4);
  }

  Future<void> saveApiKey(String input) async {
    final key = input.trim();
    if (!ApiKeyFormat.isValid('opencode', key)) {
      throw const InvalidOpenCodeApiKeyException();
    }
    try {
      await _secureStorage.write(key: _storageKey, value: key);
    } on Exception {
      throw const OpenCodeApiKeyStorageException();
    }
  }

  Future<void> deleteApiKey() async {
    try {
      await _secureStorage.delete(key: _storageKey);
    } on Exception {
      throw const OpenCodeApiKeyStorageException();
    }
  }
}

class InvalidOpenCodeApiKeyException implements Exception {
  const InvalidOpenCodeApiKeyException();
}

class OpenCodeApiKeyStorageException implements Exception {
  const OpenCodeApiKeyStorageException();
}
