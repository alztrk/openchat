import 'package:flutter_secure_storage/flutter_secure_storage.dart';

class OpenCodeApiKeyStore {
  OpenCodeApiKeyStore(this._secureStorage);

  static const _storageKey = 'zihora.opencode.console_api_key';
  static const _maximumKeyLength = 4096;

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
    if (key.isEmpty ||
        key.length > _maximumKeyLength ||
        key.contains('\n') ||
        key.contains('\r')) {
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
