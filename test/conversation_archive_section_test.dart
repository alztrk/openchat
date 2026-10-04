import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/settings/presentation/conversation_archive_section.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

void main() {
  testWidgets(
    'archive actions explain when local dependencies are unavailable',
    (tester) async {
      await tester.pumpWidget(
        MaterialApp(
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          theme: ThemeData(extensions: const [OpenChatPalette.light]),
          home: const Scaffold(
            body: ConversationArchiveSection(
              serviceClient: null,
              chatRepository: null,
            ),
          ),
        ),
      );

      expect(find.text('Conversation archives'), findsOneWidget);
      expect(find.textContaining('local archive service'), findsOneWidget);
      final exportButton = tester.widget<OutlinedButton>(
        find.ancestor(
          of: find.text('Export conversations'),
          matching: find.byType(OutlinedButton),
        ),
      );
      final importButton = tester.widget<OutlinedButton>(
        find.ancestor(
          of: find.text('Import archive'),
          matching: find.byType(OutlinedButton),
        ),
      );
      expect(exportButton.onPressed, isNull);
      expect(importButton.onPressed, isNull);
    },
  );
}
