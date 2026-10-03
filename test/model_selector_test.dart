import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/chatgpt_connection.dart';
import 'package:openchat/features/chat/presentation/widgets/model_selector.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    final iconFont = FontLoader('MaterialIcons')
      ..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'));
    await iconFont.load();
  });

  testWidgets('ChatGPT provider is disabled until a source is available', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(800, 700);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    var isChatGptConnected = false;
    var availableProviderIds = <String>{};
    var selectedProviderId = 'opencode';
    late StateSetter updateSelector;

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('tr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        home: Scaffold(
          body: Center(
            child: StatefulBuilder(
              builder: (context, setState) {
                updateSelector = setState;
                return ModelSelector(
                  label: 'Model seç',
                  iconRoot: 'assets/icons',
                  palette: OpenChatPalette.of(context),
                  compact: false,
                  models: const <ChatGptModel>[],
                  favoriteModels: const [],
                  selectedModelId: null,
                  selectedModelRouteKey: null,
                  providerId: selectedProviderId,
                  isChatGptConnected: isChatGptConnected,
                  availableProviderIds: availableProviderIds,
                  onProviderSelected: (providerId) =>
                      setState(() => selectedProviderId = providerId),
                  isLoadingModels: false,
                  emptyModelsLabel: 'Model yok',
                  onSelected: (_) {},
                  onFavoriteChanged: (_, _, _, _, _) {},
                  onFavoriteSelected: (_) {},
                );
              },
            ),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Model seç'));
    await tester.pumpAndSettle();

    var chatGptTab = tester.widget<TextButton>(
      find.widgetWithText(TextButton, 'ChatGPT'),
    );
    expect(chatGptTab.onPressed, isNull);
    await tester.tap(find.text('ChatGPT'));
    await tester.pumpAndSettle();
    expect(selectedProviderId, 'opencode');

    updateSelector(() => isChatGptConnected = true);
    await tester.pumpAndSettle();
    chatGptTab = tester.widget<TextButton>(
      find.widgetWithText(TextButton, 'ChatGPT'),
    );
    expect(chatGptTab.onPressed, isNotNull);

    await tester.tap(find.text('ChatGPT'));
    await tester.pumpAndSettle();
    expect(selectedProviderId, 'chatgpt');

    var mistralTab = tester.widget<TextButton>(
      find.widgetWithText(TextButton, 'Mistral'),
    );
    expect(mistralTab.onPressed, isNull);

    updateSelector(() => availableProviderIds = {'mistral'});
    await tester.pumpAndSettle();
    mistralTab = tester.widget<TextButton>(
      find.widgetWithText(TextButton, 'Mistral'),
    );
    expect(mistralTab.onPressed, isNotNull);
    await tester.tap(find.text('Mistral'));
    await tester.pumpAndSettle();
    expect(selectedProviderId, 'mistral');
  });

  testWidgets('model catalog shows source and OpenCode category sections', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(800, 700);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    var providerId = 'chatgpt';
    late StateSetter updateSelector;
    final chatGptModels = [
      const ChatGptModel(
        id: 'gpt-api-model',
        displayName: 'API model',
        isAvailable: true,
        reasoningLevels: [],
        providerId: 'chatgpt_api',
        connectionId: 'api-key-1',
        groupId: 'api',
      ),
      const ChatGptModel(
        id: 'gpt-oauth-model',
        displayName: 'OAuth model',
        isAvailable: true,
        reasoningLevels: [],
        providerId: 'chatgpt',
        connectionId: 'account-1',
        workspaceId: 'workspace-1',
        groupId: 'oauth',
      ),
    ];
    final openCodeModels = [
      const ChatGptModel(
        id: 'free-model',
        displayName: 'Free model',
        description: 'free',
        isAvailable: true,
        reasoningLevels: [],
        providerId: 'opencode',
        groupId: 'free',
      ),
      const ChatGptModel(
        id: 'api-model',
        displayName: 'API model',
        description: 'paid',
        isAvailable: true,
        reasoningLevels: [],
        providerId: 'opencode',
        groupId: 'paid',
      ),
    ];

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('tr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        home: Scaffold(
          body: Center(
            child: StatefulBuilder(
              builder: (context, setState) {
                updateSelector = setState;
                return ModelSelector(
                  label: 'Model seç',
                  iconRoot: 'assets/icons',
                  palette: OpenChatPalette.of(context),
                  compact: false,
                  models: providerId == 'opencode'
                      ? openCodeModels
                      : chatGptModels,
                  favoriteModels: const [],
                  selectedModelId: null,
                  selectedModelRouteKey: null,
                  providerId: providerId,
                  isChatGptConnected: true,
                  onProviderSelected: (selected) =>
                      setState(() => providerId = selected),
                  isLoadingModels: false,
                  emptyModelsLabel: 'Model yok',
                  onSelected: (_) {},
                  onFavoriteChanged: (_, _, _, _, _) {},
                  onFavoriteSelected: (_) {},
                );
              },
            ),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Model seç'));
    await tester.pumpAndSettle();
    expect(find.text('API'), findsOneWidget);
    expect(find.text('OAuth'), findsOneWidget);
    expect(find.text('API model'), findsOneWidget);
    expect(find.text('OAuth model'), findsOneWidget);

    await tester.tap(find.text('OpenCode'));
    updateSelector(() => providerId = 'opencode');
    await tester.pumpAndSettle();
    expect(find.text('Ücretsiz modeller'), findsOneWidget);
    expect(find.text('API modelleri'), findsOneWidget);
    expect(find.text('Free model'), findsOneWidget);
  });

  testWidgets('hidden models are excluded from model selector list', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(800, 700);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final openCodeModels = [
      const ChatGptModel(
        id: 'free-model-1',
        displayName: 'Free Model 1',
        isAvailable: true,
        reasoningLevels: [],
        providerId: 'opencode',
        groupId: 'free',
      ),
      const ChatGptModel(
        id: 'free-model-2',
        displayName: 'Free Model 2',
        isAvailable: true,
        reasoningLevels: [],
        providerId: 'opencode',
        groupId: 'free',
      ),
    ];

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('tr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        home: Scaffold(
          body: Center(
            child: ModelSelector(
              label: 'Model seç',
              iconRoot: 'assets/icons',
              palette: OpenChatPalette.light,
              compact: false,
              models: openCodeModels,
              favoriteModels: const [],
              hiddenModelKeys: const {'opencode:::free-model-2'},
              selectedModelId: null,
              selectedModelRouteKey: null,
              providerId: 'opencode',
              isChatGptConnected: false,
              onProviderSelected: (_) {},
              isLoadingModels: false,
              emptyModelsLabel: 'Model yok',
              onSelected: (_) {},
              onFavoriteChanged: (_, _, _, _, _) {},
              onFavoriteSelected: (_) {},
            ),
          ),
        ),
      ),
    );

    await tester.tap(find.text('Model seç'));
    await tester.pumpAndSettle();

    expect(find.text('Free Model 1'), findsOneWidget);
    expect(find.text('Free Model 2'), findsNothing);
  });
}
