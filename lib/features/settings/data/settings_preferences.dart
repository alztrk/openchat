import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:openchat/features/chat/domain/default_model_preference.dart';
import 'package:openchat/features/chat/domain/history_search_result.dart';
import 'package:shared_preferences/shared_preferences.dart';

enum ToolPermissionMode {
  requireApproval,
  approveSafeOperations,
  fullAccess;

  String get serviceValue => switch (this) {
    ToolPermissionMode.requireApproval => 'require_approval',
    ToolPermissionMode.approveSafeOperations => 'approve_safe_operations',
    ToolPermissionMode.fullAccess => 'full_access',
  };
}

enum ToolPermissionRule {
  inherit,
  ask,
  allow,
  deny;

  String get storageValue => switch (this) {
    ToolPermissionRule.inherit => 'inherit',
    ToolPermissionRule.ask => 'ask',
    ToolPermissionRule.allow => 'allow',
    ToolPermissionRule.deny => 'deny',
  };

  String? get serviceValue => switch (this) {
    ToolPermissionRule.inherit => null,
    ToolPermissionRule.ask => 'ask',
    ToolPermissionRule.allow => 'allow',
    ToolPermissionRule.deny => 'deny',
  };

  static ToolPermissionRule? fromStorageValue(Object? value) => switch (value) {
    'ask' => ToolPermissionRule.ask,
    'allow' => ToolPermissionRule.allow,
    'deny' => ToolPermissionRule.deny,
    _ => null,
  };
}

const projectToolRuleNames = <String>{
  'list_files',
  'search_files',
  'read_file',
  'write_file',
  'edit_file',
  'get_file_info',
  'execute_command',
  'send_terminal_input',
  'git_status',
  'git_diff',
  'git_history',
  'web_search',
  'read_url_content',
  'delegate_task',
  'run_project_task',
};

bool _isProjectToolRuleName(String name) {
  if (projectToolRuleNames.contains(name)) return true;
  const prefix = 'mcp__';
  const suffix = '__*';
  if (!name.startsWith(prefix) || !name.endsWith(suffix)) return false;
  final serverId = name.substring(prefix.length, name.length - suffix.length);
  return RegExp(r'^[a-z0-9_]{1,24}$').hasMatch(serverId);
}

enum ConversationWidthPreference {
  narrow(760),
  normal(920),
  wide(1120);

  const ConversationWidthPreference(this.maxWidth);

  final double maxWidth;
}

enum ConversationTextSizePreference {
  small(0.85),
  normal(1),
  large(1.25);

  const ConversationTextSizePreference(this.scale);

  final double scale;
}

enum AppFontPreference {
  manrope('Manrope'),
  segoeUi('Segoe UI'),
  georgia('Georgia');

  const AppFontPreference(this.familyName);

  final String familyName;
}

class SettingsPreferences {
  SettingsPreferences(this._preferences);

  static const _themeModeKey = 'appearance.theme_mode';
  static const _localeKey = 'appearance.locale';
  static const _sharedInstructionsKey = 'chat.shared_instructions';
  static const _chatGptFastModeKey = 'chatgpt.fast_mode';
  static const _toolPermissionModeKey = 'tools.permission_mode';
  static const _projectToolPermissionPrefix = 'tools.project_rules.';
  static const _conversationWidthKey = 'appearance.conversation_width';
  static const _conversationTextSizeKey = 'appearance.conversation_text_size';
  static const _appFontKey = 'appearance.conversation_font';
  static const _defaultModelKey = 'models.default_model';
  static const _hiddenModelKeysKey = 'models.hidden_keys';
  static const _collapsedSidebarSectionsKey = 'chat.collapsed_sidebar_sections';
  static const _savedHistorySearchesKey = 'chat.saved_history_searches';
  static const _localModelDirectoryPrefix = 'models.local_engine_directory.';
  static const maxSharedInstructionsCharacters = 4096;

