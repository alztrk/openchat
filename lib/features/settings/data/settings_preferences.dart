import 'package:flutter/material.dart';
import 'package:shared_preferences/shared_preferences.dart';

enum ToolPermissionMode {
  requireApproval,
  fullAccess;

  String get serviceValue => switch (this) {
    ToolPermissionMode.requireApproval => 'require_approval',
    ToolPermissionMode.fullAccess => 'full_access',
  };
}

class SettingsPreferences {
  SettingsPreferences(this._preferences);

  static const _themeModeKey = 'appearance.theme_mode';
  static const _localeKey = 'appearance.locale';
  static const _sharedInstructionsKey = 'chat.shared_instructions';
  static const _toolPermissionModeKey = 'tools.permission_mode';
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

  Future<Locale?> readLocale() async {
    final value = await _preferences.getString(_localeKey);
    return switch (value) {
      'en' => const Locale('en'),
      'tr' => const Locale('tr'),
      _ => null,
    };
  }

  Future<void> writeLocale(Locale? locale) {
    final value = switch (locale?.languageCode) {
      null => 'system',
      'en' => 'en',
      'tr' => 'tr',
      final languageCode => throw ArgumentError.value(
        languageCode,
        'locale',
        'Only the supported English and Turkish locales can be saved.',
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
}
