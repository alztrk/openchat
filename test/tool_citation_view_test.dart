import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:intl/intl.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/presentation/widgets/tool_web_search_view.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

void main() {
  testWidgets('shows local source identity and original retrieval time', (
    tester,
  ) async {
    const locale = Locale('en');
    final l10n = await AppLocalizations.delegate.load(locale);
    final activity = ChatToolActivity(
      callId: 'call-1234',
      name: 'web_search',
      status: ChatToolActivityStatus.completed,
      arguments: const <String, Object?>{'query': 'OpenChat'},
      output: const <String, Object?>{
        'query': 'OpenChat',
        'sourceType': 'local_web_search',
        'retrievedAtUnixMs': 1800000000000,
        'results': [
          {
            'sourceId': 'S1-call1234',
            'title': 'OpenChat source',
            'url': 'https://example.org/openchat',
            'snippet': 'Project information',
            'engine': 'bing',
          },
        ],
      },
    );

    await tester.pumpWidget(
      MaterialApp(
        locale: locale,
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark,
        home: Scaffold(
          body: SingleChildScrollView(
            child: ToolWebSearchResult(
              activity: activity,
              palette: OpenChatPalette.dark,
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text(l10n.toolCitationSource('S1-call1234')), findsOneWidget);
    expect(find.text(l10n.toolLocalWebSource), findsOneWidget);
    final retrievedAt = DateTime.fromMillisecondsSinceEpoch(1800000000000)
        .toLocal();
    expect(
      find.text(
        l10n.toolSourceRetrievedAt(
          DateFormat.yMMMd('en').add_jm().format(retrievedAt),
        ),
      ),
      findsOneWidget,
    );
  });
}
