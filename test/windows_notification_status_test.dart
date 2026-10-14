import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/features/settings/presentation/notification_settings_section.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/platform/windows/windows_notification_status.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:shared_preferences_platform_interface/in_memory_shared_preferences_async.dart';
import 'package:shared_preferences_platform_interface/shared_preferences_async_platform_interface.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  const channel = MethodChannel('openchat/window');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;

  setUp(() {
    SharedPreferencesAsyncPlatform.instance =
        InMemorySharedPreferencesAsync.empty();
  });

  tearDown(() {
    messenger.setMockMethodCallHandler(channel, null);
    SharedPreferencesAsyncPlatform.instance = null;
  });

  test('maps every Windows toast notification setting', () {
    expect(
      WindowsNotificationStatus.fromPlatformValue('enabled'),
      WindowsNotificationStatus.enabled,
    );
    expect(
      WindowsNotificationStatus.fromPlatformValue('disabled_for_application'),
      WindowsNotificationStatus.disabledForApplication,
    );
    expect(
      WindowsNotificationStatus.fromPlatformValue('disabled_for_user'),
      WindowsNotificationStatus.disabledForUser,
    );
    expect(
      WindowsNotificationStatus.fromPlatformValue('disabled_by_group_policy'),
      WindowsNotificationStatus.disabledByGroupPolicy,
    );
    expect(
      WindowsNotificationStatus.fromPlatformValue('disabled_by_manifest'),
      WindowsNotificationStatus.disabledByManifest,
    );
    expect(
      WindowsNotificationStatus.fromPlatformValue('unexpected'),
      WindowsNotificationStatus.unavailable,
    );
  });

  test('queries Windows and opens its notification settings page', () async {
    final methods = <String>[];
    messenger.setMockMethodCallHandler(channel, (call) async {
      methods.add(call.method);
      return call.method == 'notificationStatus' ? 'disabled_for_user' : null;
    });
    const client = WindowsNotificationStatusClient(
      targetPlatform: TargetPlatform.windows,
    );

    expect(
      await client.readStatus(),
      WindowsNotificationStatus.disabledForUser,
    );
    await client.openSettings();
    expect(methods, <String>['notificationStatus', 'openNotificationSettings']);
  });

  test('does not call the Windows channel on other platforms', () async {
    var called = false;
    messenger.setMockMethodCallHandler(channel, (call) async {
      called = true;
      return null;
    });

    expect(
      await const WindowsNotificationStatusClient(
        targetPlatform: TargetPlatform.linux,
      ).readStatus(),
      WindowsNotificationStatus.notSupported,
    );
    expect(called, isFalse);
  });

  testWidgets(
    'settings explain a Windows notification block and open recovery',
    (tester) async {
      final methods = <String>[];
      messenger.setMockMethodCallHandler(channel, (call) async {
        methods.add(call.method);
        return call.method == 'notificationStatus' ? 'disabled_for_user' : null;
      });
      final preferences = SettingsPreferences(SharedPreferencesAsync());

      await tester.pumpWidget(
        MaterialApp(
          locale: const Locale('en'),
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          theme: OpenChatTheme.light,
          home: Scaffold(
            body: SingleChildScrollView(
              child: NotificationSettingsSection(
                settingsPreferences: preferences,
                notificationStatusClient: const WindowsNotificationStatusClient(
                  targetPlatform: TargetPlatform.windows,
                ),
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(
        find.text('Notifications are turned off in Windows settings.'),
        findsOneWidget,
      );
      await tester.tap(find.text('Open Windows notification settings'));
      expect(methods, <String>[
        'notificationStatus',
        'openNotificationSettings',
      ]);
    },
  );
}