  final SharedPreferencesAsync _preferences;

  Future<List<SavedHistorySearch>> readSavedHistorySearches() async {
    final encoded = await _preferences.getString(_savedHistorySearchesKey);
    if (encoded == null) return const <SavedHistorySearch>[];
    final decoded = jsonDecode(encoded);
    if (decoded is! List<Object?> || decoded.length > 20) {
      throw const FormatException('Saved history searches are invalid.');
    }
    final searches = decoded
        .map(SavedHistorySearch.fromJson)
        .toList(growable: false);
    final names = searches.map((search) => search.name.toLowerCase()).toSet();
    if (names.length != searches.length) {
      throw const FormatException('Saved history searches are invalid.');
    }
    return List<SavedHistorySearch>.unmodifiable(searches);
  }

  Future<void> writeSavedHistorySearches(
    List<SavedHistorySearch> searches,
  ) async {
    if (searches.length > 20 ||
        searches.map((search) => search.name.toLowerCase()).toSet().length !=
            searches.length) {
      throw ArgumentError.value(
        searches,
        'searches',
        'Saved history search names must be unique and limited to 20 entries.',
      );
    }
    for (final search in searches) {
      SavedHistorySearch.fromJson(search.toJson());
    }
    await _preferences.setString(
      _savedHistorySearchesKey,
      jsonEncode(searches.map((search) => search.toJson()).toList()),
    );
  }

  Future<ThemeMode?> readThemeMode() async {
    final value = await _preferences.getString(_themeModeKey);
    return switch (value) {
      'system' => ThemeMode.system,
      'light' => ThemeMode.light,
      'dark' => ThemeMode.dark,
      _ => null,
    };
  }

  Future<void> writeThemeMode(ThemeMode mode) {
    return _preferences.setString(_themeModeKey, mode.name);
  }

  Future<ConversationWidthPreference> readConversationWidth() async {
    final value = await _preferences.getString(_conversationWidthKey);
    return switch (value) {
      null || 'normal' => ConversationWidthPreference.normal,
      'narrow' => ConversationWidthPreference.narrow,
      'wide' => ConversationWidthPreference.wide,
      _ => throw const FormatException(
        'The saved conversation width preference is invalid.',
      ),
    };
  }

  Future<void> writeConversationWidth(ConversationWidthPreference width) {
    return _preferences.setString(_conversationWidthKey, width.name);
  }

  Future<ConversationTextSizePreference> readConversationTextSize() async {
    final value = await _preferences.getString(_conversationTextSizeKey);
    return switch (value) {
      null || 'normal' => ConversationTextSizePreference.normal,
      'small' => ConversationTextSizePreference.small,
      'large' => ConversationTextSizePreference.large,
      _ => throw const FormatException(
        'The saved conversation text size preference is invalid.',
      ),
    };
  }

  Future<void> writeConversationTextSize(ConversationTextSizePreference size) {
    return _preferences.setString(_conversationTextSizeKey, size.name);
  }

  Future<AppFontPreference> readAppFont() async {
    final value = await _preferences.getString(_appFontKey);
    return switch (value) {
      null || 'manrope' => AppFontPreference.manrope,
      'segoeUi' => AppFontPreference.segoeUi,
      'georgia' => AppFontPreference.georgia,
      _ => throw const FormatException(
        'The saved app font preference is invalid.',
      ),
    };
  }

  Future<void> writeAppFont(AppFontPreference font) {
    return _preferences.setString(_appFontKey, font.name);
  }

  Future<Locale?> readLocale() async {
    final value = await _preferences.getString(_localeKey);
    return switch (value) {
      null || 'system' => null,
      'en' => const Locale('en'),
      'tr' => const Locale('tr'),
      'es' => const Locale('es'),
      'de' => const Locale('de'),
      'fr' => const Locale('fr'),
      _ => null,
    };
  }

