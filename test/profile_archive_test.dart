import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/settings/data/profile_archive_repository.dart';
import 'package:openchat/features/settings/presentation/profile_archive_section.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

void main() {
  group('ProfileArchiveResult', () {
    test('parses an export response without requesting restart', () {
      final result = ProfileArchiveResult.fromJson(<String, Object?>{
        'conversationCount': 3,
        'messageCount': 18,
        'attachmentCount': 2,
        'attachmentBytes': 8192,
        'requiresRestart': false,
      });

      expect(result.conversationCount, 3);
      expect(result.messageCount, 18);
      expect(result.attachmentCount, 2);
      expect(result.attachmentBytes, 8192);
      expect(result.requiresRestart, isFalse);
    });

    test('parses a restore response that requires restart', () {
      final result = ProfileArchiveResult.fromJson(<String, Object?>{
        'conversationCount': 1,
        'messageCount': 4,
        'attachmentCount': 0,
        'attachmentBytes': 0,
        'requiresRestart': true,
      });

      expect(result.requiresRestart, isTrue);
    });

    test('rejects missing or invalid response fields', () {
      expect(
        () => ProfileArchiveResult.fromJson(<String, Object?>{
          'conversationCount': -1,
          'messageCount': 0,
          'attachmentCount': 0,
          'attachmentBytes': 0,
          'requiresRestart': false,
        }),
        throwsFormatException,
      );
      expect(
        () => ProfileArchiveResult.fromJson(<String, Object?>{
          'conversationCount': 0,
          'messageCount': 0,
          'attachmentCount': 0,
          'attachmentBytes': 0,
          'requiresRestart': 'false',
        }),
        throwsFormatException,
      );
    });
  });

  testWidgets(
    'shows unavailable state and disables actions without the service',
    (tester) async {
      await tester.pumpWidget(
        MaterialApp(
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          theme: ThemeData(extensions: const [OpenChatPalette.light]),
          home: const Scaffold(
            body: ProfileArchiveSection(serviceClient: null),
          ),
        ),
      );

      expect(find.text('Application data backup'), findsOneWidget);
      expect(
        find.textContaining('local profile backup service'),
        findsOneWidget,
      );
      final backupButton = tester.widget<OutlinedButton>(
        find.ancestor(
          of: find.text('Back up application data'),
          matching: find.byType(OutlinedButton),
        ),
      );
      final restoreButton = tester.widget<OutlinedButton>(
        find.ancestor(
          of: find.text('Restore application data'),
          matching: find.byType(OutlinedButton),
        ),
      );
      expect(backupButton.onPressed, isNull);
      expect(restoreButton.onPressed, isNull);
    },
  );
}
