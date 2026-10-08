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

        final field = find.byType(TextField);
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
          for (final width in <double>[480, 1200]) {
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
                      widget.text.toPlainText().contains('P1'),
                );
                expect(citations, findsOneWidget);
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
                  builder: (context, child) => MediaQuery(
                    data: MediaQuery.of(context)
                        .copyWith(textScaler: TextScaler.linear(textScale)),
                    child: child ?? const SizedBox.shrink(),
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