  Future<void> writeLocale(Locale? locale) {
    final value = switch (locale?.languageCode) {
      null => 'system',
      'en' => 'en',
      'tr' => 'tr',
      'es' => 'es',
      'de' => 'de',
      'fr' => 'fr',
      final languageCode => throw ArgumentError.value(
        languageCode,
        'locale',
        'Only the supported English, Turkish, Spanish, German, and French locales can be saved.',
      ),
    };
    return _preferences.setString(_localeKey, value);
  }

  Future<String> readSharedInstructions() async {
    return await _preferences.getString(_sharedInstructionsKey) ?? '';
  }

  Future<void> writeSharedInstructions(String instructions) {
    if (instructions.length > maxSharedInstructionsCharacters) {
      throw ArgumentError.value(
        instructions.length,
        'instructions',
        'Shared instructions must not exceed 4096 characters.',
      );
    }
    if (instructions.trim().isEmpty) {
      return _preferences.remove(_sharedInstructionsKey);
    }
    return _preferences.setString(_sharedInstructionsKey, instructions);
  }

  Future<bool> readChatGptFastMode() async {
    return await _preferences.getBool(_chatGptFastModeKey) ?? false;
  }

  Future<void> writeChatGptFastMode(bool enabled) {
    return _preferences.setBool(_chatGptFastModeKey, enabled);
  }

  Future<ToolPermissionMode> readToolPermissionMode() async {
    final value = await _preferences.getString(_toolPermissionModeKey);
    return switch (value) {
      null || 'require_approval' => ToolPermissionMode.requireApproval,
      'approve_safe_operations' => ToolPermissionMode.approveSafeOperations,
      'full_access' => ToolPermissionMode.fullAccess,
      _ => throw const FormatException(
        'The saved tool permission mode is invalid.',
      ),
    };
  }

  Future<void> writeToolPermissionMode(ToolPermissionMode mode) {
    return _preferences.setString(_toolPermissionModeKey, mode.serviceValue);
  }

  Future<Map<String, ToolPermissionRule>> readProjectToolPermissionRules(
    String projectId,
  ) async {
    final key =
        '$_projectToolPermissionPrefix${_validatedProjectId(projectId)}';
    final raw = await _preferences.getString(key);
    if (raw == null) return const <String, ToolPermissionRule>{};
    final Object? decoded;
    try {
      decoded = jsonDecode(raw);
    } on FormatException {
      throw const FormatException(
        'Saved project tool permissions could not be read.',
      );
    }
    if (decoded is! Map) {
      throw const FormatException(
        'Saved project tool permissions have an invalid shape.',
      );
    }
    if (decoded.length > 32) {
      throw const FormatException(
        'Saved project tool permissions exceed the supported limit.',
      );
    }
    final rules = <String, ToolPermissionRule>{};
    for (final entry in decoded.entries) {
      final toolName = entry.key;
      if (toolName is! String ||
          !_isProjectToolRuleName(toolName) ||
          entry.value == 'inherit') {
        throw const FormatException(
          'Saved project tool permissions contain an invalid rule.',
        );
      }
      final rule = ToolPermissionRule.fromStorageValue(entry.value);
      if (rule == null) {
        throw const FormatException(
          'Saved project tool permissions contain an invalid rule.',
        );
      }
      rules[toolName] = rule;
    }
    return rules;
  }

  Future<void> writeProjectToolPermissionRules(
    String projectId,
    Map<String, ToolPermissionRule> rules,
  ) async {
    final normalizedProjectId = _validatedProjectId(projectId);
    final storedRules = <String, String>{};
    for (final entry in rules.entries) {
      if (!_isProjectToolRuleName(entry.key)) {
        throw ArgumentError.value(
          entry.key,
          'rules',
          'The project tool rule name is unsupported.',
        );
      }
      if (entry.value != ToolPermissionRule.inherit) {
        storedRules[entry.key] = entry.value.storageValue;
      }
    }
    final key = '$_projectToolPermissionPrefix$normalizedProjectId';
    if (storedRules.isEmpty) {
      await _preferences.remove(key);
      return;
    }
    await _preferences.setString(key, jsonEncode(storedRules));
  }

