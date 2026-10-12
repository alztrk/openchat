import 'dart:convert';
import 'dart:math';

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

enum CustomProviderProtocol { openAi, anthropic }

class CustomCompatibleProvider {
  const CustomCompatibleProvider({
    required this.id,
    required this.name,
    required this.baseUrl,
    required this.protocol,
  });

  final String id;
  final String name;
  final String baseUrl;
  final CustomProviderProtocol protocol;

  Map<String, Object?> toJson() => <String, Object?>{
    'id': id,
    'name': name,
    'baseUrl': baseUrl,
    'protocol': protocol.name,
  };

  factory CustomCompatibleProvider.fromJson(Object? value) {
    if (value is! Map<String, Object?>) {
      throw const FormatException('A custom provider record was invalid.');
    }
    final id = value['id'];
    final name = value['name'];
    final baseUrl = value['baseUrl'];
    final protocol = value['protocol'];
    if (id is! String ||
        !_customProviderId.hasMatch(id) ||
        name is! String ||
        name.trim().isEmpty ||
        name.length > 80 ||
        baseUrl is! String ||
        !_isValidBaseUrl(baseUrl) ||
        protocol is! String) {
      throw const FormatException('A custom provider record was invalid.');
    }
    return CustomCompatibleProvider(
      id: id,
      name: name.trim(),
      baseUrl: baseUrl,
      protocol: switch (protocol) {
        'openAi' => CustomProviderProtocol.openAi,
        'anthropic' => CustomProviderProtocol.anthropic,
        _ => throw const FormatException(
          'A custom provider protocol was invalid.',
        ),
      },
    );
  }
}

class ApiCompatibleProviderKeyStore {
  ApiCompatibleProviderKeyStore(this._secureStorage);

  static const providerIds = <String>{
    'gemini',
    'groq',
    'cerebras',
    'openrouter',
    'mistral',
    'deepseek',
    'xai',
    'perplexity',
    'together',
    'sambanova',
  };
  static bool isCustomProviderId(String providerId) =>
      _customProviderId.hasMatch(providerId);

  static bool isApiKeyProviderId(String providerId) =>
      providerIds.contains(providerId) || isCustomProviderId(providerId);
  static const _storageKeyPrefix = 'openchat.compatible_provider.api_key.';
  static const _customProviderKeyPrefix = 'openchat.custom_provider.api_key.';
  static const _customProviderConfigPrefix = 'openchat.custom_provider.config.';

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

  Future<List<CustomCompatibleProvider>> readCustomProviders() async {
    try {
      final values = await _secureStorage.readAll();
      return values.entries
          .where((entry) => entry.key.startsWith(_customProviderConfigPrefix))
          .map((entry) {
            final decoded = jsonDecode(entry.value);
            final provider = CustomCompatibleProvider.fromJson(decoded);
            final storageId = entry.key.substring(
              _customProviderConfigPrefix.length,
            );
            if (storageId != provider.id) {
              throw const FormatException(
                'A custom provider record was invalid.',
              );
            }
            return provider;
          })
          .toList(growable: false);
    } on ApiCompatibleProviderKeyStorageException {
      rethrow;
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
      return Set<String>.unmodifiable(<String>{
        ...providerIds.where((providerId) {
          final value = values['$_storageKeyPrefix$providerId'];
          return value != null && value.trim().isNotEmpty;
        }),
        ...values.keys
            .where((key) => key.startsWith(_customProviderConfigPrefix))
            .map((key) => key.substring(_customProviderConfigPrefix.length))
            .where(_customProviderId.hasMatch),
      });
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

  Future<CustomCompatibleProvider> saveCustomProvider({
    String? id,
    required String name,
    required String baseUrl,
    required CustomProviderProtocol protocol,
    required String apiKey,
  }) async {
    final normalizedName = name.trim();
    final normalizedUrl = _normalizeBaseUrl(baseUrl);
    final normalizedKey = apiKey.trim();
    if (normalizedName.isEmpty ||
        normalizedName.length > 80 ||
        normalizedUrl == null ||
        normalizedKey.length > ApiKeyFormat.maximumLength ||
        normalizedKey.contains(RegExp(r'[\x00-\x1F\x7F]'))) {
      throw const InvalidApiCompatibleProviderKeyException();
    }
    final providerId = id ?? _newCustomProviderId();
    if (!_customProviderId.hasMatch(providerId)) {
      throw const InvalidApiCompatibleProviderException();
    }
    final provider = CustomCompatibleProvider(
      id: providerId,
      name: normalizedName,
      baseUrl: normalizedUrl,
      protocol: protocol,
    );
    try {
      if (normalizedKey.isNotEmpty) {
        await _secureStorage.write(
          key: '$_customProviderKeyPrefix$providerId',
          value: normalizedKey,
        );
      }
      await _secureStorage.write(
        key: '$_customProviderConfigPrefix$providerId',
        value: jsonEncode(provider.toJson()),
      );
    } on Exception {
      throw const ApiCompatibleProviderKeyStorageException();
    }
    return provider;
  }

  Future<void> deleteCustomProvider(String providerId) async {
    if (!_customProviderId.hasMatch(providerId)) {
      throw const InvalidApiCompatibleProviderException();
    }
    try {
      await _secureStorage.delete(key: '$_customProviderKeyPrefix$providerId');
      await _secureStorage.delete(
        key: '$_customProviderConfigPrefix$providerId',
      );
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
    if (providerIds.contains(providerId)) {
      return '$_storageKeyPrefix$providerId';
    }
    if (_customProviderId.hasMatch(providerId)) {
      return '$_customProviderKeyPrefix$providerId';
    }
    throw const InvalidApiCompatibleProviderException();
  }
}

final _customProviderId = RegExp(r'^custom_[a-z0-9_]{12,64}$');

bool _isValidBaseUrl(String value) => _normalizeBaseUrl(value) == value;

String? _normalizeBaseUrl(String value) {
  final input = value.trim();
  if (input.isEmpty || input.length > 2048) return null;
  final uri = Uri.tryParse(input);
  if (uri == null ||
      !const <String>{'http', 'https'}.contains(uri.scheme.toLowerCase()) ||
      uri.host.isEmpty ||
      uri.userInfo.isNotEmpty ||
      uri.hasQuery ||
      uri.hasFragment) {
    return null;
  }
  final path = uri.path.replaceFirst(RegExp(r'/+$'), '');
  return uri.replace(path: path).toString().replaceFirst(RegExp(r'/+$'), '');
}

String _newCustomProviderId() {
  final random = Random.secure();
  final suffix = List<int>.generate(
    16,
    (_) => random.nextInt(256),
  ).map((byte) => byte.toRadixString(16).padLeft(2, '0')).join();
  return 'custom_$suffix';
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
