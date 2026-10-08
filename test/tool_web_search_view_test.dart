import 'package:flutter/material.dart';
import 'package:flutter_widget_from_html_core/flutter_widget_from_html_core.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/domain/tool_permission_request.dart';
import 'package:openchat/features/chat/presentation/widgets/tool_activity.dart';
import 'package:openchat/features/chat/presentation/widgets/tool_permission_card.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

void main() {
  Widget buildTestableWidget({
    required Widget child,
    Locale locale = const Locale('tr'),
  }) {
    return MaterialApp(
      locale: locale,
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      theme: OpenChatTheme.light,
      home: Scaffold(body: SingleChildScrollView(child: child)),
    );
  }

  group('ToolWebSearchResult', () {
    testWidgets('renders search query, results, domain, and snippet', (
      tester,
    ) async {
      const locale = Locale('tr');
      final l10n = await AppLocalizations.delegate.load(locale);

      final activity = ChatToolActivity(
        callId: 'call_search_1',
        name: 'web_search',
        status: ChatToolActivityStatus.completed,
        arguments: const <String, Object?>{
          'query': 'rust tokio async',
          'limit': 2,
        },
        output: const <String, Object?>{
          'query': 'rust tokio async',
          'total_results': 2,
          'results': [
            {
              'title': 'Tokio - An asynchronous runtime for Rust',
              'url': 'https://tokio.rs',
              'snippet': 'Tokio is an event-driven, non-blocking I/O platform.',
              'engine': 'google',
            },
            {
              'title': 'Rust Async Programming Book',
              'url': 'https://rust-lang.github.io/async-book',
              'snippet': 'Asynchronous programming in Rust with async/await.',
              'engine': 'duckduckgo',
            },
          ],
        },
      );

      await tester.pumpWidget(
        buildTestableWidget(
          locale: locale,
          child: ToolActivityAccordion(
            activity: activity,
            palette: OpenChatPalette.light,
          ),
        ),
      );
      await tester.pumpAndSettle();

      // Expand accordion
      await tester.tap(find.text(l10n.toolWebSearch));
      await tester.pumpAndSettle();

      expect(
        find.text('Tokio - An asynchronous runtime for Rust'),
        findsAtLeastNWidgets(1),
      );
      expect(find.text('GOOGLE'), findsAtLeastNWidgets(1));
      expect(find.text('DUCKDUCKGO'), findsAtLeastNWidgets(1));
      expect(find.text('tokio.rs'), findsAtLeastNWidgets(1));
      expect(
        find.text('Tokio is an event-driven, non-blocking I/O platform.'),
        findsAtLeastNWidgets(1),
      );
    });

    testWidgets('shows empty notice when no results returned', (tester) async {
      const locale = Locale('tr');
      final l10n = await AppLocalizations.delegate.load(locale);

      final activity = ChatToolActivity(
        callId: 'call_search_empty',
        name: 'web_search',
        status: ChatToolActivityStatus.completed,
        arguments: const <String, Object?>{'query': 'nonexistenttermxyz123'},
        output: const <String, Object?>{
          'query': 'nonexistenttermxyz123',
          'total_results': 0,
          'results': [],
        },
      );

      await tester.pumpWidget(
        buildTestableWidget(
          locale: locale,
          child: ToolActivityAccordion(
            activity: activity,
            palette: OpenChatPalette.light,
          ),
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.text(l10n.toolWebSearch));
      await tester.pumpAndSettle();

      expect(find.text(l10n.toolWebSearchNoResults), findsAtLeastNWidgets(1));
    });

    testWidgets('renders sanitized provider search attribution', (
      tester,
    ) async {
      final l10n = await AppLocalizations.delegate.load(const Locale('tr'));
      final activity = ChatToolActivity(
        callId: 'call_search_grounded',
        name: 'web_search',
        status: ChatToolActivityStatus.completed,
        arguments: const <String, Object?>{'query': 'query'},
        output: const <String, Object?>{
          'sourceType': 'provider_native',
          'attributionHtml': '<div>Google Search <script>bad()</script></div>',
          'results': [
            {
              'title': 'Example',
              'url': 'https://example.org',
              'snippet': '',
              'engine': 'google',
            },
          ],
        },
      );
      await tester.pumpWidget(
        buildTestableWidget(
          child: ToolActivityAccordion(
            activity: activity,
            palette: OpenChatPalette.light,
          ),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text(l10n.toolWebSearch));
      await tester.pumpAndSettle();

      final attribution = tester.widget<HtmlWidget>(find.byType(HtmlWidget));
      expect(attribution.html, contains('Google Search'));
      expect(attribution.html, isNot(contains('<script')));
    });
  });

  group('ToolReadUrlResult', () {
    testWidgets(
      'renders webpage title, length, markdown content, and truncated badge',
      (tester) async {
        const locale = Locale('tr');
        final l10n = await AppLocalizations.delegate.load(locale);

        final activity = ChatToolActivity(
          callId: 'call_read_url_1',
          name: 'read_url_content',
          status: ChatToolActivityStatus.completed,
          arguments: const <String, Object?>{'url': 'https://docs.rs/tokio'},
          output: const <String, Object?>{
            'url': 'https://docs.rs/tokio',
            'title': 'Tokio Documentation',
            'content': '# Tokio Core\nRuntime for building fast applications.',
            'length': 48,
            'truncated': true,
          },
        );

        await tester.pumpWidget(
          buildTestableWidget(
            locale: locale,
            child: ToolActivityAccordion(
              activity: activity,
              palette: OpenChatPalette.light,
            ),
          ),
        );
        await tester.pumpAndSettle();

        await tester.tap(find.text(l10n.toolReadUrlContent));
        await tester.pumpAndSettle();

        expect(find.text('Tokio Documentation'), findsAtLeastNWidgets(1));
        expect(find.text(l10n.toolReadUrlLength(48)), findsAtLeastNWidgets(1));
        expect(find.text(l10n.toolOperationTruncated), findsAtLeastNWidgets(1));
        expect(
          find.text('# Tokio Core\nRuntime for building fast applications.'),
          findsAtLeastNWidgets(1),
        );
      },
    );
  });

  group('ToolPermissionCard with Web Tools', () {
    testWidgets('shows approval prompt for web_search', (tester) async {
      const locale = Locale('tr');
      final l10n = await AppLocalizations.delegate.load(locale);

      const request = ToolPermissionRequest(
        id: 'req_ws_1',
        toolName: 'web_search',
        targetPath: 'flutter vs react',
        arguments: <String, Object?>{'query': 'flutter vs react', 'limit': 5},
      );

      await tester.pumpWidget(
        buildTestableWidget(
          locale: locale,
          child: const ToolPermissionCard(
            request: request,
            isResponding: false,
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text(l10n.toolWebSearch), findsOneWidget);
      expect(find.text(l10n.toolSearchQuery), findsOneWidget);
      expect(find.text('flutter vs react'), findsAtLeast(1));
    });

    testWidgets('shows approval prompt for read_url_content', (tester) async {
      const locale = Locale('tr');
      final l10n = await AppLocalizations.delegate.load(locale);

      const request = ToolPermissionRequest(
        id: 'req_ru_1',
        toolName: 'read_url_content',
        targetPath: 'https://github.com/rust-lang/rust',
        arguments: <String, Object?>{
          'url': 'https://github.com/rust-lang/rust',
          'max_chars': 10000,
        },
      );

      await tester.pumpWidget(
        buildTestableWidget(
          locale: locale,
          child: const ToolPermissionCard(
            request: request,
            isResponding: false,
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text(l10n.toolReadUrlContent), findsOneWidget);
      expect(find.text(l10n.toolUrl), findsOneWidget);
      expect(find.text('https://github.com/rust-lang/rust'), findsAtLeast(1));
    });
  });
}
