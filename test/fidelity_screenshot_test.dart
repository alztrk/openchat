import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:zihora/app/zihora_theme.dart';
import 'package:zihora/features/chat/domain/chat_message.dart';
import 'package:zihora/features/chat/domain/history_storage_status.dart';
import 'package:zihora/features/chat/presentation/chat_screen.dart';
import 'package:zihora/features/chat/presentation/widgets/chat_composer.dart';
import 'package:zihora/features/chat/presentation/widgets/chat_navigation_rail.dart';
import 'package:zihora/features/chat/presentation/widgets/conversation_pane.dart';
import 'package:zihora/features/chat/presentation/widgets/conversation_sidebar.dart';
import 'package:zihora/l10n/generated/app_localizations.dart';

import 'fixtures/figma_chat_messages.dart';
import 'fixtures/figma_sidebar_items.dart';

late GoldenFileComparator _previousGoldenComparator;

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    _previousGoldenComparator = goldenFileComparator;
    goldenFileComparator = _FigmaRenderingComparator(
      Uri.parse('test/fidelity_screenshot_test.dart'),
    );
    final fontLoader = FontLoader('Manrope')
      ..addFont(rootBundle.load('assets/fonts/Manrope[wght].ttf'));
    await fontLoader.load();
  });
  tearDownAll(() => goldenFileComparator = _previousGoldenComparator);

  for (final appearance in [
    (name: 'light', theme: ZihoraTheme.light),
    (name: 'dark', theme: ZihoraTheme.dark),
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
        conversationTitle: 'Zihora sohbeti',
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
        sidebarWidth: 280,
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
        sidebarWidth: 320,
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
    (name: 'wide', width: 1680.0, height: 900.0, rail: 260.0, sidebar: 320.0),
    (name: 'compact', width: 1492.0, height: 900.0, rail: 72.0, sidebar: 320.0),
    (name: 'narrow', width: 1280.0, height: 720.0, rail: 72.0, sidebar: 280.0),
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
}

Future<void> _expectCompactNavigationRailMatchesFigma(
  WidgetTester tester, {
  required ThemeData theme,
  required String goldenPath,
}) async {
  tester.view.physicalSize = const Size(72, 900);
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
    const Rect.fromLTWH(14, 75, 44, 44),
  );
  expect(
    tester.getRect(find.byType(ChatNavigationRail)),
    const Rect.fromLTWH(0, 0, 72, 900),
  );
  expect(
    tester.getRect(find.byKey(const ValueKey<String>('compact-brand'))),
    const Rect.fromLTWH(14, 21, 44, 44),
  );
  expect(find.text('Zihora'), findsNothing);

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
      theme: ZihoraTheme.light.copyWith(platform: TargetPlatform.windows),
      home: const ChatScreen(
        themeMode: ThemeMode.light,
        onThemeModeChanged: _ignoreThemeMode,
        onToggleTheme: _ignoreThemeToggle,
        historyStorageStatus: HistoryStorageStatus.available,
      ),
    ),
  );
  await tester.pumpAndSettle();

  expect(
    tester.getRect(find.byType(ChatNavigationRail)),
    Rect.fromLTWH(0, 0, railWidth, height),
  );
  expect(
    tester.getRect(find.byType(ConversationSidebar)),
    Rect.fromLTWH(railWidth, 0, sidebarWidth, height),
  );
  expect(
    tester.getRect(
      find.descendant(
        of: find.byType(ConversationSidebar),
        matching: find.byType(Divider),
      ),
    ),
    Rect.fromLTWH(railWidth + 20, 114, sidebarWidth - 40, 1),
  );
  expect(
    tester.getRect(find.byType(ConversationPane)),
    Rect.fromLTWH(
      railWidth + sidebarWidth,
      0,
      width - railWidth - sidebarWidth,
      height,
    ),
  );
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
}) async {
  tester.view.physicalSize = const Size(1680, 900);
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
  await tester.tap(find.text('Ayarlar').first);
  await tester.pumpAndSettle();

  await expectLater(
    find.byKey(const ValueKey<String>('settings-screen-screenshot')),
    matchesGoldenFile(goldenPath),
  );
}

