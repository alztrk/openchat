import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/settings/presentation/usage_quotas_settings_section.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

class _FakeServiceClient extends OpenChatServiceClient {
  _FakeServiceClient(this.handler);

  final Future<Map<String, Object?>> Function(
    String method,
    Map<String, Object?> params,
  )
  handler;

  final List<({String method, Map<String, Object?> params})> calls = [];

  @override
  Future<Map<String, Object?>> call(
    String method, {
    Map<String, Object?> params = const <String, Object?>{},
    Duration timeout = const Duration(seconds: 15),
  }) {
    calls.add((method: method, params: params));
    return handler(method, params);
  }
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    final iconFont = FontLoader('MaterialIcons')
      ..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'));
    await iconFont.load();
  });

  testWidgets(
    'UsageQuotasSettingsSection displays empty state when no accounts connected',
    (tester) async {
      tester.view.physicalSize = const Size(1000, 800);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      var navigatedToConnections = false;
      final fakeService = _FakeServiceClient((method, params) async {
        if (method == 'chatgpt.connections.list') {
          return {'connections': <Object?>[]};
        }
        return {};
      });

      await tester.pumpWidget(
        MaterialApp(
          locale: const Locale('tr'),
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          theme: OpenChatTheme.light,
          home: Scaffold(
            body: SingleChildScrollView(
              child: UsageQuotasSettingsSection(
                serviceClient: fakeService,
                onNavigateToConnections: () => navigatedToConnections = true,
              ),
            ),
          ),
        ),
      );

      await tester.pumpAndSettle();

      expect(find.text('Kullanım Kotaları'), findsOneWidget);
      expect(find.text('Bağlı ChatGPT hesabı bulunmuyor.'), findsOneWidget);
      expect(find.text('Bağlantılara Git'), findsOneWidget);

      await tester.tap(find.text('Bağlantılara Git'));
      await tester.pump();

      expect(navigatedToConnections, isTrue);
    },
  );

  testWidgets(
    'UsageQuotasSettingsSection lists multiple accounts with quotas and reset times',
    (tester) async {
      tester.view.physicalSize = const Size(1000, 900);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final fakeService = _FakeServiceClient((method, params) async {
        if (method == 'chatgpt.connections.list') {
          return {
            'connections': [
              {
                'id': 'conn-personal',
                'email': 'personal@example.com',
                'planType': 'Plus',
                'authStatus': 'authorized',
                'isSelected': true,
                'workspaces': [
                  {
                    'id': 'ws-personal',
                    'externalId': 'ext-ws-1',
                    'displayName': 'Kişisel',
                    'planType': 'Plus',
                    'isSelected': true,
                  },
                ],
              },
              {
                'id': 'conn-work',
                'email': 'work@company.com',
                'planType': 'Team',
                'authStatus': 'authorized',
                'isSelected': false,
                'workspaces': [
                  {
                    'id': 'ws-work',
                    'externalId': 'ext-ws-2',
                    'displayName': 'Şirket',
                    'planType': 'Team',
                    'isSelected': true,
                  },
                ],
              },
            ],
          };
        }

        if (method == 'chatgpt.usage.get') {
          final connectionId = params['connectionId'];
          if (connectionId == 'conn-personal') {
            return {
              'fetchedAtUnixMs': 1769774400000,
              'freshness': 'current',
              'ordinaryUsageAllowed': true,
              'resetCreditCount': 0,
              'resetCreditDetailsState': 'available',
              'resetCredits': <Object?>[],
              'buckets': [
                {
                  'limitId': 'codex:primary',
                  'usedPercent': 25.0,
                  'windowSeconds': 18000,
                  'resetAtUnixMs': 1769792400000,
                },
                {
                  'limitId': 'codex:secondary',
                  'usedPercent': 50.0,
                  'windowSeconds': 604800,
                  'resetAtUnixMs': 1770379200000,
                },
              ],
            };
          }
          if (connectionId == 'conn-work') {
            return {
              'fetchedAtUnixMs': 1769774400000,
              'freshness': 'current',
              'ordinaryUsageAllowed': true,
              'resetCreditCount': 1,
              'resetCreditDetailsState': 'available',
              'resetCredits': [
                {
                  'id': 'credit-1',
                  'title': 'Takım Sıfırlama Kredisi',
                  'status': 'available',
                },
              ],
              'buckets': [
                {
                  'limitId': 'codex:primary',
                  'usedPercent': 80.0,
                  'windowSeconds': 18000,
                  'resetAtUnixMs': 1769792400000,
                },
              ],
            };
          }
        }

        return {};
      });

      await tester.pumpWidget(
        MaterialApp(
          locale: const Locale('tr'),
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          theme: OpenChatTheme.light,
          home: Scaffold(
            body: SingleChildScrollView(
              child: UsageQuotasSettingsSection(serviceClient: fakeService),
            ),
          ),
        ),
      );

      await tester.pumpAndSettle();

      // İki hesap da görüntülenmeli
      expect(find.text('personal@example.com'), findsOneWidget);
      expect(find.text('work@company.com'), findsOneWidget);

      // Aktif hesap rozeti sadece seçili olanda olmalı
      expect(find.text('Aktif'), findsOneWidget);

      // Plan türleri
      expect(find.text('Kullanım · Plus'), findsOneWidget);
      expect(find.text('Kullanım · Team'), findsOneWidget);

      // Kota kullanım oranları
      expect(find.text('%25 kullanıldı'), findsOneWidget);
      expect(find.text('%50 kullanıldı'), findsOneWidget);
      expect(find.text('%80 kullanıldı'), findsOneWidget);

      // Reset credit bilgisi
      expect(find.text('Kullanılabilir sıfırlama hakkı: 1'), findsOneWidget);
      expect(find.text('Takım Sıfırlama Kredisi'), findsOneWidget);
      expect(find.text('Hakkı kullan'), findsOneWidget);

      // Tümünü yenile butonu
      expect(find.text('Tümünü yenile'), findsOneWidget);
      await tester.tap(find.text('Tümünü yenile'));
      await tester.pumpAndSettle();

      // Yeniden istek yapılmış olmalı
      final usageCalls = fakeService.calls
          .where((call) => call.method == 'chatgpt.usage.get')
          .toList();
      expect(usageCalls.length, greaterThanOrEqualTo(4));
    },
  );
}
