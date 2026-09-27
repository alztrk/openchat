import 'package:flutter/material.dart';
import 'package:shared_preferences/shared_preferences.dart';

class SettingsPreferences {
  SettingsPreferences(this._preferences);

  static const _themeModeKey = 'appearance.theme_mode';

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
}
