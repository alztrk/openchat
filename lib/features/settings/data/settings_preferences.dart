import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:openchat/features/chat/domain/default_model_preference.dart';
import 'package:openchat/features/chat/domain/history_search_result.dart';
import 'package:openchat/features/chat/domain/title_generation_preference.dart';
import 'package:openchat/features/settings/data/personal_memory_fact.dart';
import 'package:openchat/features/settings/domain/provider_verification_record.dart';
import 'package:openchat/features/settings/domain/provider_request_outcome_record.dart';
import 'package:openchat/features/settings/domain/connector_catalog_entry.dart';
import 'package:openchat/features/settings/domain/keyboard_shortcut_bindings.dart';
import 'package:shared_preferences/shared_preferences.dart';

enum ToolPermissionMode {
  plan,
  requireApproval,
  approveSafeOperations,
  fullAccess;

  String get serviceValue => switch (this) {
    ToolPermissionMode.plan => 'plan',
    ToolPermissionMode.requireApproval => 'require_approval',
    ToolPermissionMode.approveSafeOperations => 'approve_safe_operations',
    ToolPermissionMode.fullAccess => 'full_access',
  };

  String get preferenceValue => switch (this) {
    ToolPermissionMode.plan => throw ArgumentError.value(
      this,
      'mode',
      'Plan mode is a per-request composer mode and cannot be saved.',
    ),
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
const _maxProjectToolPermissionRules = 1080;

bool _isProjectToolRuleName(String name) {
  if (projectToolRuleNames.contains(name)) return true;
  const taskPrefix = 'run_project_task__';
  if (name.startsWith(taskPrefix)) {
    return RegExp(r'^[a-z0-9_-]{1,64}$')
        .hasMatch(name.substring(taskPrefix.length));
  }
  const configuredToolPrefix = 'project_tool__';
  if (name.startsWith(configuredToolPrefix)) {
    return RegExp(r'^[a-z0-9_-]{1,48}$')
        .hasMatch(name.substring(configuredToolPrefix.length));
  }
  const prefix = 'mcp__';
  if (!name.startsWith(prefix) || name.length > 64) return false;
  final suffix = name.substring(prefix.length);
  if (suffix.endsWith('__*')) {
    return RegExp(r'^[a-z0-9_]{1,24}$')
        .hasMatch(suffix.substring(0, suffix.length - 3));
  }
  return RegExp(r'^[a-z0-9_]{1,24}__[A-Za-z0-9_-]+$').hasMatch(suffix);
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
  sourceSans3('Source Sans 3'),
  manrope('Manrope'),
  segoeUi('Segoe UI'),
  georgia('Georgia');

  const AppFontPreference(this.familyName);

  final String familyName;
}

@immutable
class ConversationNotificationPreferences {
  const ConversationNotificationPreferences({
    this.responses = true,
    this.questions = true,
    this.responsePreview = true,
    this.quickReply = true,
  });

  final bool responses;
  final bool questions;
  final bool responsePreview;
  final bool quickReply;

  ConversationNotificationPreferences copyWith({
    bool? responses,
    bool? questions,
    bool? responsePreview,
    bool? quickReply,
  }) => ConversationNotificationPreferences(
    responses: responses ?? this.responses,
    questions: questions ?? this.questions,
    responsePreview: responsePreview ?? this.responsePreview,
    quickReply: quickReply ?? this.quickReply,
  );
}

class SettingsPreferences {
  SettingsPreferences(this._preferences);

  static const _themeModeKey = 'appearance.theme_mode';
  static const _localeKey = 'appearance.locale';
  static const _sharedInstructionsKey = 'chat.shared_instructions';
  static const _keyboardShortcutsKey = 'chat.keyboard_shortcuts';
  static const _chatGptFastModeKey = 'chatgpt.fast_mode';
  static const _notificationResponsesKey = 'notifications.responses';
  static const _notificationQuestionsKey = 'notifications.questions';
  static const _notificationResponsePreviewKey =
      'notifications.response_preview';
  static const _notificationQuickReplyKey = 'notifications.quick_reply';
  static const _toolPermissionModeKey = 'tools.permission_mode';
  static const _projectToolPermissionPrefix = 'tools.project_rules.';
  static const _conversationWidthKey = 'appearance.conversation_width';
  static const _conversationTextSizeKey = 'appearance.conversation_text_size';
  static const _appFontKey = 'appearance.conversation_font';
  static const _defaultModelKey = 'models.default_model';
  static const _titleGenerationPreferenceKey = 'chat.title_generation';
  static const _projectDefaultModelPrefix = 'models.project_default.';
  static const _projectSkillsPrefix = 'project.skills.';
  static const _personalMemoryEnabledKey = 'privacy.personal_memory_enabled';
  static const _attachmentContentIndexEnabledKey =
      'privacy.attachment_content_index_enabled';
  static const _personalMemoryFactsKey = 'privacy.personal_memory_facts';
  static const _personalMemoryExcludedConversationsKey =
      'privacy.personal_memory_excluded_conversations';
  static const _providerVerificationPrefix = 'providers.verification.';
  static const _providerRequestOutcomePrefix = 'providers.last_request.';
  static const _connectorCatalogEntriesKey = 'connectors.local_catalog';
  static const _hiddenModelKeysKey = 'models.hidden_keys';
  static const _modelContextLimitsKey = 'models.context_limits';
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
      null || 'sourceSans3' => AppFontPreference.sourceSans3,
      'manrope' => AppFontPreference.manrope,
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

  Future<List<ConnectorCatalogEntry>> readConnectorCatalogEntries() async {
    final raw = await _preferences.getString(_connectorCatalogEntriesKey);
    if (raw == null) return const <ConnectorCatalogEntry>[];
    if (utf8.encode(raw).length > 64 * 1024) {
      throw const FormatException('The connector catalog is too large.');
    }
    final Object? decoded;
    try {
      decoded = jsonDecode(raw);
    } on FormatException {
      throw const FormatException('The connector catalog is invalid.');
    }
    if (decoded is! List<Object?>) {
      throw const FormatException('The connector catalog is invalid.');
    }
    final entries = decoded
        .map(ConnectorCatalogEntry.fromJson)
        .toList(growable: false);
    try {
      validateConnectorCatalog(entries);
    } on ArgumentError {
      throw const FormatException('The connector catalog is invalid.');
    }
    return List<ConnectorCatalogEntry>.unmodifiable(entries);
  }

  Future<void> writeConnectorCatalogEntries(
    List<ConnectorCatalogEntry> entries,
  ) async {
    validateConnectorCatalog(entries);
    if (entries.isEmpty) {
      await _preferences.remove(_connectorCatalogEntriesKey);
      return;
    }
    await _preferences.setString(
      _connectorCatalogEntriesKey,
      jsonEncode(entries.map((entry) => entry.toJson()).toList()),
    );
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

  Future<KeyboardShortcutBindings> readKeyboardShortcuts() async {
    final encoded = await _preferences.getString(_keyboardShortcutsKey);
    if (encoded == null) return KeyboardShortcutBindings.defaults;
    return KeyboardShortcutBindings.fromJson(jsonDecode(encoded));
  }

  Future<void> writeKeyboardShortcuts(KeyboardShortcutBindings bindings) async {
    await _preferences.setString(
      _keyboardShortcutsKey,
      jsonEncode(bindings.toJson()),
    );
  }

  Future<bool> readChatGptFastMode() async {
    return await _preferences.getBool(_chatGptFastModeKey) ?? false;
  }

  Future<void> writeChatGptFastMode(bool enabled) {
    return _preferences.setBool(_chatGptFastModeKey, enabled);
  }

  Future<ConversationNotificationPreferences>
  readConversationNotificationPreferences() async =>
      ConversationNotificationPreferences(
        responses:
            await _preferences.getBool(_notificationResponsesKey) ?? true,
        questions:
            await _preferences.getBool(_notificationQuestionsKey) ?? true,
        responsePreview:
            await _preferences.getBool(_notificationResponsePreviewKey) ?? true,
        quickReply:
            await _preferences.getBool(_notificationQuickReplyKey) ?? true,
      );

  Future<void> writeNotificationResponses(bool enabled) =>
      _preferences.setBool(_notificationResponsesKey, enabled);

  Future<void> writeNotificationQuestions(bool enabled) =>
      _preferences.setBool(_notificationQuestionsKey, enabled);

  Future<void> writeNotificationResponsePreview(bool enabled) =>
      _preferences.setBool(_notificationResponsePreviewKey, enabled);

  Future<void> writeNotificationQuickReply(bool enabled) =>
      _preferences.setBool(_notificationQuickReplyKey, enabled);

  Future<ToolPermissionMode> readToolPermissionMode() async {
    final value = await _preferences.getString(_toolPermissionModeKey);
    return switch (value) {
      null || 'require_approval' => ToolPermissionMode.requireApproval,
      'plan' => ToolPermissionMode.requireApproval,
      'approve_safe_operations' => ToolPermissionMode.approveSafeOperations,
      'full_access' => ToolPermissionMode.fullAccess,
      _ => throw const FormatException(
        'The saved tool permission mode is invalid.',
      ),
    };
  }

  Future<void> writeToolPermissionMode(ToolPermissionMode mode) {
    return _preferences.setString(_toolPermissionModeKey, mode.preferenceValue);
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
    if (decoded.length > _maxProjectToolPermissionRules) {
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
    if (storedRules.length > _maxProjectToolPermissionRules) {
      throw ArgumentError.value(
        rules,
        'rules',
        'The project tool rule count exceeds the supported limit.',
      );
    }
    final key = '$_projectToolPermissionPrefix$normalizedProjectId';
    if (storedRules.isEmpty) {
      await _preferences.remove(key);
      return;
    }
    await _preferences.setString(key, jsonEncode(storedRules));
  }

  Future<bool> readPersonalMemoryEnabled() async =>
      await _preferences.getBool(_personalMemoryEnabledKey) ?? false;

  Future<void> writePersonalMemoryEnabled(bool enabled) =>
      _preferences.setBool(_personalMemoryEnabledKey, enabled);

  Future<bool> readAttachmentContentIndexEnabled() async =>
      await _preferences.getBool(_attachmentContentIndexEnabledKey) ?? false;

  Future<void> writeAttachmentContentIndexEnabled(bool enabled) =>
      _preferences.setBool(_attachmentContentIndexEnabledKey, enabled);

  Future<void> clearPersonalMemory() async {
    await _preferences.remove(_personalMemoryEnabledKey);
    await _preferences.remove(_personalMemoryFactsKey);
  }

  Future<Set<String>> readPersonalMemoryExcludedConversationIds() async {
    final stored = await _preferences.getStringList(
      _personalMemoryExcludedConversationsKey,
    );
    if (stored == null) return const <String>{};
    if (stored.length > 10000 ||
        stored.any(
          (id) =>
              id.trim() != id ||
              id.isEmpty ||
              id.length > 128 ||
              id.startsWith('tmp_'),
        ) ||
        stored.toSet().length != stored.length) {
      throw const FormatException(
        'Personal memory conversation exclusions are invalid.',
      );
    }
    return Set<String>.unmodifiable(stored);
  }

  Future<void> writePersonalMemoryExcludedConversationIds(
    Set<String> conversationIds,
  ) async {
    if (conversationIds.length > 10000 ||
        conversationIds.any(
          (id) =>
              id.trim() != id ||
              id.isEmpty ||
              id.length > 128 ||
              id.startsWith('tmp_'),
        )) {
      throw ArgumentError.value(
        conversationIds,
        'conversationIds',
        'Personal memory exclusions must contain valid durable conversation IDs.',
      );
    }
    if (conversationIds.isEmpty) {
      await _preferences.remove(_personalMemoryExcludedConversationsKey);
      return;
    }
    await _preferences.setStringList(
      _personalMemoryExcludedConversationsKey,
      conversationIds.toList(growable: false),
    );
  }

  Future<List<PersonalMemoryFact>> readPersonalMemoryFacts() async {
    final raw = await _preferences.getString(_personalMemoryFactsKey);
    if (raw == null) return const <PersonalMemoryFact>[];
    final Object? decoded;
    try {
      decoded = jsonDecode(raw);
    } on FormatException {
      throw const FormatException('Saved personal memories are unreadable.');
    }
    if (decoded is! List<Object?> || decoded.length > maxPersonalMemoryFacts) {
      throw const FormatException('Saved personal memories are invalid.');
    }
    final facts = decoded
        .map(PersonalMemoryFact.fromJson)
        .toList(growable: false);
    if (facts.map((fact) => fact.id).toSet().length != facts.length) {
      throw const FormatException('Saved personal memories are invalid.');
    }
    return List<PersonalMemoryFact>.unmodifiable(facts);
  }

  Future<void> writePersonalMemoryFacts(List<PersonalMemoryFact> facts) async {
    if (facts.length > maxPersonalMemoryFacts ||
        facts.map((fact) => fact.id).toSet().length != facts.length) {
      throw ArgumentError.value(
        facts,
        'facts',
        'Personal memories must have unique IDs and be limited to $maxPersonalMemoryFacts items.',
      );
    }
    for (final fact in facts) {
      if (!RegExp(r'^[a-f0-9]{32}$').hasMatch(fact.id) ||
          fact.content.trim().isEmpty ||
          fact.content.trim() != fact.content ||
          fact.content.length > maxPersonalMemoryFactCharacters) {
        throw ArgumentError.value(
          fact,
          'facts',
          'A personal memory has an invalid ID or content.',
        );
      }
    }
    await _preferences.setString(
      _personalMemoryFactsKey,
      jsonEncode(facts.map((fact) => fact.toJson()).toList(growable: false)),
    );
  }

  Future<ProviderVerificationRecord?> readProviderVerificationRecord(
    String providerId,
  ) async {
    final raw = await _preferences.getString(
      '$_providerVerificationPrefix${_validatedProviderId(providerId)}',
    );
    if (raw == null) return null;
    final Object? decoded;
    try {
      decoded = jsonDecode(raw);
    } on FormatException {
      throw const FormatException(
        'A provider verification record was invalid.',
      );
    }
    return ProviderVerificationRecord.fromJson(decoded);
  }

  Future<void> writeProviderVerificationRecord(
    String providerId,
    ProviderVerificationRecord record,
  ) => _preferences.setString(
    '$_providerVerificationPrefix${_validatedProviderId(providerId)}',
    jsonEncode(record.toJson()),
  );

  Future<void> clearProviderVerificationRecord(String providerId) =>
      _preferences.remove(
        '$_providerVerificationPrefix${_validatedProviderId(providerId)}',
      );

  Future<ProviderVerificationRecord?> readProviderVerificationRecordForRoute({
    required String providerId,
    required String connectionId,
    String? workspaceId,
  }) async {
    final raw = await _preferences.getString(
      _providerVerificationRouteKey(
        providerId: providerId,
        connectionId: connectionId,
        workspaceId: workspaceId,
      ),
    );
    if (raw == null) return null;
    final Object? decoded;
    try {
      decoded = jsonDecode(raw);
    } on FormatException {
      throw const FormatException(
        'A provider route verification record was invalid.',
      );
    }
    return ProviderVerificationRecord.fromJson(decoded);
  }

  Future<void> writeProviderVerificationRecordForRoute({
    required String providerId,
    required String connectionId,
    required ProviderVerificationRecord record,
    String? workspaceId,
  }) => _preferences.setString(
    _providerVerificationRouteKey(
      providerId: providerId,
      connectionId: connectionId,
      workspaceId: workspaceId,
    ),
    jsonEncode(record.toJson()),
  );

  Future<void> clearProviderVerificationRecordForRoute({
    required String providerId,
    required String connectionId,
    String? workspaceId,
  }) => _preferences.remove(
    _providerVerificationRouteKey(
      providerId: providerId,
      connectionId: connectionId,
      workspaceId: workspaceId,
    ),
  );

  Future<ProviderRequestOutcomeRecord?> readProviderRequestOutcomeForRoute({
    required String providerId,
    required String connectionId,
    String? workspaceId,
  }) async {
    final raw = await _preferences.getString(
      _providerRequestOutcomeRouteKey(
        providerId: providerId,
        connectionId: connectionId,
        workspaceId: workspaceId,
      ),
    );
    if (raw == null) return null;
    final Object? decoded;
    try {
      decoded = jsonDecode(raw);
    } on FormatException {
      throw const FormatException('A provider request outcome was invalid.');
    }
    return ProviderRequestOutcomeRecord.fromJson(decoded);
  }

  Future<void> writeProviderRequestOutcomeForRoute({
    required String providerId,
    required String connectionId,
    required ProviderRequestOutcomeRecord record,
    String? workspaceId,
  }) => _preferences.setString(
    _providerRequestOutcomeRouteKey(
      providerId: providerId,
      connectionId: connectionId,
      workspaceId: workspaceId,
    ),
    jsonEncode(record.toJson()),
  );

  Future<void> clearProviderRequestOutcomeForRoute({
    required String providerId,
    required String connectionId,
    String? workspaceId,
  }) => _preferences.remove(
    _providerRequestOutcomeRouteKey(
      providerId: providerId,
      connectionId: connectionId,
      workspaceId: workspaceId,
    ),
  );

  Future<DefaultModelPreference?> readDefaultModel() async {
    final raw = await _preferences.getString(_defaultModelKey);
    return raw == null ? null : _decodeDefaultModelPreference(raw);
  }

  Future<TitleGenerationPreference> readTitleGenerationPreference() async {
    final raw = await _preferences.getString(_titleGenerationPreferenceKey);
    if (raw == null || raw.trim().isEmpty) {
      return const TitleGenerationPreference();
    }
    final decoded = jsonDecode(raw);
    if (decoded is! Map) {
      throw const FormatException('Title generation preferences are invalid.');
    }
    final values = <String, Object?>{};
    for (final entry in decoded.entries) {
      if (entry.key is! String) {
        throw const FormatException(
          'Title generation preferences are invalid.',
        );
      }
      values[entry.key as String] = entry.value;
    }
    return TitleGenerationPreference.fromJson(values);
  }

  Future<void> writeTitleGenerationPreference(
    TitleGenerationPreference preference,
  ) async {
    if ((preference.mode == TitleGenerationMode.conversation &&
            preference.model != null) ||
        (preference.mode == TitleGenerationMode.selectedModel &&
            preference.model == null)) {
      throw ArgumentError.value(
        preference,
        'preference',
        'A title model is required outside conversation-model mode.',
      );
    }
    await _preferences.setString(
      _titleGenerationPreferenceKey,
      jsonEncode(preference.toJson()),
    );
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

  Future<DefaultModelPreference?> readProjectDefaultModel(
    String projectId,
  ) async {
    final key = '$_projectDefaultModelPrefix${_validatedProjectId(projectId)}';
    final raw = await _preferences.getString(key);
    return raw == null ? null : _decodeDefaultModelPreference(raw);
  }

  DefaultModelPreference _decodeDefaultModelPreference(String raw) {
    if (raw.trim().isEmpty) {
      throw const FormatException(
        'The saved default model preference is empty.',
      );
    }
    final Object? decoded;
    try {
      decoded = jsonDecode(raw);
    } on FormatException {
      throw const FormatException(
        'The saved default model preference is malformed.',
      );
    }
    if (decoded is Map<String, Object?>) {
      return DefaultModelPreference.fromJson(decoded);
    }
    if (decoded is Map<Object?, Object?>) {
      final values = <String, Object?>{};
      for (final entry in decoded.entries) {
        if (entry.key case final String key) {
          values[key] = entry.value;
        } else {
          throw const FormatException(
            'The saved default model preference has an invalid field name.',
          );
        }
      }
      return DefaultModelPreference.fromJson(values);
    }
    throw const FormatException(
      'The saved default model preference has an invalid shape.',
    );
  }

  Future<({DefaultModelPreference? preference, bool isProjectSpecific})>
  readDefaultModelForProject(
    String? projectId, {
    bool useProjectDefault = true,
  }) async {
    if (useProjectDefault && projectId != null) {
      final projectPreference = await readProjectDefaultModel(projectId);
      if (projectPreference != null) {
        return (preference: projectPreference, isProjectSpecific: true);
      }
    }
    return (preference: await readDefaultModel(), isProjectSpecific: false);
  }

  Future<void> writeProjectDefaultModel(
    String projectId,
    DefaultModelPreference? preference,
  ) async {
    final key = '$_projectDefaultModelPrefix${_validatedProjectId(projectId)}';
    if (preference == null) {
      await _preferences.remove(key);
      return;
    }
    await _preferences.setString(key, jsonEncode(preference.toJson()));
  }

  Future<Set<String>> readProjectSkillIds(String projectId) async {
    final key = '$_projectSkillsPrefix${_validatedProjectId(projectId)}';
    final ids = await _preferences.getStringList(key) ?? const <String>[];
    if (ids.length > 32 ||
        ids.toSet().length != ids.length ||
        ids.any((id) => !_isValidProjectSkillId(id))) {
      throw const FormatException('Project Skill preferences are invalid.');
    }
    return Set<String>.unmodifiable(ids);
  }

  Future<void> writeProjectSkillIds(
    String projectId,
    Set<String> skillIds,
  ) async {
    if (skillIds.length > 32 ||
        skillIds.any((id) => !_isValidProjectSkillId(id))) {
      throw ArgumentError.value(
        skillIds,
        'skillIds',
        'Project Skill selection is invalid.',
      );
    }
    final key = '$_projectSkillsPrefix${_validatedProjectId(projectId)}';
    if (skillIds.isEmpty) {
      await _preferences.remove(key);
      return;
    }
    final sorted = skillIds.toList()..sort();
    await _preferences.setStringList(key, sorted);
  }

  Future<Set<String>> readHiddenModelKeys() async {
    final raw = await _preferences.getString(_hiddenModelKeysKey);
    if (raw == null) return const <String>{};
    final Object? decoded;
    try {
      decoded = jsonDecode(raw);
    } on FormatException {
      throw const FormatException('Saved hidden model keys are invalid.');
    }
    if (decoded is! List<Object?> ||
        decoded.length > 10000 ||
        decoded.any(
          (key) =>
              key is! String ||
              key.isEmpty ||
              key.trim() != key ||
              key.length > 1024,
        )) {
      throw const FormatException('Saved hidden model keys are invalid.');
    }
    final keys = decoded.cast<String>().toSet();
    if (keys.length != decoded.length) {
      throw const FormatException('Saved hidden model keys are invalid.');
    }
    return Set<String>.unmodifiable(keys);
  }

  Future<void> writeHiddenModelKeys(Set<String> keys) {
    if (keys.length > 10000 ||
        keys.any(
          (key) =>
              key.isEmpty ||
              key.trim() != key ||
              key.length > 1024 ||
              key.contains('\u0000'),
        )) {
      throw ArgumentError.value(keys, 'keys', 'Hidden model keys are invalid.');
    }
    if (keys.isEmpty) {
      return _preferences.remove(_hiddenModelKeysKey);
    }
    return _preferences.setString(
      _hiddenModelKeysKey,
      jsonEncode(keys.toList(growable: false)),
    );
  }

  Future<void> clearHiddenModelKeys() =>
      _preferences.remove(_hiddenModelKeysKey);

  Future<Map<String, int>> readModelContextLimits() async {
    final raw = await _preferences.getString(_modelContextLimitsKey);
    if (raw == null || raw.trim().isEmpty) return const <String, int>{};
    final Object? decoded;
    try {
      decoded = jsonDecode(raw);
    } on FormatException {
      throw const FormatException('Saved model context limits are invalid.');
    }
    if (decoded is! Map || decoded.length > 10000) {
      throw const FormatException('Saved model context limits are invalid.');
    }
    final limits = <String, int>{};
    for (final entry in decoded.entries) {
      final key = entry.key;
      final value = entry.value;
      if (key is! String || key.isEmpty || value is! int || value < 2048) {
        throw const FormatException('Saved model context limits are invalid.');
      }
      limits[key] = value;
    }
    return Map<String, int>.unmodifiable(limits);
  }

  Future<void> writeModelContextLimit(String routeKey, int? limit) async {
    if (routeKey.trim().isEmpty ||
        routeKey.length > 1024 ||
        routeKey.contains('\u0000') ||
        (limit != null && limit < 2048)) {
      throw ArgumentError.value(
        routeKey,
        'routeKey',
        'The model context limit is invalid.',
      );
    }
    final limits = Map<String, int>.from(await readModelContextLimits());
    if (limit == null) {
      limits.remove(routeKey);
    } else {
      limits[routeKey] = limit;
    }
    if (limits.isEmpty) {
      await _preferences.remove(_modelContextLimitsKey);
    } else {
      await _preferences.setString(_modelContextLimitsKey, jsonEncode(limits));
    }
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
    'llama_cpp' => engineId,
    _ => throw ArgumentError.value(
      engineId,
      'engineId',
      'The local engine is not supported.',
    ),
  };

  String _validatedProviderId(String providerId) {
    if (!RegExp(r'^[a-z][a-z0-9_-]{0,63}$').hasMatch(providerId)) {
      throw ArgumentError.value(
        providerId,
        'providerId',
        'The provider identifier is invalid.',
      );
    }
    return providerId;
  }

  String _providerVerificationRouteKey({
    required String providerId,
    required String connectionId,
    String? workspaceId,
  }) {
    final normalizedProviderId = _validatedProviderId(providerId);
    final normalizedConnectionId = _validatedVerificationRouteId(
      connectionId,
      'connectionId',
    );
    final encodedConnectionId = base64Url.encode(
      utf8.encode(normalizedConnectionId),
    );
    final encodedWorkspaceId = workspaceId == null
        ? null
        : base64Url.encode(
            utf8.encode(
              _validatedVerificationRouteId(workspaceId, 'workspaceId'),
            ),
          );
    return '${_providerVerificationPrefix}route.$normalizedProviderId.'
        '$encodedConnectionId${encodedWorkspaceId == null ? '' : '.$encodedWorkspaceId'}';
  }

  String _providerRequestOutcomeRouteKey({
    required String providerId,
    required String connectionId,
    String? workspaceId,
  }) {
    final normalizedProviderId = _validatedProviderId(providerId);
    final normalizedConnectionId = _validatedVerificationRouteId(
      connectionId,
      'connectionId',
    );
    final encodedConnectionId = base64Url.encode(
      utf8.encode(normalizedConnectionId),
    );
    final encodedWorkspaceId = workspaceId == null
        ? null
        : base64Url.encode(
            utf8.encode(
              _validatedVerificationRouteId(workspaceId, 'workspaceId'),
            ),
          );
    return '$_providerRequestOutcomePrefix$normalizedProviderId.'
        '$encodedConnectionId${encodedWorkspaceId == null ? '' : '.$encodedWorkspaceId'}';
  }

  String _validatedVerificationRouteId(String value, String argumentName) {
    if (value.trim().isEmpty ||
        value.length > 512 ||
        value.contains('\u0000')) {
      throw ArgumentError.value(
        value,
        argumentName,
        'The provider route identifier is invalid.',
      );
    }
    return value;
  }

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

  bool _isValidProjectSkillId(String id) =>
      id.length <= 64 && RegExp(r'^[a-z][a-z0-9-]*$').hasMatch(id);
}
