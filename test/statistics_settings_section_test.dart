import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/settings/presentation/statistics_settings_section.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

class _FakeStatisticsServiceClient extends OpenChatServiceClient {
  final List<({String method, Map<String, Object?> params})> calls = [];

  @override
  Future<Map<String, Object?>> call(
    String method, {
    Map<String, Object?> params = const <String, Object?>{},
    Duration timeout = const Duration(seconds: 15),
  }) async {
    calls.add((method: method, params: params));
    return _statisticsResponse();
  }
}

Map<String, Object?> _statisticsResponse() => {
  'summary': {
    'inputTokens': 120,
    'outputTokens': 80,
    'totalTokens': 200,
    'reasoningTokens': 0,
    'cachedInputTokens': 0,
    'cacheWriteTokens': 0,
    'providerReportedCostUsd': 0.0,
    'costReportedRequests': 0,
    'modelsDevCatalogCostUsd': 0.0,
    'modelsDevCatalogCostRequests': 0,
    'totalRequests': 2,
    'successfulRequests': 2,
    'failedRequests': 0,
    'cancelledRequests': 0,
    'interruptedRequests': 0,
    'pendingRequests': 0,
    'conversationCount': 1,
    'fastRequestedRequests': 0,
    'inputUsageRequests': 2,
    'outputUsageRequests': 2,
    'reasoningUsageRequests': 0,
    'anyUsageRequests': 2,
    'legacyOutputMessages': 0,
  },
  'modelsDevPricing': {'source': 'models.dev', 'state': 'current'},
  'providerBreakdown': <Object?>[],
  'modelBreakdown': <Object?>[],
  'reasoningBreakdown': <Object?>[],
  'operationBreakdown': <Object?>[],
  'fastModeBreakdown': <Object?>[],
  'serviceTierBreakdown': <Object?>[],
  'dailyTrend': [
    {
      'day': '2026-10-01',
      'inputTokens': 40,
      'outputTokens': 20,
      'modelsDevCatalogCostUsd': 0.0,
      'modelsDevCatalogCostRequests': 0,
      'requests': 1,
    },
    {
      'day': '2026-10-08',
      'inputTokens': 80,
      'outputTokens': 60,
      'modelsDevCatalogCostUsd': 0.0,
      'modelsDevCatalogCostRequests': 0,
      'requests': 1,
    },
  ],
  'monthlyTrend': <Object?>[],
  'conversationRanking': <Object?>[],
  'requestDetailCount': 0,
  'requestDetails': <Object?>[],
  'filterOptions': {
    'providers': ['openai'],
    'models': ['gpt-example'],
    'reasoningEfforts': <String>[],
  },
  'quotaHistory': <Object?>[],
};

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('statistics filters live in the header and update the query', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(390, 820);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final service = _FakeStatisticsServiceClient();

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('tr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark,
        home: Scaffold(
          body: SingleChildScrollView(
            child: StatisticsSettingsSection(serviceClient: service),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('İstatistikler'), findsOneWidget);
    expect(find.text('Filtreler'), findsOneWidget);
    expect(find.byType(CustomPaint), findsWidgets);
    expect(find.text('İstek geçmişi'), findsNothing);
    expect(find.text('ChatGPT kota geçmişi'), findsNothing);
    expect(tester.takeException(), isNull);

    await tester.tap(find.text('Filtreler'));
    await tester.pumpAndSettle();
    final providerMenuItem = find.text('Sağlayıcı');
    await tester.sendEventToBinding(
      PointerHoverEvent(position: tester.getCenter(providerMenuItem)),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('OpenAI'));
    await tester.pumpAndSettle();

    expect(service.calls.last.params['providerId'], 'openai');
    expect(tester.takeException(), isNull);
  });
}
