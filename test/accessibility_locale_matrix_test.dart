import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/presentation/widgets/assistant_message.dart';
import 'package:openchat/features/chat/presentation/widgets/chat_composer.dart';
import 'package:openchat/features/chat/domain/conversation_sidebar_data.dart';
import 'package:openchat/features/chat/presentation/widgets/conversation_sidebar.dart';
import 'package:openchat/features/chat/presentation/widgets/sidebar_conversation_tile.dart';
import 'package:openchat/features/chat/domain/tool_permission_request.dart';
import 'package:openchat/features/chat/presentation/widgets/tool_permission_card.dart';
import 'package:openchat/features/chat/presentation/widgets/project_tool_permissions_dialog.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

import 'support/shad_test_scope.dart';

void main() {
  testWidgets(
    'chat composer send action is labeled in every supported locale',
    (tester) async {
      final controller = TextEditingController(text: 'Send this message');
      addTearDown(controller.dispose);
      final semantics = tester.ensureSemantics();
      try {
        for (final locale in AppLocalizations.supportedLocales) {
          await tester.pumpWidget(
            MaterialApp(
              key: ValueKey<String>('composer-${locale.languageCode}'),
              locale: locale,
              localizationsDelegates: AppLocalizations.localizationsDelegates,
              supportedLocales: AppLocalizations.supportedLocales,
              theme: OpenChatTheme.dark,
              builder: (context, child) => openChatShadTestScope(
                OpenChatTheme.dark,
                child ?? const SizedBox.shrink(),
              ),
              home: Scaffold(
                body: ChatComposer(
                  controller: controller,
                  onSendMessage: () {},
                  canSendMessage: true,
                  showReasoningSelector: false,
                  providerId: 'opencode',
                ),
              ),
            ),
          );
          await tester.pumpAndSettle();

          final localizations = AppLocalizations.of(
            tester.element(find.byType(ChatComposer)),
          )!;
          expect(
            find.bySemanticsLabel(localizations.send),
            findsOneWidget,
            reason: locale.languageCode,
          );
          expect(tester.takeException(), isNull);
        }
      } finally {
        semantics.dispose();
      }
    },
  );

  testWidgets(
    'chat composer stop action is labeled and keyboard-operable in every locale',
    (tester) async {
      final semantics = tester.ensureSemantics();
      final controller = TextEditingController(text: 'In flight');
      addTearDown(controller.dispose);
      try {
        for (final locale in AppLocalizations.supportedLocales) {
          var stopped = 0;
          await tester.pumpWidget(
            MaterialApp(
              key: ValueKey<String>('composer-stop-${locale.languageCode}'),
              locale: locale,
              localizationsDelegates: AppLocalizations.localizationsDelegates,
              supportedLocales: AppLocalizations.supportedLocales,
              theme: OpenChatTheme.dark,
              builder: (context, child) => openChatShadTestScope(
                OpenChatTheme.dark,
                child ?? const SizedBox.shrink(),
              ),
              home: Scaffold(
                body: FocusTraversalGroup(
                  child: Column(
                    children: [
                      TextButton(onPressed: () {}, child: const Text('Before')),
                      ChatComposer(
                        controller: controller,
                        onSendMessage: () {},
                        canSendMessage: false,
                        isSending: true,
                        onStopMessage: () => stopped++,
                        showReasoningSelector: false,
                        providerId: 'opencode',
                      ),
                    ],
                  ),
                ),
              ),
            ),
          );
          await tester.pumpAndSettle();

          final l10n = AppLocalizations.of(
            tester.element(find.byType(ChatComposer)),
          )!;
          expect(find.bySemanticsLabel(l10n.stop), findsOneWidget);

          for (var attempt = 0; attempt < 12 && stopped == 0; attempt++) {
            await tester.sendKeyEvent(LogicalKeyboardKey.tab);
            await tester.sendKeyEvent(LogicalKeyboardKey.enter);
          }
          expect(stopped, 1, reason: locale.languageCode);
          expect(tester.takeException(), isNull);
        }
      } finally {
        semantics.dispose();
      }
    },
  );

  testWidgets('composer stop action is disabled when no stop callback exists', (
    tester,
  ) async {
    final semantics = tester.ensureSemantics();
    final controller = TextEditingController(text: 'In flight');
    try {
      await tester.pumpWidget(
        MaterialApp(
          locale: const Locale('en'),
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          theme: OpenChatTheme.dark,
          builder: (context, child) => openChatShadTestScope(
            OpenChatTheme.dark,
            child ?? const SizedBox.shrink(),
          ),
          home: Scaffold(
            body: ChatComposer(
              controller: controller,
              onSendMessage: () {},
              canSendMessage: false,
              isSending: true,
              showReasoningSelector: false,
              providerId: 'opencode',
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      final l10n = AppLocalizations.of(
        tester.element(find.byType(ChatComposer)),
      )!;
      final stopAction = find.bySemanticsLabel(l10n.stop);
      expect(stopAction, findsOneWidget);
      expect(
        tester.getSemantics(stopAction).flagsCollection.isEnabled,
        ui.Tristate.isFalse,
      );
    } finally {
      semantics.dispose();
      controller.dispose();
    }
  });

  testWidgets(
    'assistant response actions stay labeled and operable at enlarged text in every locale',
    (tester) async {
      final semantics = tester.ensureSemantics();
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      tester.view.physicalSize = const Size(320, 640);
      tester.view.devicePixelRatio = 1;
      try {
        for (final locale in AppLocalizations.supportedLocales) {
          var selectedVersion = -1;
          await tester.pumpWidget(
            MaterialApp(
              key: ValueKey<String>('assistant-actions-${locale.languageCode}'),
              locale: locale,
              localizationsDelegates: AppLocalizations.localizationsDelegates,
              supportedLocales: AppLocalizations.supportedLocales,
              theme: OpenChatTheme.dark,
              home: MediaQuery(
                data: const MediaQueryData(
                  size: Size(320, 640),
                  textScaler: TextScaler.linear(2),
                  disableAnimations: true,
                ),
                child: Scaffold(
                  body: SingleChildScrollView(
                    child: AssistantMessage(
                      message: const ChatMessage(
                        id: 'assistant-a11y',
                        role: ChatMessageRole.assistant,
                        content:
                            'A readable response for assistive technology.',
                        providerId: 'chatgpt',
                        modelId: 'model-1',
                      ),
                      modelLabel: 'model-1',
                      providerId: 'chatgpt',
                      onRetry: () {},
                      responseVersionIndex: 0,
                      responseVersionCount: 2,
                      onSelectResponseVersion: (index) {
                        selectedVersion = index;
                      },
                    ),
                  ),
                ),
              ),
            ),
          );
          await tester.pumpAndSettle();

          final l10n = AppLocalizations.of(
            tester.element(find.byType(AssistantMessage)),
          )!;
          expect(
            find.bySemanticsLabel(
              'A readable response for assistive technology.',
            ),
            findsOneWidget,
          );
          _expectAccessibleButton(tester, find.byTooltip(l10n.copyMessage));
          _expectAccessibleButton(tester, find.byTooltip(l10n.retry));
          _expectAccessibleButton(
            tester,
            find.byTooltip(l10n.previousResponseVersion),
          );
          _expectAccessibleButton(
            tester,
            find.byTooltip(l10n.nextResponseVersion),
          );
          await tester.tap(find.byTooltip(l10n.nextResponseVersion));
          expect(selectedVersion, 1, reason: locale.languageCode);
          expect(tester.takeException(), isNull);
        }
      } finally {
        semantics.dispose();
      }
    },
  );

  testWidgets(
    'chat composer sends from the keyboard in every supported locale',
    (tester) async {
      final controller = TextEditingController();
      addTearDown(controller.dispose);

      for (final locale in AppLocalizations.supportedLocales) {
        var sent = 0;
        await tester.pumpWidget(
          MaterialApp(
            key: ValueKey<String>('keyboard-composer-${locale.languageCode}'),
            locale: locale,
            localizationsDelegates: AppLocalizations.localizationsDelegates,
            supportedLocales: AppLocalizations.supportedLocales,
            theme: OpenChatTheme.dark,
            builder: (context, child) => openChatShadTestScope(
              OpenChatTheme.dark,
              child ?? const SizedBox.shrink(),
            ),
            home: Scaffold(
              body: ChatComposer(
                controller: controller,
                onSendMessage: () => sent++,
                canSendMessage: true,
                showReasoningSelector: false,
                providerId: 'opencode',
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();

        final field = find.byType(EditableText);
        await tester.tap(field);
        await tester.enterText(field, 'Keyboard message');
        await tester.sendKeyEvent(LogicalKeyboardKey.enter);
        expect(sent, 1, reason: locale.languageCode);
        expect(tester.takeException(), isNull);
        controller.clear();
      }
    },
  );

  testWidgets('conversation sidebar selection works with keyboard focus', (
    tester,
  ) async {
    var selected = false;
    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark,
        home: Scaffold(
          body: FocusTraversalGroup(
            child: Column(
              children: [
                TextButton(onPressed: () {}, child: const Text('Before')),
                SidebarConversationTile(
                  conversation: const ConversationSidebarConversation(
                    id: 'keyboard-selection',
                    title: 'Keyboard selection conversation',
                  ),
                  selected: false,
                  height: 40,
                  inset: 8,
                  onPressed: () => selected = true,
                ),
              ],
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();

    expect(selected, isTrue);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'tool approval and denial actions work from the keyboard in every locale',
    (tester) async {
      for (final locale in AppLocalizations.supportedLocales) {
        var approved = 0;
        var denied = 0;
        await tester.pumpWidget(
          MaterialApp(
            key: ValueKey<String>('tool-permission-${locale.languageCode}'),
            locale: locale,
            localizationsDelegates: AppLocalizations.localizationsDelegates,
            supportedLocales: AppLocalizations.supportedLocales,
            theme: OpenChatTheme.dark,
            home: Scaffold(
              body: FocusTraversalGroup(
                child: Column(
                  children: [
                    TextButton(onPressed: () {}, child: const Text('Before')),
                    ToolPermissionCard(
                      request: const ToolPermissionRequest(
                        id: 'keyboard-permission',
                        toolName: 'write_file',
                        targetPath: 'notes.txt',
                        arguments: <String, Object?>{
                          'path': 'notes.txt',
                          'content': 'reviewed content',
                        },
                      ),
                      isResponding: false,
                      onApprove: () => approved++,
                      onDeny: () => denied++,
                    ),
                  ],
                ),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();

        await tester.sendKeyEvent(LogicalKeyboardKey.tab);
        await tester.sendKeyEvent(LogicalKeyboardKey.tab);
        await tester.sendKeyEvent(LogicalKeyboardKey.enter);
        expect(denied, 1, reason: 'deny ${locale.languageCode}');

        await tester.sendKeyEvent(LogicalKeyboardKey.tab);
        await tester.sendKeyEvent(LogicalKeyboardKey.enter);
        expect(approved, 1, reason: 'approve ${locale.languageCode}');
        expect(tester.takeException(), isNull);
      }
    },
  );

  testWidgets(
    'provider citation source actions are keyboard accessible in every locale',
    (tester) async {
      final semantics = tester.ensureSemantics();
      try {
        for (final locale in AppLocalizations.supportedLocales) {
          final l10n = await AppLocalizations.delegate.load(locale);
          await tester.pumpWidget(
            MaterialApp(
              key: ValueKey<String>('citation-keyboard-${locale.languageCode}'),
              locale: locale,
              localizationsDelegates: AppLocalizations.localizationsDelegates,
              supportedLocales: AppLocalizations.supportedLocales,
              theme: OpenChatTheme.dark,
              home: Scaffold(
                body: FocusTraversalGroup(
                  child: Column(
                    children: [
                      TextButton(onPressed: () {}, child: const Text('Before')),
                      const AssistantMessage(
                        message: ChatMessage(
                          id: 'citation-keyboard',
                          role: ChatMessageRole.assistant,
                          content: 'Claim supported by [P1].',
                          providerId: 'gemini',
                          modelId: 'selected-model',
                          citationSources: <ChatCitationSource>[
                            ChatCitationSource(
                              id: 'P1',
                              title: 'Provider source title',
                              url: 'https://example.org/source',
                              sourceType: 'provider_native',
                            ),
                          ],
                        ),
                        modelLabel: 'selected-model',
                        providerId: 'gemini',
                      ),
                    ],
                  ),
                ),
              ),
            ),
          );
          await tester.pumpAndSettle();

          expect(
            find.bySemanticsLabel(l10n.toolCitationSource('P1')),
            findsOneWidget,
            reason: locale.languageCode,
          );
          await tester.sendKeyEvent(LogicalKeyboardKey.tab);
          await tester.sendKeyEvent(LogicalKeyboardKey.tab);
          await tester.sendKeyEvent(LogicalKeyboardKey.enter);
          await tester.pumpAndSettle();

          expect(find.text('Provider source title'), findsOneWidget);
          expect(find.text('https://example.org/source'), findsOneWidget);
          expect(tester.takeException(), isNull);
        }
      } finally {
        semantics.dispose();
      }
    },
  );

  testWidgets(
    'conversation selection stays labeled across locales, widths, and text scales',
    (tester) async {
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final semantics = tester.ensureSemantics();
      final searchController = TextEditingController();
      addTearDown(searchController.dispose);
      try {
        for (final locale in AppLocalizations.supportedLocales) {
          for (final width in <double>[320, 480, 1200]) {
            for (final scale in <double>[1, 1.5, 2]) {
              tester.view.devicePixelRatio = 1;
              tester.view.physicalSize = Size(width, 900);
              await tester.pumpWidget(
                MaterialApp(
                  key: ValueKey<String>('${locale.languageCode}-$width-$scale'),
                  locale: locale,
                  localizationsDelegates:
                      AppLocalizations.localizationsDelegates,
                  supportedLocales: AppLocalizations.supportedLocales,
                  theme: OpenChatTheme.dark,
                  home: MediaQuery(
                    data: MediaQueryData(
                      size: Size(width, 900),
                      textScaler: TextScaler.linear(scale),
                      disableAnimations: true,
                    ),
                    child: Scaffold(
                      body: ConversationSidebar(
                        width: 300,
                        searchController: searchController,
                        conversations: const <ConversationSidebarConversation>[
                          ConversationSidebarConversation(
                            id: 'selection-matrix',
                            title: 'Selection matrix conversation',
                            isBookmarked: true,
                            tags: <String>['Research'],
                          ),
                        ],
                        selectionMode: true,
                        selectedConversationIds: const <String>{},
                        onToggleSelectionMode: () {},
                        onToggleBatchSelection: (_) {},
                      ),
                    ),
                  ),
                ),
              );
              await tester.pumpAndSettle();
              final localizations = AppLocalizations.of(
                tester.element(find.byType(ConversationSidebar)),
              )!;
              expect(
                find.bySemanticsLabel(
                  localizations.selectConversation(
                    'Selection matrix conversation',
                  ),
                ),
                findsOneWidget,
                reason: '${locale.languageCode}, width $width, scale $scale',
              );
              final tileSemantics = tester.widget<Semantics>(
                find
                    .descendant(
                      of: find.byType(SidebarConversationTile),
                      matching: find.byType(Semantics),
                    )
                    .first,
              );
              expect(
                tileSemantics.properties.label,
                'Selection matrix conversation, Research, '
                '${localizations.conversationBookmarked}',
              );
              expect(tester.takeException(), isNull);
            }
          }
        }
      } finally {
        semantics.dispose();
      }
    },
  );

  testWidgets(
    'provider citation responses remain readable across locales and display scales',
    (tester) async {
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final semantics = tester.ensureSemantics();
      try {
        for (final locale in AppLocalizations.supportedLocales) {
          final localizations = await AppLocalizations.delegate.load(locale);
          for (final width in <double>[320, 480, 1200]) {
            for (final scale in <double>[1, 1.5, 2]) {
              for (final ratio in <double>[1, 1.5, 2]) {
                tester.view.devicePixelRatio = ratio;
                tester.view.physicalSize = Size(width * ratio, 900 * ratio);
                await tester.pumpWidget(
                  MaterialApp(
                    key: ValueKey<String>(
                      '${locale.languageCode}-$width-$scale-$ratio',
                    ),
                    locale: locale,
                    localizationsDelegates:
                        AppLocalizations.localizationsDelegates,
                    supportedLocales: AppLocalizations.supportedLocales,
                    theme: OpenChatTheme.dark,
                    home: MediaQuery(
                      data: MediaQueryData(
                        size: Size(width, 900),
                        devicePixelRatio: ratio,
                        textScaler: TextScaler.linear(scale),
                        disableAnimations: true,
                      ),
                      child: Scaffold(
                        body: SingleChildScrollView(
                          child: AssistantMessage(
                            message: const ChatMessage(
                              id: 'citation-matrix',
                              role: ChatMessageRole.assistant,
                              content: 'Kaynaklandırılmış yanıt [P1].',
                              providerId: 'chatgpt',
                              modelId: 'model-1',
                              citationSources: <ChatCitationSource>[
                                ChatCitationSource(
                                  id: 'P1',
                                  title: 'A provider supplied source with a deliberately long title',
                                  url: 'https://example.org/article',
                                  sourceType: 'provider_native',
                                ),
                              ],
                            ),
                            modelLabel: 'model-1',
                            providerId: 'chatgpt',
                          ),
                        ),
                      ),
                    ),
                  ),
                );
                await tester.pumpAndSettle();

                final citations = find.byWidgetPredicate(
                  (widget) =>
                      widget is RichText &&
                      widget.text.toPlainText().contains(
                        'Kaynaklandırılmış yanıt P1.',
                      ),
                );
                expect(citations, findsOneWidget);
                expect(
                  find.bySemanticsLabel(localizations.toolCitationSource('P1')),
                  findsOneWidget,
                  reason:
                      '${locale.languageCode}, width $width, scale $scale, ratio $ratio',
                );
                expect(tester.takeException(), isNull);
              }
            }
          }
        }
      } finally {
        semantics.dispose();
      }
    },
  );

  testWidgets('system high contrast selects accessible light and dark colors', (
    tester,
  ) async {
    tester.binding.platformDispatcher.accessibilityFeaturesTestValue =
        const FakeAccessibilityFeatures(highContrast: true);
    addTearDown(
      tester.binding.platformDispatcher.clearAccessibilityFeaturesTestValue,
    );
    for (final brightness in Brightness.values) {
      ThemeData? activeTheme;
      final dark = brightness == Brightness.dark;
      await tester.pumpWidget(
        MaterialApp(
          locale: const Locale('en'),
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          theme: OpenChatTheme.light,
          darkTheme: OpenChatTheme.dark,
          highContrastTheme: OpenChatTheme.highContrastLight,
          highContrastDarkTheme: OpenChatTheme.highContrastDark,
          themeMode: dark ? ThemeMode.dark : ThemeMode.light,
          home: Builder(
            builder: (context) {
              activeTheme = Theme.of(context);
              return Scaffold(
                body: ToolPermissionCard(
                  request: const ToolPermissionRequest(
                    id: 'high-contrast-approval',
                    toolName: 'git_status',
                    targetPath: 'project',
                    arguments: <String, Object?>{},
                  ),
                  isResponding: false,
                  onApprove: () {},
                  onDeny: () {},
                ),
              );
            },
          ),
        ),
      );
      await tester.pumpAndSettle();

      final theme = activeTheme;
      expect(theme, isNotNull);
      final palette = theme!.extension<OpenChatPalette>()!;
      final semantic = theme.extension<OpenChatSemanticColors>()!;
      expect(
        palette.text,
        dark
            ? OpenChatPalette.highContrastDark.text
            : OpenChatPalette.highContrastLight.text,
      );
      expect(
        _contrastRatio(palette.text, palette.surface),
        greaterThanOrEqualTo(7),
      );
      expect(
        _contrastRatio(palette.secondaryText, palette.surface),
        greaterThanOrEqualTo(4.5),
      );
      expect(
        _contrastRatio(semantic.focusRing, semantic.background),
        greaterThanOrEqualTo(4.5),
      );
      expect(tester.takeException(), isNull);
    }
  });

  testWidgets(
    'tool approval exposes localized actions across supported locales at 150% text scale',
    (tester) async {
      tester.view.physicalSize = const Size(1200, 900);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final semantics = tester.ensureSemantics();
      try {
        for (final locale in AppLocalizations.supportedLocales) {
          final l10n = await AppLocalizations.delegate.load(locale);
          for (final width in <double>[480, 1200]) {
            for (final textScale in <double>[1, 1.25, 1.5, 2]) {
              tester.view.physicalSize = Size(width, 900);
              await tester.pumpWidget(
                MaterialApp(
                  locale: locale,
                  localizationsDelegates:
                      AppLocalizations.localizationsDelegates,
                  supportedLocales: AppLocalizations.supportedLocales,
                  theme: OpenChatTheme.dark,
                  home: MediaQuery(
                    data: MediaQueryData(
                      textScaler: TextScaler.linear(textScale),
                    ),
                    child: Scaffold(
                      body: Padding(
                        padding: const EdgeInsets.all(16),
                        child: ToolPermissionCard(
                          request: const ToolPermissionRequest(
                            id: 'approval-accessibility',
                            toolName: 'git_status',
                            targetPath: r'D:\projects\OpenChat',
                            arguments: <String, Object?>{},
                          ),
                          isResponding: false,
                          onApprove: () {},
                          onDeny: () {},
                        ),
                      ),
                    ),
                  ),
                ),
              );
              await tester.pumpAndSettle();

              expect(
                find.bySemanticsLabel(l10n.toolPermissionDeny),
                findsOneWidget,
                reason:
                    '${locale.languageCode} at width $width and text scale $textScale',
              );
              expect(
                find.bySemanticsLabel(l10n.toolPermissionAllowOnce),
                findsOneWidget,
                reason:
                    '${locale.languageCode} at width $width and text scale $textScale',
              );
              expect(find.text(l10n.toolGitStatus), findsOneWidget);
              expect(tester.takeException(), isNull);
            }
          }
        }
      } finally {
        semantics.dispose();
      }
    },
  );

  testWidgets(
    'project tool permission dialog lays out across locales, widths, and text scales',
    (tester) async {
      tester.view.physicalSize = const Size(1200, 900);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final semantics = tester.ensureSemantics();
      try {
        for (final locale in AppLocalizations.supportedLocales) {
          final l10n = await AppLocalizations.delegate.load(locale);
          for (final width in <double>[480, 1200]) {
            for (final textScale in <double>[1, 1.25, 1.5, 2]) {
              tester.view.physicalSize = Size(width, 900);
              await tester.pumpWidget(
                MaterialApp(
                  locale: locale,
                  localizationsDelegates:
                      AppLocalizations.localizationsDelegates,
                  supportedLocales: AppLocalizations.supportedLocales,
                  theme: OpenChatTheme.dark,
                  builder: (context, child) => openChatShadTestScope(
                    OpenChatTheme.dark,
                    MediaQuery(
                      data: MediaQuery.of(context)
                          .copyWith(textScaler: TextScaler.linear(textScale)),
                      child: child ?? const SizedBox.shrink(),
                    ),
                  ),
                  home: Builder(
                    builder: (context) => Scaffold(
                      body: TextButton(
                        onPressed: () => showDialog<void>(
                          context: context,
                          builder: (context) => ProjectToolPermissionsDialog(
                            initialRules: const <String, ToolPermissionRule>{},
                            onSave: (_) async {},
                          ),
                        ),
                        child: const Text('Open rules'),
                      ),
                    ),
                  ),
                ),
              );
              await tester.tap(find.text('Open rules'));
              await tester.pumpAndSettle();

              expect(find.text(l10n.projectToolRulesTitle), findsOneWidget);
              expect(tester.takeException(), isNull);
              await tester.pumpWidget(const SizedBox.shrink());
            }
          }
        }
      } finally {
        semantics.dispose();
      }
    },
  );
}

void _expectAccessibleButton(WidgetTester tester, Finder finder) {
  expect(finder, findsOneWidget);
  expect(
    tester.getSemantics(finder).getSemanticsData().flagsCollection.isButton,
    isTrue,
  );
}

double _contrastRatio(Color foreground, Color background) {
  final foregroundLuminance = foreground.computeLuminance();
  final backgroundLuminance = background.computeLuminance();
  final lighter = foregroundLuminance > backgroundLuminance
      ? foregroundLuminance
      : backgroundLuminance;
  final darker = foregroundLuminance > backgroundLuminance
      ? backgroundLuminance
      : foregroundLuminance;
  return (lighter + 0.05) / (darker + 0.05);
}