Future<void> _expectActiveScreenMatchesFigma(
  WidgetTester tester, {
  required ThemeData theme,
  required String goldenPath,
  Size size = const Size(1680, 900),
  bool expandedRail = true,
  double sidebarWidth = ZihoraSpacing.sidebarWidth,
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
          body: Row(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              ChatNavigationRail(
                expanded: expandedRail,
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
                selectedProjectId: 'zihora-project',
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
                  messages: figmaChatMessages,
                  conversationTitle: 'Zihora sohbeti',
                  selectedModelLabel: 'Örnek 1',
                  reasoningLevel: 'Orta',
                  showWindowControls: true,
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
  } else if (sidebarWidth == ZihoraSpacing.compactSidebarWidth) {
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
    const Rect.fromLTWH(0, 0, 72, 720),
  );
  expect(
    tester.getRect(find.byKey(const ValueKey<String>('compact-brand'))),
    const Rect.fromLTWH(14, 21, 44, 44),
  );
  expect(
    tester.getRect(
      find.descendant(
        of: find.byType(ConversationSidebar),
        matching: find.byType(TextField),
      ),
    ),
    const Rect.fromLTWH(92, 58, 240, 36),
  );
  expect(
    tester.getRect(
      find.byKey(const ValueKey<String>('sidebar-project-zihora-project')),
    ),
    const Rect.fromLTWH(92, 158, 232, 38),
  );
  expect(
    tester.getRect(
      find.byKey(
        const ValueKey<String>('sidebar-project-options-zihora-project'),
      ),
    ),
    const Rect.fromLTWH(256, 163, 24, 28),
  );
  expect(
    tester.getRect(
      find.byKey(
        const ValueKey<String>('sidebar-project-new-chat-zihora-project'),
      ),
    ),
    const Rect.fromLTWH(281, 163, 24, 28),
  );
  expect(
    tester.getRect(
      find.byKey(
        const ValueKey<String>('sidebar-conversation-first-chat-experience'),
      ),
    ),
    const Rect.fromLTWH(104, 204, 220, 32),
  );
  expect(
    tester.getRect(
      find.byKey(const ValueKey<String>('sidebar-conversation-chat-interface')),
    ),
    const Rect.fromLTWH(92, 378, 232, 34),
  );
}

void _expectCompactFigmaSidebarGeometry(WidgetTester tester) {
  expect(
    tester.getRect(find.byType(ChatNavigationRail)),
    const Rect.fromLTWH(0, 0, 72, 900),
  );
  expect(
    tester.getRect(find.byKey(const ValueKey<String>('compact-brand'))),
    const Rect.fromLTWH(14, 21, 44, 44),
  );
  expect(
    tester.getRect(
      find.descendant(
        of: find.byType(ConversationSidebar),
        matching: find.byType(TextField),
      ),
    ),
    const Rect.fromLTWH(92, 58, 280, 36),
  );
  expect(
    tester.getRect(
      find.byKey(const ValueKey<String>('sidebar-project-zihora-project')),
    ),
    const Rect.fromLTWH(92, 158, 272, 38),
  );
  expect(
    tester.getRect(
      find.byKey(
        const ValueKey<String>('sidebar-project-options-zihora-project'),
      ),
    ),
    const Rect.fromLTWH(296, 163, 24, 28),
  );
  expect(
    tester.getRect(
      find.byKey(
        const ValueKey<String>('sidebar-project-new-chat-zihora-project'),
      ),
    ),
    const Rect.fromLTWH(321, 163, 24, 28),
  );
  expect(
    tester.getRect(
      find.byKey(
        const ValueKey<String>('sidebar-conversation-first-chat-experience'),
      ),
    ),
    const Rect.fromLTWH(104, 204, 260, 32),
  );
  expect(
    tester.getRect(
      find.byKey(const ValueKey<String>('sidebar-conversation-chat-interface')),
    ),
    const Rect.fromLTWH(92, 378, 272, 34),
  );
}

void _expectFigmaSidebarGeometry(WidgetTester tester) {
  expect(
    tester.getRect(
      find.descendant(
        of: find.byType(ConversationSidebar),
        matching: find.byType(TextField),
      ),
    ),
    const Rect.fromLTWH(280, 58, 280, 36),
  );
  expect(
    tester.getRect(
      find.byKey(const ValueKey<String>('sidebar-project-zihora-project')),
    ),
    const Rect.fromLTWH(280, 158, 272, 38),
  );
  expect(
    tester.getRect(
      find.byKey(
        const ValueKey<String>('sidebar-project-label-zihora-project'),
      ),
    ),
    const Rect.fromLTWH(292, 167, 184, 20),
  );
  expect(
    tester.getRect(
      find.byKey(
        const ValueKey<String>('sidebar-project-options-zihora-project'),
      ),
    ),
    const Rect.fromLTWH(484, 163, 24, 28),
  );
  expect(
    tester.getRect(
      find.byKey(
        const ValueKey<String>('sidebar-project-new-chat-zihora-project'),
      ),
    ),
    const Rect.fromLTWH(509, 163, 24, 28),
  );
  expect(
    tester.getRect(
      find.byKey(
        const ValueKey<String>('sidebar-conversation-first-chat-experience'),
      ),
    ),
    const Rect.fromLTWH(292, 204, 260, 32),
  );
  expect(
    tester.getRect(
      find.byKey(const ValueKey<String>('sidebar-conversation-chat-interface')),
    ),
    const Rect.fromLTWH(280, 378, 272, 34),
  );
  expect(
    tester.getRect(
      find.byKey(
        const ValueKey<String>('sidebar-conversation-light-and-dark-theme'),
      ),
    ),
    const Rect.fromLTWH(280, 498, 272, 32),
  );
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
            messages: figmaChatMessages,
            conversationTitle: 'Zihora sohbeti',
            selectedModelLabel: 'Örnek 1',
            reasoningLevel: 'Orta',
            showWindowControls: true,
          ),
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();

  expect(
    tester.getRect(find.byType(ChatComposer)),
    const Rect.fromLTWH(32, 580, 864, 116),
  );
  expect(
    tester.getRect(find.byType(TextField)),
    const Rect.fromLTWH(48, 592, 832, 20),
  );

  final outlinedButtons = find.byType(OutlinedButton);
  expect(outlinedButtons, findsNWidgets(3));
  expect(
    tester.getRect(outlinedButtons.at(0)),
    const Rect.fromLTWH(48, 644, 132, 36),
  );
  expect(
    tester.getRect(outlinedButtons.at(1)),
    const Rect.fromLTWH(192, 644, 176, 36),
  );
  expect(
    tester.getRect(outlinedButtons.at(2)),
    const Rect.fromLTWH(740, 644, 36, 36),
  );
  expect(
    tester.getRect(find.byType(FilledButton)),
    const Rect.fromLTWH(784, 644, 96, 36),
  );
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
              messages: messages,
              conversationTitle: conversationTitle,
              selectedModelLabel: selectedModelLabel,
              reasoningLevel: reasoningLevel,
              showWindowControls: true,
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
  expect(
    tester.getRect(find.byType(ChatComposer)),
    const Rect.fromLTWH(32, 776, 1036, 100),
  );
  expect(
    tester.getRect(find.byType(TextField)),
    const Rect.fromLTWH(51, 793, 998, 20),
  );

  final outlinedButtons = find.byType(OutlinedButton);
  expect(outlinedButtons, findsNWidgets(hasReasoningSelector ? 3 : 2));
  expect(
    tester.getRect(outlinedButtons.at(0)),
    const Rect.fromLTWH(51, 825, 132, 36),
  );
  if (hasReasoningSelector) {
    expect(
      tester.getRect(outlinedButtons.at(1)),
      const Rect.fromLTWH(195, 825, 176, 36),
    );
  }
  expect(
    tester.getRect(outlinedButtons.last),
    Rect.fromLTWH(hasReasoningSelector ? 909 : 921, 825, 36, 36),
  );
  expect(
    tester.getRect(find.byType(FilledButton)),
    Rect.fromLTWH(
      hasReasoningSelector ? 953 : 965,
      825,
      hasReasoningSelector ? 96 : 84,
      36,
    ),
  );
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
