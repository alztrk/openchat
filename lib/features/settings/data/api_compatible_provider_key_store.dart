import 'package:flutter_secure_storage/flutter_secure_storage.dart';

import 'package:openchat/features/settings/data/api_key_format.dart';

class ApiCompatibleProviderKeyStatus {
  const ApiCompatibleProviderKeyStatus({
    required this.isConfigured,
    this.keySuffix,
  });

  final bool isConfigured;
  final String? keySuffix;
}

class ApiCompatibleProviderKeyStore {
  ApiCompatibleProviderKeyStore(this._secureStorage);

  static const providerIds = <String>{
    'gemini',
    'groq',
    'cerebras',
    'openrouter',
    'mistral',
  };
  static const _storageKeyPrefix = 'openchat.compatible_provider.api_key.';

  final FlutterSecureStorage _secureStorage;

  Future<String?> readApiKey(String providerId) async {
    final key = _storageKey(providerId);
    try {
      final value = await _secureStorage.read(key: key);
      return value?.trim().isEmpty == true ? null : value;
    } on Exception {
      throw const ApiCompatibleProviderKeyStorageException();
    }
  }

  Future<ApiCompatibleProviderKeyStatus> readStatus(String providerId) async {
    final value = await readApiKey(providerId);
    return ApiCompatibleProviderKeyStatus(
      isConfigured: value != null,
      keySuffix: value == null || value.length < 4
          ? null
          : value.substring(value.length - 4),
    );
  }

  Future<Set<String>> readConfiguredProviderIds() async {
    try {
      final values = await _secureStorage.readAll();
      return Set<String>.unmodifiable(
        providerIds.where((providerId) {
          final value = values['$_storageKeyPrefix$providerId'];
          return value != null && value.trim().isNotEmpty;
        }),
      );
    } on Exception {
      throw const ApiCompatibleProviderKeyStorageException();
    }
  }

  Future<void> saveApiKey(String providerId, String input) async {
    final key = input.trim();
    final storageKey = _storageKey(providerId);
    if (!ApiKeyFormat.isValid(providerId, key)) {
      throw const InvalidApiCompatibleProviderKeyException();
    }
    try {
      await _secureStorage.write(key: storageKey, value: key);
    } on Exception {
      throw const ApiCompatibleProviderKeyStorageException();
    }
  }

  Future<void> deleteApiKey(String providerId) async {
    final key = _storageKey(providerId);
    try {
      await _secureStorage.delete(key: key);
    } on Exception {
      throw const ApiCompatibleProviderKeyStorageException();
    }
  }

  String _storageKey(String providerId) {
    if (!providerIds.contains(providerId)) {
      throw const InvalidApiCompatibleProviderException();
    }
    return '$_storageKeyPrefix$providerId';
  }
}

class InvalidApiCompatibleProviderException implements Exception {
  const InvalidApiCompatibleProviderException();
}

class InvalidApiCompatibleProviderKeyException implements Exception {
  const InvalidApiCompatibleProviderKeyException();
}

class ApiCompatibleProviderKeyStorageException implements Exception {
  const ApiCompatibleProviderKeyStorageException();
}
