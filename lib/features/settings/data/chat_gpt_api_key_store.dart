import 'dart:convert';
import 'dart:math' as math;

import 'package:flutter_secure_storage/flutter_secure_storage.dart';

import '../domain/chat_gpt_api_key_connection.dart';

class ChatGptApiKeyStore {
  ChatGptApiKeyStore(this._secureStorage);

  static const _storageKeyPrefix = 'openchat.chatgpt.api_key.';
  static final _apiKeyPattern = RegExp(r'^sk-[A-Za-z0-9][A-Za-z0-9_-]*$');

  final FlutterSecureStorage _secureStorage;

  Future<List<ChatGptApiKeyConnection>> readConnections() async {
    final storedKeys = await _readStoredKeys();
    storedKeys.sort((left, right) => left.id.compareTo(right.id));
    return List<ChatGptApiKeyConnection>.unmodifiable(
      storedKeys.map(_toConnection),
    );
  }

  Future<List<ChatGptApiKeyConnection>> saveApiKey(String input) async {
    final apiKey = input.trim();
    if (apiKey.isEmpty) {
      throw const EmptyChatGptApiKeyException();
    }
    if (!_apiKeyPattern.hasMatch(apiKey)) {
      throw const InvalidChatGptApiKeyFormatException();
    }

    final storedKeys = await _readStoredKeys();
    if (storedKeys.any((storedKey) => storedKey.value == apiKey)) {
      throw const DuplicateChatGptApiKeyException();
    }

    final id = _createId();
    try {
      await _secureStorage.write(key: '$_storageKeyPrefix$id', value: apiKey);
    } on Exception {
      throw const ChatGptApiKeyStorageException();
    }

    storedKeys.add(_StoredChatGptApiKey(id: id, value: apiKey));
    storedKeys.sort((left, right) => left.id.compareTo(right.id));
    return List<ChatGptApiKeyConnection>.unmodifiable(
      storedKeys.map(_toConnection),
    );
  }

  Future<void> deleteApiKey(String id) async {
    if (!RegExp(r'^[A-Za-z0-9_-]{1,64}$').hasMatch(id)) {
      throw const InvalidChatGptApiKeyConnectionIdException();
    }
    try {
      await _secureStorage.delete(key: '$_storageKeyPrefix$id');
    } on Exception {
      throw const ChatGptApiKeyStorageException();
    }
  }

  Future<List<_StoredChatGptApiKey>> _readStoredKeys() async {
    try {
      final values = await _secureStorage.readAll();
      final storedKeys = <_StoredChatGptApiKey>[];

      for (final entry in values.entries) {
        if (!entry.key.startsWith(_storageKeyPrefix)) continue;

        final id = entry.key.substring(_storageKeyPrefix.length);
        if (id.isEmpty || entry.value.trim().isEmpty) {
          throw const ChatGptApiKeyStorageException();
        }

        storedKeys.add(_StoredChatGptApiKey(id: id, value: entry.value));
      }

      return storedKeys;
    } on ChatGptApiKeyStorageException {
      rethrow;
    } on Exception {
      throw const ChatGptApiKeyStorageException();
    }
  }

  ChatGptApiKeyConnection _toConnection(_StoredChatGptApiKey storedKey) {
    final keySuffix = storedKey.value.length < 4
        ? null
        : storedKey.value.substring(storedKey.value.length - 4);

    return ChatGptApiKeyConnection(id: storedKey.id, keySuffix: keySuffix);
  }

  String _createId() {
    final random = math.Random.secure();
    final bytes = List<int>.generate(
      16,
      (_) => random.nextInt(256),
      growable: false,
    );
    return base64Url.encode(bytes).replaceAll('=', '');
  }
}

class EmptyChatGptApiKeyException implements Exception {
  const EmptyChatGptApiKeyException();
}

class InvalidChatGptApiKeyFormatException implements Exception {
  const InvalidChatGptApiKeyFormatException();
}

class DuplicateChatGptApiKeyException implements Exception {
  const DuplicateChatGptApiKeyException();
}

class ChatGptApiKeyStorageException implements Exception {
  const ChatGptApiKeyStorageException();
}

class InvalidChatGptApiKeyConnectionIdException implements Exception {
  const InvalidChatGptApiKeyConnectionIdException();
}

class _StoredChatGptApiKey {
  const _StoredChatGptApiKey({required this.id, required this.value});

  final String id;
  final String value;
}
