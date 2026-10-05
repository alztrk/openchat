import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/openchat_window_title_bar.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/domain/history_storage_status.dart';
import 'package:openchat/features/chat/presentation/chat_screen.dart';
import 'package:openchat/features/chat/presentation/widgets/chat_composer.dart';
import 'package:openchat/features/chat/presentation/widgets/chat_navigation_rail.dart';
import 'package:openchat/features/chat/presentation/widgets/conversation_pane.dart';
import 'package:openchat/features/chat/presentation/widgets/conversation_sidebar.dart';
import 'package:openchat/features/chat/presentation/widgets/window_control_bar.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/features/settings/presentation/settings_screen.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:shared_preferences_platform_interface/in_memory_shared_preferences_async.dart';
import 'package:shared_preferences_platform_interface/shared_preferences_async_platform_interface.dart';

import 'fixtures/figma_chat_messages.dart';
import 'fixtures/figma_sidebar_items.dart';

const _mainSurfaceBottomInset = 10.0;

late GoldenFileComparator _previousGoldenComparator;

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    SharedPreferencesAsyncPlatform.instance =
        InMemorySharedPreferencesAsync.empty();
    _previousGoldenComparator = goldenFileComparator;
    goldenFileComparator = _FigmaRenderingComparator(
      Uri.parse('test/fidelity_screenshot_test.dart'),
    );
    final iconFont = FontLoader('MaterialIcons')
      ..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'));
    await iconFont.load();
    final fontLoader = FontLoader('Manrope')
      ..addFont(rootBundle.load('assets/fonts/Manrope[wght].ttf'));
    await fontLoader.load();
  });
  tearDownAll(() {
    goldenFileComparator = _previousGoldenComparator;
    SharedPreferencesAsyncPlatform.instance = null;
  });

  testWidgets('keeps the app title bar within a narrow restored window', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(160, 40);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('tr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark.copyWith(platform: TargetPlatform.windows),
        home: const Scaffold(body: OpenChatWindowTitleBar()),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('OpenChat'), findsNothing);
    final titleBar = tester.getRect(find.byType(OpenChatWindowTitleBar));
    final controls = tester.getRect(find.byType(WindowControlBar));
    expect(titleBar.height, OpenChatSpacing.appTitleBarHeight);
    expect(controls.top, 4);
    expect(controls.bottom, titleBar.bottom - 4);
    expect(tester.takeException(), isNull);
  });

  testWidgets('project creation appears while hovering its section heading', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(360, 720);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final searchController = TextEditingController();
    addTearDown(searchController.dispose);
    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('tr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark,
        home: Scaffold(
          body: ConversationSidebar(
            searchController: searchController,
            width: OpenChatSpacing.sidebarWidth,
            projects: figmaSidebarProjects,
            onCreateProject: () {},
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    final createButton = find.byKey(
      const ValueKey<String>('project-create-button'),
    );
    final opacity = find.ancestor(
      of: createButton,
      matching: find.byType(AnimatedOpacity),
    );
    expect(tester.widget<AnimatedOpacity>(opacity).opacity, 0);
    final l10n = AppLocalizations.of(tester.element(createButton));
    expect(l10n, isNotNull);
    expect(
      tester.getSize(find.byTooltip(l10n!.searchMessagesTooltip)).width,
      32,
    );
    expect(tester.getSize(find.byTooltip(l10n.newConversation)).width, 32);

    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    addTearDown(mouse.removePointer);
    await mouse.addPointer(location: Offset.zero);
    await mouse.moveTo(tester.getCenter(find.text('Projeler')));
    await tester.pump(const Duration(milliseconds: 160));
    expect(tester.widget<AnimatedOpacity>(opacity).opacity, 1);

    await mouse.moveTo(const Offset(350, 200));
    await tester.pump(const Duration(milliseconds: 160));
    expect(tester.widget<AnimatedOpacity>(opacity).opacity, 0);
    expect(tester.takeException(), isNull);
  });

  testWidgets('chat sidebar animates closed', (tester) async {
    tester.view.physicalSize = const Size(1280, 720);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('tr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light.copyWith(platform: TargetPlatform.windows),
        home: const ChatScreen(
          themeMode: ThemeMode.light,
          onThemeModeChanged: _ignoreThemeMode,
          onToggleTheme: _ignoreThemeToggle,
          historyStorageStatus: HistoryStorageStatus.available,
        ),
      ),
    );
    await tester.pumpAndSettle();

    final transition = find.byKey(
      const ValueKey<String>('conversation-sidebar-transition'),
    );
    final l10n = AppLocalizations.of(tester.element(transition));
    expect(l10n, isNotNull);
    expect(
      tester.getSize(transition).width,
      OpenChatSpacing.compactSidebarWidth,
    );
    await tester.tap(find.byTooltip(l10n!.collapseSidebars));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 110));
    final intermediateWidth = tester.getSize(transition).width;
    expect(intermediateWidth, greaterThan(0));
    expect(intermediateWidth, lessThan(OpenChatSpacing.compactSidebarWidth));
    await tester.pumpAndSettle();
    expect(tester.getSize(transition).width, 0);
    expect(find.byType(ConversationSidebar), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testWidgets('scrolls navigation controls in a short window', (tester) async {
    tester.view.physicalSize = const Size(56, 120);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('tr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark.copyWith(platform: TargetPlatform.windows),
        home: Scaffold(
          body: SizedBox.expand(
            child: ChatNavigationRail(
              expanded: false,
              showBrand: false,
              settingsSelected: false,
              onOpenChat: () {},
              onOpenSettings: () {},
              onToggleTheme: () {},
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.byType(SingleChildScrollView), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('keeps the permission menu compact and without tooltips', (
    tester,
  ) async {
    const locale = Locale('tr');
    final l10n = await AppLocalizations.delegate.load(locale);
    final controller = TextEditingController();
    addTearDown(controller.dispose);
    tester.view.physicalSize = const Size(1200, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(
      MaterialApp(
        locale: locale,
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light,
        home: Scaffold(
          body: Center(
            child: SizedBox(
              width: 1000,
              child: ChatComposer(
                controller: controller,
                onSendMessage: () {},
                canSendMessage: false,
                showReasoningSelector: false,
                providerId: 'chatgpt',
                onToolPermissionModeChanged: (_) {},
              ),
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    final permissionButton = find.widgetWithText(
      OutlinedButton,
      l10n.toolPermissionRequireApproval,
    );
    expect(permissionButton, findsOneWidget);
    expect(
      find.ancestor(of: permissionButton, matching: find.byType(Tooltip)),
      findsNothing,
    );
    expect(
      find.text(l10n.toolPermissionRequireApprovalDescription),
      findsNothing,
    );

    await tester.tap(permissionButton);
    await tester.pumpAndSettle();

    expect(
      find.text(l10n.toolPermissionRequireApprovalDescription),
      findsNothing,
    );
    expect(find.text(l10n.toolPermissionFullAccessDescription), findsNothing);
  });

  for (final appearance in [
    (name: 'light', theme: OpenChatTheme.light),
    (name: 'dark', theme: OpenChatTheme.dark),
  ]) {
    testWidgets('matches the empty ${appearance.name} Figma pane', (
      tester,
    ) async {
      await _expectPaneMatchesFigma(
        tester,
        theme: appearance.theme,
        selectedModelLabel: 'Örnek 1',
        goldenPath: 'goldens/figma-empty-pane-${appearance.name}.png',
      );
    });

    testWidgets('matches the active ${appearance.name} Figma pane', (
      tester,
    ) async {
      await _expectPaneMatchesFigma(
        tester,
        theme: appearance.theme,
        messages: figmaChatMessages,
        conversationTitle: 'OpenChat sohbeti',
        selectedModelLabel: 'Örnek 1',
        reasoningLevel: 'Orta',
        goldenPath: 'goldens/figma-active-pane-${appearance.name}.png',
      );
    });

    testWidgets('matches the active ${appearance.name} Figma screen', (
      tester,
    ) async {
      await _expectActiveScreenMatchesFigma(
        tester,
        theme: appearance.theme,
        goldenPath: 'goldens/figma-active-screen-${appearance.name}.png',
      );
    });

    testWidgets('matches the narrow active ${appearance.name} Figma screen', (
      tester,
    ) async {
      await _expectActiveScreenMatchesFigma(
        tester,
        theme: appearance.theme,
        size: const Size(1280, 720),
        expandedRail: false,
        sidebarWidth: OpenChatSpacing.compactSidebarWidth,
        goldenPath: 'goldens/figma-active-narrow-${appearance.name}.png',
      );
    });

    testWidgets('matches the compact active ${appearance.name} Figma screen', (
      tester,
    ) async {
      await _expectActiveScreenMatchesFigma(
        tester,
        theme: appearance.theme,
        size: const Size(1492, 900),
        expandedRail: false,
        sidebarWidth: OpenChatSpacing.sidebarWidth,
        goldenPath: 'goldens/figma-active-compact-${appearance.name}.png',
      );
    });

    testWidgets('matches the empty ${appearance.name} Figma screen', (
      tester,
    ) async {
      await _expectEmptyScreenMatchesFigma(
        tester,
        theme: appearance.theme,
        goldenPath: 'goldens/figma-empty-screen-${appearance.name}.png',
      );
    });

    testWidgets('matches the compact empty ${appearance.name} Figma screen', (
      tester,
    ) async {
      await _expectEmptyScreenMatchesFigma(
        tester,
        theme: appearance.theme,
        size: const Size(1492, 900),
        goldenPath: 'goldens/figma-empty-compact-${appearance.name}.png',
      );
    });

    testWidgets('matches the ${appearance.name} Figma settings screen', (
      tester,
    ) async {
      await _expectSettingsScreenMatchesFigma(
        tester,
        theme: appearance.theme,
        goldenPath: 'goldens/figma-settings-screen-${appearance.name}.png',
      );
    });

    testWidgets('matches the narrow ${appearance.name} composer geometry', (
      tester,
    ) async {
      await _expectNarrowComposerGeometry(tester, theme: appearance.theme);
    });

    testWidgets('matches the compact ${appearance.name} navigation rail', (
      tester,
    ) async {
      await _expectCompactNavigationRailMatchesFigma(
        tester,
        theme: appearance.theme,
        goldenPath: 'goldens/figma-compact-rail-${appearance.name}.png',
      );
    });
  }

  for (final layout in [
    (name: 'wide', width: 1680.0, height: 900.0, rail: 56.0, sidebar: 320.0),
    (name: 'compact', width: 1492.0, height: 900.0, rail: 56.0, sidebar: 320.0),
    (name: 'narrow', width: 1280.0, height: 720.0, rail: 56.0, sidebar: 300.0),
  ]) {
    testWidgets('matches the ${layout.name} Figma composition geometry', (
      tester,
    ) async {
      await _expectCompositionGeometry(
        tester,
        width: layout.width,
        height: layout.height,
        railWidth: layout.rail,
        sidebarWidth: layout.sidebar,
      );
    });
  }

  testWidgets('sidebar message search opens within the sidebar bounds', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1280, 720);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('tr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light.copyWith(platform: TargetPlatform.windows),
        home: const ChatScreen(
          themeMode: ThemeMode.light,
          onThemeModeChanged: _ignoreThemeMode,
          onToggleTheme: _ignoreThemeToggle,
          historyStorageStatus: HistoryStorageStatus.available,
        ),
      ),
    );
    await tester.pumpAndSettle();

    final sidebarFinder = find.byType(ConversationSidebar);
    final sidebar = tester.getRect(sidebarFinder);
    final l10n = AppLocalizations.of(tester.element(sidebarFinder));
    expect(l10n, isNotNull);
    await tester.tap(find.byTooltip(l10n!.searchMessagesTooltip));
    await tester.pumpAndSettle();

    final search = find.byKey(const ValueKey<String>('history-search-input'));
    expect(search, findsOneWidget);
    final searchBounds = tester.getRect(search);
    expect(searchBounds.left, greaterThanOrEqualTo(sidebar.left));
    expect(searchBounds.right, lessThanOrEqualTo(sidebar.right));
    expect(searchBounds.top, greaterThanOrEqualTo(sidebar.top));
    expect(searchBounds.bottom, lessThanOrEqualTo(sidebar.bottom));
  });

  testWidgets('settings rows grow to fit wrapped descriptions', (tester) async {
    tester.view.physicalSize = const Size(700, 720);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('tr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light.copyWith(platform: TargetPlatform.windows),
        home: Scaffold(
          body: SettingsScreen(
            themeMode: ThemeMode.light,
            onThemeModeChanged: (_) async {},
            historyStorageStatus: HistoryStorageStatus.available,
            hasConversationHistory: false,
            settingsPreferences: SettingsPreferences(SharedPreferencesAsync()),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(tester.takeException(), isNull);
  });
}

Future<void> _expectCompactNavigationRailMatchesFigma(
  WidgetTester tester, {
  required ThemeData theme,
  required String goldenPath,
}) async {
  tester.view.physicalSize = const Size(56, 900);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);

  await tester.pumpWidget(
    MaterialApp(
      locale: const Locale('tr'),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      theme: theme.copyWith(platform: TargetPlatform.windows),
      home: RepaintBoundary(
        key: const ValueKey<String>('compact-navigation-rail-screenshot'),
        child: SizedBox.expand(
          child: ChatNavigationRail(
            expanded: false,
            settingsSelected: false,
            onOpenChat: _ignoreThemeToggle,
            onOpenSettings: _ignoreThemeToggle,
            onToggleTheme: _ignoreThemeToggle,
          ),
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();

  expect(
    tester.getRect(find.byTooltip('Anasayfa')),
    const Rect.fromLTWH(5.5, 82, 44, 44),
  );
  expect(
    tester.getRect(find.byType(ChatNavigationRail)),
    const Rect.fromLTWH(0, 0, 56, 900),
  );
  expect(
    tester.getRect(find.byKey(const ValueKey<String>('compact-brand'))),
    const Rect.fromLTWH(5.5, 8, 44, 44),
  );
  expect(find.text('OpenChat'), findsNothing);

  await expectLater(
    find.byKey(const ValueKey<String>('compact-navigation-rail-screenshot')),
    matchesGoldenFile(goldenPath),
  );
}

Future<void> _expectCompositionGeometry(
  WidgetTester tester, {
  required double width,
  required double height,
  required double railWidth,
  required double sidebarWidth,
}) async {
  tester.view.physicalSize = Size(width, height);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);

  await tester.pumpWidget(
    MaterialApp(
      locale: const Locale('tr'),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      theme: OpenChatTheme.light.copyWith(platform: TargetPlatform.windows),
      home: const ChatScreen(
        themeMode: ThemeMode.light,
        onThemeModeChanged: _ignoreThemeMode,
        onToggleTheme: _ignoreThemeToggle,
        historyStorageStatus: HistoryStorageStatus.available,
      ),
    ),
  );
  await tester.pumpAndSettle();

  final rail = tester.getRect(find.byType(ChatNavigationRail));
  final appTitleBar = tester.getRect(find.byType(OpenChatWindowTitleBar));
  final sidebar = tester.getRect(find.byType(ConversationSidebar));
  final pane = tester.getRect(find.byType(ConversationPane));
  final sidebarHeader = tester.getRect(
    find.byKey(const ValueKey<String>('conversation-sidebar-header')),
  );
  final conversationHeader = tester.getRect(
    find.byKey(const ValueKey<String>('conversation-header')),
  );
  expect(
    rail,
    Rect.fromLTWH(
      0,
      OpenChatSpacing.appTitleBarHeight,
      railWidth,
      height - OpenChatSpacing.appTitleBarHeight,
    ),
  );
  expect(appTitleBar.height, OpenChatSpacing.appTitleBarHeight);
  expect(sidebarHeader.top, appTitleBar.bottom);
  expect(conversationHeader.top, appTitleBar.bottom);
  expect(conversationHeader.height, OpenChatSpacing.conversationHeaderHeight);
  expect(sidebarHeader.height, conversationHeader.height);
  expect(find.byType(WindowControlBar), findsOneWidget);
  expect(
    find.descendant(
      of: find.byType(ConversationPane),
      matching: find.byType(WindowControlBar),
    ),
    findsNothing,
  );
  expect(
    sidebar,
    Rect.fromLTWH(
      railWidth,
      OpenChatSpacing.appTitleBarHeight,
      sidebarWidth,
      height - OpenChatSpacing.appTitleBarHeight - _mainSurfaceBottomInset,
    ),
  );
  expect(
    pane,
    Rect.fromLTWH(
      railWidth + sidebarWidth,
      OpenChatSpacing.appTitleBarHeight,
      width - railWidth - sidebarWidth - _mainSurfaceBottomInset,
      height - OpenChatSpacing.appTitleBarHeight - _mainSurfaceBottomInset,
    ),
  );
  expect(
    find.descendant(
      of: find.byType(ConversationSidebar),
      matching: find.byType(Divider),
    ),
    findsNothing,
  );
  final composer = tester.getRect(find.byType(ChatComposer));
  expect(composer.width, OpenChatSpacing.composerMaxWidth);
  expect(composer.center.dx, pane.center.dx);
  _expectComposerControls(tester, outlinedButtonCount: 3);
}

Future<void> _expectEmptyScreenMatchesFigma(
  WidgetTester tester, {
  required ThemeData theme,
  required String goldenPath,
  Size size = const Size(1680, 900),
}) async {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);

  await tester.pumpWidget(
    MaterialApp(
      locale: const Locale('tr'),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      theme: theme.copyWith(platform: TargetPlatform.windows),
      home: RepaintBoundary(
        key: const ValueKey<String>('empty-screen-screenshot'),
        child: ChatScreen(
          themeMode: theme.brightness == Brightness.dark
              ? ThemeMode.dark
              : ThemeMode.light,
          onThemeModeChanged: _ignoreThemeMode,
          onToggleTheme: _ignoreThemeToggle,
          historyStorageStatus: HistoryStorageStatus.available,
          selectedModelLabel: 'Örnek 1',
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();

  await expectLater(
    find.byKey(const ValueKey<String>('empty-screen-screenshot')),
    matchesGoldenFile(goldenPath),
  );
}

Future<void> _expectSettingsScreenMatchesFigma(
  WidgetTester tester, {
  required ThemeData theme,
  required String goldenPath,
  Size size = const Size(1680, 900),
}) async {
  await _pumpSettingsScreen(tester, theme: theme, size: size);

  await expectLater(
    find.byKey(const ValueKey<String>('settings-screen-screenshot')),
    matchesGoldenFile(goldenPath),
  );
}

Future<void> _pumpSettingsScreen(
  WidgetTester tester, {
  required ThemeData theme,
  Size size = const Size(1680, 900),
}) async {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);

  await tester.pumpWidget(
    MaterialApp(
      locale: const Locale('tr'),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      theme: theme.copyWith(platform: TargetPlatform.windows),
      home: RepaintBoundary(
        key: const ValueKey<String>('settings-screen-screenshot'),
        child: ChatScreen(
          themeMode: theme.brightness == Brightness.dark
              ? ThemeMode.dark
              : ThemeMode.light,
          onThemeModeChanged: _ignoreThemeMode,
          onToggleTheme: _ignoreThemeToggle,
          historyStorageStatus: HistoryStorageStatus.available,
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();
  await tester.tap(find.byTooltip('Ayarlar'));
  await tester.pumpAndSettle();
}

Future<void> _expectActiveScreenMatchesFigma(
  WidgetTester tester, {
  required ThemeData theme,
  required String goldenPath,
  Size size = const Size(1680, 900),
  bool expandedRail = false,
  double sidebarWidth = OpenChatSpacing.sidebarWidth,
}) async {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);

  final searchController = TextEditingController();
  final messageController = TextEditingController();
  addTearDown(searchController.dispose);
  addTearDown(messageController.dispose);

  await tester.pumpWidget(
    MaterialApp(
      locale: const Locale('tr'),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      theme: theme.copyWith(platform: TargetPlatform.windows),
      home: RepaintBoundary(
        key: const ValueKey<String>('active-screen-screenshot'),
        child: Scaffold(
          body: Column(
            children: [
              const OpenChatWindowTitleBar(),
              Expanded(
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    ChatNavigationRail(
                      expanded: expandedRail,
                      showBrand: false,
                      settingsSelected: false,
                      onOpenChat: () {},
                      onOpenSettings: () {},
                      onToggleTheme: () {},
                    ),
                    ConversationSidebar(
                      searchController: searchController,
                      width: sidebarWidth,
                      projects: figmaSidebarProjects,
                      pinnedConversations: figmaPinnedConversations,
                      conversations: figmaConversations,
                      selectedProjectId: 'openchat-project',
                      selectedConversationId: 'first-chat-experience',
                      onSelectProject: (_) {},
                      onSelectConversation: (_) {},
                      onOpenProjectOptions: (_) {},
                      onCreateProjectConversation: (_) {},
                      onShowMoreProjectConversations: (_) {},
                    ),
                    Expanded(
                      child: ConversationPane(
                        messageController: messageController,
                        showHistoryButton: false,
                        onOpenHistory: () {},
                        onSendMessage: () {},
                        providerId: 'chatgpt',
                        messages: figmaChatMessages,
                        conversationTitle: 'OpenChat sohbeti',
                        selectedModelLabel: 'Örnek 1',
                        assistantModelLabel: 'Örnek 1',
                        reasoningLevel: 'Orta',
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();
  if (expandedRail) {
    _expectFigmaSidebarGeometry(tester);
  } else if (sidebarWidth == OpenChatSpacing.compactSidebarWidth) {
    _expectNarrowFigmaSidebarGeometry(tester);
  } else {
    _expectCompactFigmaSidebarGeometry(tester);
  }
  final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
  addTearDown(mouse.removePointer);
  await mouse.addPointer(location: Offset.zero);
  await mouse.moveTo(
    tester.getCenter(
      find.byKey(const ValueKey<String>('sidebar-conversation-chat-interface')),
    ),
  );
  await tester.pumpAndSettle();

  await expectLater(
    find.byKey(const ValueKey<String>('active-screen-screenshot')),
    matchesGoldenFile(goldenPath),
  );
}

void _expectNarrowFigmaSidebarGeometry(WidgetTester tester) {
  expect(
    tester.getRect(find.byType(ChatNavigationRail)),
    Rect.fromLTWH(
      0,
      OpenChatSpacing.appTitleBarHeight,
      OpenChatSpacing.compactRailWidth,
      720 - OpenChatSpacing.appTitleBarHeight,
    ),
  );
  expect(
    tester.getRect(find.byType(ConversationSidebar)),
    Rect.fromLTWH(
      OpenChatSpacing.compactRailWidth,
      OpenChatSpacing.appTitleBarHeight,
      OpenChatSpacing.compactSidebarWidth,
      720 - OpenChatSpacing.appTitleBarHeight,
    ),
  );
  _expectSidebarContentFits(tester);
}

void _expectCompactFigmaSidebarGeometry(WidgetTester tester) {
  expect(
    tester.getRect(find.byType(ChatNavigationRail)),
    Rect.fromLTWH(
      0,
      OpenChatSpacing.appTitleBarHeight,
      OpenChatSpacing.compactRailWidth,
      900 - OpenChatSpacing.appTitleBarHeight,
    ),
  );
  expect(
    tester.getRect(find.byType(ConversationSidebar)),
    Rect.fromLTWH(
      OpenChatSpacing.compactRailWidth,
      OpenChatSpacing.appTitleBarHeight,
      OpenChatSpacing.sidebarWidth,
      900 - OpenChatSpacing.appTitleBarHeight,
    ),
  );
  _expectSidebarContentFits(tester);
}

void _expectFigmaSidebarGeometry(WidgetTester tester) {
  expect(
    tester.getRect(find.byType(ConversationSidebar)),
    Rect.fromLTWH(
      260,
      OpenChatSpacing.appTitleBarHeight,
      OpenChatSpacing.sidebarWidth,
      900 - OpenChatSpacing.appTitleBarHeight,
    ),
  );
  _expectSidebarContentFits(tester);
}

void _expectSidebarContentFits(WidgetTester tester) {
  final sidebar = tester.getRect(find.byType(ConversationSidebar));
  final itemKeys = [
    'sidebar-project-openchat-project',
    'sidebar-project-label-openchat-project',
    'sidebar-project-options-openchat-project',
    'sidebar-project-new-chat-openchat-project',
    'sidebar-conversation-first-chat-experience',
    'sidebar-conversation-chat-interface',
    'sidebar-conversation-light-and-dark-theme',
  ];

  for (final key in itemKeys) {
    final rect = tester.getRect(find.byKey(ValueKey<String>(key)));
    expect(rect.left, greaterThanOrEqualTo(sidebar.left));
    expect(rect.right, lessThanOrEqualTo(sidebar.right));
    expect(rect.top, greaterThanOrEqualTo(sidebar.top));
    expect(rect.bottom, lessThanOrEqualTo(sidebar.bottom));
  }

  final search = find.byKey(const ValueKey<String>('history-search-input'));
  if (search.evaluate().isNotEmpty) {
    final searchBounds = tester.getRect(search);
    expect(searchBounds.left, greaterThanOrEqualTo(sidebar.left));
    expect(searchBounds.right, lessThanOrEqualTo(sidebar.right));
  } else {
    final title = tester.getRect(
      find.byKey(const ValueKey<String>('conversation-sidebar-title')),
    );
    expect(title.left, greaterThanOrEqualTo(sidebar.left));
    expect(title.right, lessThanOrEqualTo(sidebar.right));
  }
}

Future<void> _ignoreThemeMode(ThemeMode _) async {}

Future<void> _ignoreThemeToggle() async {}

Future<void> _expectNarrowComposerGeometry(
  WidgetTester tester, {
  required ThemeData theme,
}) async {
  tester.view.physicalSize = const Size(928, 720);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);

  final messageController = TextEditingController();
  addTearDown(messageController.dispose);

  await tester.pumpWidget(
    MaterialApp(
      locale: const Locale('tr'),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      theme: theme.copyWith(platform: TargetPlatform.windows),
      home: Scaffold(
        body: ColoredBox(
          color: theme.scaffoldBackgroundColor,
          child: ConversationPane(
            messageController: messageController,
            showHistoryButton: false,
            onOpenHistory: () {},
            onSendMessage: () {},
            providerId: 'chatgpt',
            messages: figmaChatMessages,
            conversationTitle: 'OpenChat sohbeti',
            selectedModelLabel: 'Örnek 1',
            assistantModelLabel: 'Örnek 1',
            reasoningLevel: 'Orta',
          ),
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();

  _expectComposerControls(tester, outlinedButtonCount: 3);
}

Future<void> _expectPaneMatchesFigma(
  WidgetTester tester, {
  required ThemeData theme,
  required String goldenPath,
  List<ChatMessage> messages = const <ChatMessage>[],
  String? conversationTitle,
  String? selectedModelLabel,
  String? reasoningLevel,
}) async {
  tester.view.physicalSize = const Size(1100, 900);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);

  final messageController = TextEditingController();
  addTearDown(messageController.dispose);

  await tester.pumpWidget(
    MaterialApp(
      locale: const Locale('tr'),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      theme: theme.copyWith(platform: TargetPlatform.windows),
      home: Scaffold(
        body: RepaintBoundary(
          key: const ValueKey<String>('pane-screenshot'),
          child: ColoredBox(
            color: theme.scaffoldBackgroundColor,
            child: ConversationPane(
              messageController: messageController,
              showHistoryButton: false,
              onOpenHistory: () {},
              onSendMessage: () {},
              providerId: 'chatgpt',
              messages: messages,
              conversationTitle: conversationTitle,
              selectedModelLabel: selectedModelLabel,
              assistantModelLabel: selectedModelLabel,
              reasoningLevel: reasoningLevel,
            ),
          ),
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();

  _expectComposerGeometry(tester, hasReasoningSelector: true);

  await expectLater(
    find.byKey(const ValueKey<String>('pane-screenshot')),
    matchesGoldenFile(goldenPath),
  );
}

void _expectComposerGeometry(
  WidgetTester tester, {
  required bool hasReasoningSelector,
}) {
  _expectComposerControls(
    tester,
    outlinedButtonCount: hasReasoningSelector ? 3 : 2,
  );
}

void _expectComposerControls(
  WidgetTester tester, {
  required int outlinedButtonCount,
}) {
  final composer = find.byType(ChatComposer);
  final composerRect = tester.getRect(composer);
  final messageField = tester.getRect(
    find.descendant(of: composer, matching: find.byType(TextField)),
  );
  final outlinedButtons = find.byType(OutlinedButton);
  expect(outlinedButtons, findsNWidgets(outlinedButtonCount));

  final controlRects = <Rect>[
    for (final element in outlinedButtons.evaluate())
      tester.getRect(find.byWidget(element.widget)),
    tester.getRect(find.byType(FilledButton)),
  ];
  expect(controlRects, hasLength(outlinedButtonCount + 1));

  bool isInsideComposer(Rect rect) =>
      composerRect.contains(rect.topLeft) &&
      composerRect.contains(Offset(rect.right - 0.5, rect.bottom - 0.5));

  expect(isInsideComposer(messageField), isTrue);
  for (var index = 0; index < controlRects.length; index++) {
    expect(isInsideComposer(controlRects[index]), isTrue);
    expect(messageField.overlaps(controlRects[index]), isFalse);
    for (final other in controlRects.skip(index + 1)) {
      expect(controlRects[index].overlaps(other), isFalse);
    }
  }
}

// Keep screenshot comparison pixel-exact; geometry assertions identify layout
// mismatches separately from differences in text rasterization.
class _FigmaRenderingComparator extends LocalFileComparator {
  _FigmaRenderingComparator(super.testFile);

  @override
  Future<bool> compare(Uint8List imageBytes, Uri golden) async {
    final result = await GoldenFileComparator.compareLists(
      imageBytes,
      await getGoldenBytes(golden),
    );
    if (result.diffPercent == 0) {
      result.dispose();
      return true;
    }

    final error = await generateFailureOutput(result, golden, basedir);
    result.dispose();
    throw FlutterError(error);
  }
}
