import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

enum WindowsNotificationStatus {
  enabled,
  disabledForApplication,
  disabledForUser,
  disabledByGroupPolicy,
  disabledByManifest,
  unavailable,
  notSupported;

  static WindowsNotificationStatus fromPlatformValue(String? value) =>
      switch (value) {
        'enabled' => WindowsNotificationStatus.enabled,
        'disabled_for_application' =>
          WindowsNotificationStatus.disabledForApplication,
        'disabled_for_user' => WindowsNotificationStatus.disabledForUser,
        'disabled_by_group_policy' =>
          WindowsNotificationStatus.disabledByGroupPolicy,
        'disabled_by_manifest' => WindowsNotificationStatus.disabledByManifest,
        _ => WindowsNotificationStatus.unavailable,
      };
}

class WindowsNotificationStatusClient {
  const WindowsNotificationStatusClient({this.targetPlatform});

  static const MethodChannel _channel = MethodChannel('openchat/window');
  final TargetPlatform? targetPlatform;

  TargetPlatform get _currentTargetPlatform =>
      targetPlatform ?? defaultTargetPlatform;

  Future<WindowsNotificationStatus> readStatus() async {
    if (_currentTargetPlatform != TargetPlatform.windows) {
      return WindowsNotificationStatus.notSupported;
    }
    final value = await _channel.invokeMethod<String>('notificationStatus');
    return WindowsNotificationStatus.fromPlatformValue(value);
  }

  Future<void> openSettings() async {
    if (_currentTargetPlatform != TargetPlatform.windows) {
      throw UnsupportedError(
        'Windows notification settings are only available on Windows.',
      );
    }
    await _channel.invokeMethod<void>('openNotificationSettings');
  }
}
