import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/presentation/widgets/assistant_message.dart';
import 'package:openchat/features/chat/presentation/widgets/conversation_pane.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

import 'support/shad_test_scope.dart';

void main() {
  testWidgets('retries stay selectable as response versions', (tester) async {
    final controller = TextEditingController();
    addTearDown(controller.dispose);
    final l10n = await AppLocalizations.delegate.load(const Locale('tr'));

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('tr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark,
        builder: openChatShadTestBuilder,
        home: Scaffold(
          body: SizedBox(
            width: 1000,
            height: 800,
            child: ConversationPane(
              messageController: controller,
              showHistoryButton: false,
              onOpenHistory: () {},
              onSendMessage: () {},
              providerId: 'opencode',
              messages: const <ChatMessage>[
                ChatMessage(
                  id: 'user-1',
                  role: ChatMessageRole.user,
                  content: 'Soruyu yanıtla',
                ),
                ChatMessage(
                  id: 'assistant-version-1',
                  role: ChatMessageRole.assistant,
                  content: 'İlk yanıt',
                  providerId: 'gemini',
                  modelId: 'gemini-model-1',
                ),
                ChatMessage(
                  id: 'assistant-version-2',
                  role: ChatMessageRole.assistant,
                  content: 'İkinci yanıt',
                  providerId: 'opencode',
                  modelId: 'opencode-model-2',
                ),
              ],
              assistantModelLabel: 'current model',
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(_assistantText('İkinci yanıt'), findsOneWidget);
    expect(_assistantText('İlk yanıt'), findsNothing);
    expect(find.text('opencode-model-2'), findsOneWidget);
    expect(find.text('gemini-model-1'), findsNothing);
    expect(find.text(l10n.responseVersionCount(2, 2)), findsOneWidget);

    await tester.tap(find.byTooltip(l10n.previousResponseVersion));
    await tester.pumpAndSettle();

    expect(_assistantText('İlk yanıt'), findsOneWidget);
    expect(_assistantText('İkinci yanıt'), findsNothing);
    expect(find.text('gemini-model-1'), findsOneWidget);
    expect(find.text('opencode-model-2'), findsNothing);
    expect(find.text(l10n.responseVersionCount(1, 2)), findsOneWidget);

    await tester.tap(find.byTooltip(l10n.nextResponseVersion));
    await tester.pumpAndSettle();

    expect(_assistantText('İkinci yanıt'), findsOneWidget);
    expect(find.text(l10n.responseVersionCount(2, 2)), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('provider citations open their stored source record', (
    tester,
  ) async {
    const locale = Locale('en');
    final l10n = await AppLocalizations.delegate.load(locale);

    await tester.pumpWidget(
      MaterialApp(
        locale: locale,
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark,
        builder: openChatShadTestBuilder,
        home: Scaffold(
          body: SizedBox(
            width: 900,
            height: 700,
            child: const AssistantMessage(
              message: ChatMessage(
                id: 'assistant-citation',
                role: ChatMessageRole.assistant,
                content: 'A sourced answer [P1].',
                providerId: 'chatgpt',
                modelId: 'model-1',
                citationSources: <ChatCitationSource>[
                  ChatCitationSource(
                    id: 'P1',
                    title: 'Provider article',
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
    );
    await tester.pumpAndSettle();

    final citation = find.byWidgetPredicate(
      (widget) =>
          widget is RichText &&
          widget.text.toPlainText().contains('A sourced answer P1.'),
    );
    expect(citation, findsOneWidget);
    final richText = tester.widget<RichText>(citation);
    final text = richText.text.toPlainText();
    final citationStart = text.indexOf('P1');
    final paragraph = tester.renderObject<RenderParagraph>(citation);
    final caretOffset = paragraph.getOffsetForCaret(
      TextPosition(offset: citationStart),
      Rect.zero,
    );
    final bounds = tester.getRect(citation);
    await tester.tapAt(
      Offset(bounds.left + caretOffset.dx + 2, bounds.top + bounds.height / 2),
    );
    await tester.pumpAndSettle();

    expect(find.text('Provider article'), findsOneWidget);
    expect(find.text(l10n.toolProviderSource), findsOneWidget);
    expect(find.text('https://example.org/article'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('provider-native web-search results back inline citations', (
    tester,
  ) async {
    const locale = Locale('en');
    final l10n = await AppLocalizations.delegate.load(locale);

    await tester.pumpWidget(
      MaterialApp(
        locale: locale,
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark,
        builder: openChatShadTestBuilder,
        home: Scaffold(
          body: SizedBox(
            width: 900,
            height: 700,
            child: const AssistantMessage(
              message: ChatMessage(
                id: 'assistant-mistral-search-citation',
                role: ChatMessageRole.assistant,
                content: 'A searched answer [S1-call1234].',
                providerId: 'mistral',
                modelId: 'mistral-medium-latest',
                toolActivities: <ChatToolActivity>[
                  ChatToolActivity(
                    callId: 'call1234',
                    name: 'web_search',
                    arguments: <String, Object?>{'query': 'OpenChat'},
                    status: ChatToolActivityStatus.completed,
                    output: <String, Object?>{
                      'sourceType': 'provider_native',
                      'results': <Object?>[
                        <String, Object?>{
                          'sourceId': 'S1-call1234',
                          'title': 'OpenChat source',
                          'url': 'https://example.org/openchat',
                          'snippet': 'Search result summary',
                        },
                      ],
                    },
                  ),
                ],
              ),
              modelLabel: 'mistral-medium-latest',
              providerId: 'mistral',
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    final citation = find.byWidgetPredicate(
      (widget) =>
          widget is RichText &&
          widget.text.toPlainText().contains('A searched answer S1-call1234.'),
    );
    expect(citation, findsOneWidget);
    final richText = tester.widget<RichText>(citation);
    final citationStart = richText.text.toPlainText().indexOf('S1-call1234');
    final paragraph = tester.renderObject<RenderParagraph>(citation);
    final caretOffset = paragraph.getOffsetForCaret(
      TextPosition(offset: citationStart),
      Rect.zero,
    );
    final bounds = tester.getRect(citation);
    await tester.tapAt(
      Offset(bounds.left + caretOffset.dx + 2, bounds.top + bounds.height / 2),
    );
    await tester.pumpAndSettle();

    expect(find.text('OpenChat source'), findsOneWidget);
    expect(find.text(l10n.toolProviderSource), findsOneWidget);
    expect(find.text('https://example.org/openchat'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
}

Finder _assistantText(String value) => find.byWidgetPredicate(
  (widget) => widget is RichText && widget.text.toPlainText().contains(value),
);
