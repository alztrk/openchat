import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:openchat/features/chat/domain/default_model_preference.dart';
import 'package:shared_preferences/shared_preferences.dart';

enum ToolPermissionMode {
  requireApproval,
  fullAccess;

  String get serviceValue => switch (this) {
    ToolPermissionMode.requireApproval => 'require_approval',
    ToolPermissionMode.fullAccess => 'full_access',
  };
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
  static const _toolPermissionModeKey = 'tools.permission_mode';
  static const _conversationWidthKey = 'appearance.conversation_width';
  static const _conversationTextSizeKey = 'appearance.conversation_text_size';
  static const _appFontKey = 'appearance.conversation_font';
  static const _defaultModelKey = 'models.default_model';
  static const _hiddenModelKeysKey = 'models.hidden_keys';
  static const maxSharedInstructionsCharacters = 4096;

  final SharedPreferencesAsync _preferences;

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

  Future<ToolPermissionMode> readToolPermissionMode() async {
    final value = await _preferences.getString(_toolPermissionModeKey);
    return switch (value) {
      null || 'require_approval' => ToolPermissionMode.requireApproval,
      'full_access' => ToolPermissionMode.fullAccess,
      _ => throw const FormatException(
        'The saved tool permission mode is invalid.',
      ),
    };
  }

  Future<void> writeToolPermissionMode(ToolPermissionMode mode) {
    return _preferences.setString(_toolPermissionModeKey, mode.serviceValue);
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
}