  Future<DefaultModelPreference?> readDefaultModel() async {
    final raw = await _preferences.getString(_defaultModelKey);
    if (raw == null || raw.trim().isEmpty) return null;
    try {
      final decoded = jsonDecode(raw);
      if (decoded is Map<String, Object?>) {
        return DefaultModelPreference.fromJson(decoded);
      }
      if (decoded is Map) {
        return DefaultModelPreference.fromJson(
          decoded.map((k, v) => MapEntry(k.toString(), v)),
        );
      }
      return null;
    } on Object {
      return null;
    }
  }

  Future<void> writeDefaultModel(DefaultModelPreference? preference) {
    if (preference == null) {
      return _preferences.remove(_defaultModelKey);
    }
    return _preferences.setString(
      _defaultModelKey,
      jsonEncode(preference.toJson()),
    );
  }

  Future<Set<String>> readHiddenModelKeys() async {
    final raw = await _preferences.getString(_hiddenModelKeysKey);
    if (raw == null || raw.trim().isEmpty) return const <String>{};
    try {
      final decoded = jsonDecode(raw);
      if (decoded is List) {
        return decoded.whereType<String>().toSet();
      }
      return const <String>{};
    } on Object {
      return const <String>{};
    }
  }

  Future<void> writeHiddenModelKeys(Set<String> keys) {
    if (keys.isEmpty) {
      return _preferences.remove(_hiddenModelKeysKey);
    }
    return _preferences.setString(
      _hiddenModelKeysKey,
      jsonEncode(keys.toList(growable: false)),
    );
  }

  Future<String?> readLocalModelDirectory(String engineId) async {
    final value = await _preferences.getString(
      '$_localModelDirectoryPrefix${_validatedLocalEngineId(engineId)}',
    );
    final path = value?.trim();
    return path == null || path.isEmpty ? null : path;
  }

  Future<void> writeLocalModelDirectory(String engineId, String? path) {
    final key =
        '$_localModelDirectoryPrefix${_validatedLocalEngineId(engineId)}';
    final normalizedPath = path?.trim();
    if (normalizedPath == null || normalizedPath.isEmpty) {
      return _preferences.remove(key);
    }
    if (normalizedPath.contains('\u0000')) {
      throw ArgumentError.value(
        path,
        'path',
        'The model directory is invalid.',
      );
    }
    return _preferences.setString(key, normalizedPath);
  }

  Future<Set<String>> readCollapsedSidebarSections() async {
    final values = await _preferences.getStringList(
      _collapsedSidebarSectionsKey,
    );
    if (values == null) return const <String>{};
    const supported = <String>{'pinned', 'projects', 'chats', 'archived'};
    return values.where(supported.contains).toSet();
  }

  Future<void> writeCollapsedSidebarSections(Set<String> sections) {
    const supported = <String>{'pinned', 'projects', 'chats', 'archived'};
    if (!supported.containsAll(sections)) {
      throw ArgumentError.value(
        sections,
        'sections',
        'A sidebar section is not supported.',
      );
    }
    return _preferences.setStringList(
      _collapsedSidebarSectionsKey,
      sections.toList(growable: false),
    );
  }

  String _validatedLocalEngineId(String engineId) => switch (engineId) {
    'llama_cpp' || 'exllama' || 'vllm' => engineId,
    _ => throw ArgumentError.value(
      engineId,
      'engineId',
      'The local engine is not supported.',
    ),
  };

  String _validatedProjectId(String projectId) {
    final normalized = projectId.trim();
    if (normalized.isEmpty ||
        normalized.length > 128 ||
        normalized.contains('\u0000')) {
      throw ArgumentError.value(
        projectId,
        'projectId',
        'The project identifier is invalid.',
      );
    }
    return normalized;
  }
}
