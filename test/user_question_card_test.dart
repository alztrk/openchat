import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/agent_question.dart';
import 'package:openchat/features/chat/presentation/widgets/user_question_card.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

void main() {
  group('AgentQuestionGroup.fromJson', () {
    test('parses multiple choice and free-form questions', () {
      final group = AgentQuestionGroup.fromJson(_questionGroupJson());

      expect(group.questions, hasLength(2));
      expect(group.questions.first.kind, AgentQuestionKind.choice);
      expect(group.questions.last.kind, AgentQuestionKind.text);
      expect(group.questions.last.maxLength, 400);
    });

    test(
      'rejects malformed, oversized, and duplicate question definitions',
      () {
        final invalidChoice = _questionGroupJson();
        invalidChoice['questions'] = <Object?>[
          <String, Object?>{
            'id': 'choose',
            'kind': 'choice',
            'title': 'Select one',
            'required': true,
            'options': <Object?>[
              <String, Object?>{'id': 'only', 'label': 'Only option'},
            ],
          },
        ];
        expect(
          () => AgentQuestionGroup.fromJson(invalidChoice),
          throwsFormatException,
        );

        final oversized = _questionGroupJson();
        oversized['questions'] = List<Object?>.generate(
          33,
          (index) => <String, Object?>{
            'id': 'question-$index',
            'kind': 'text',
            'title': 'Question $index',
            'required': false,
            'options': <Object?>[],
          },
        );
        expect(
          () => AgentQuestionGroup.fromJson(oversized),
          throwsFormatException,
        );
      },
    );
  });

  testWidgets('shows required validation and submits a selected answer', (
    tester,
  ) async {
    const locale = Locale('en');
    final l10n = await AppLocalizations.delegate.load(locale);
    List<AgentQuestionAnswer>? submitted;

    await tester.pumpWidget(
      _testApp(
        locale,
        UserQuestionCard(
          group: AgentQuestionGroup.fromJson(_questionGroupJson()),
          onSubmit: (answers) async {
            submitted = answers;
            return null;
          },
        ),
      ),
    );

    await tester.tap(find.text(l10n.userQuestionSubmit));
    await tester.pumpAndSettle();
    expect(find.text(l10n.userQuestionRequiredValidation), findsOneWidget);
    expect(submitted, isNull);

    await tester.tap(find.text('First option'));
    await tester.enterText(find.byType(TextField), 'A short explanation.');
    await tester.tap(find.text(l10n.userQuestionSubmit));
    await tester.pumpAndSettle();

    final capturedAnswers = submitted;
    if (capturedAnswers == null) {
      fail('The selected answers were not submitted.');
    }
    expect(capturedAnswers, hasLength(2));
    expect(
      capturedAnswers[0].value,
      isA<AgentChoiceAnswer>().having(
        (answer) => answer.optionId,
        'optionId',
        'first',
      ),
    );
    expect(
      capturedAnswers[1].value,
      isA<AgentTextAnswer>().having(
        (answer) => answer.text,
        'text',
        'A short explanation.',
      ),
    );
    expect(find.text(l10n.userQuestionRequiredValidation), findsNothing);
  });

  testWidgets('focuses the first answer control for a notification target', (
    tester,
  ) async {
    const locale = Locale('en');
    await tester.pumpWidget(
      _testApp(
        locale,
        UserQuestionCard(
          group: AgentQuestionGroup.fromJson(_questionGroupJson()),
          focusOnBuild: true,
          onSubmit: (_) async => null,
        ),
      ),
    );

    final radioTiles = tester.widgetList<RadioListTile<String>>(
      find.byType(RadioListTile<String>),
    );
    expect(radioTiles.first.autofocus, isTrue);
    expect(radioTiles.skip(1).every((tile) => !tile.autofocus), isTrue);
    expect(tester.widget<TextField>(find.byType(TextField)).autofocus, isFalse);
  });

  testWidgets('focuses continue after a saved answer is restored', (
    tester,
  ) async {
    const locale = Locale('en');
    final questionGroup = _questionGroupJson()
      ..['status'] = 'answered'
      ..['answers'] = <Object?>[
        <String, Object?>{
          'questionId': 'choice-1',
          'value': <String, Object?>{'kind': 'choice', 'optionId': 'first'},
        },
        <String, Object?>{
          'questionId': 'text-1',
          'value': <String, Object?>{
            'kind': 'text',
            'text': 'Saved explanation.',
          },
        },
      ];

    await tester.pumpWidget(
      _testApp(
        locale,
        UserQuestionCard(
          group: AgentQuestionGroup.fromJson(questionGroup),
          focusOnBuild: true,
          onSubmit: (_) async => null,
        ),
      ),
    );

    final continueButton = tester.widget<FilledButton>(
      find.byType(FilledButton),
    );
    expect(continueButton.autofocus, isTrue);
    expect(continueButton.onPressed, isNotNull);
    expect(
      tester
          .widget<RadioListTile<String>>(
            find.byType(RadioListTile<String>).first,
          )
          .enabled,
      isFalse,
    );
  });

  testWidgets('keeps the card usable when submitting returns an error', (
    tester,
  ) async {
    const locale = Locale('tr');
    final l10n = await AppLocalizations.delegate.load(locale);
    var submitCount = 0;
    await tester.pumpWidget(
      _testApp(
        locale,
        UserQuestionCard(
          group: AgentQuestionGroup.fromJson(_questionGroupJson()),
          onSubmit: (_) async {
            submitCount++;
            return l10n.userQuestionSubmitFailed;
          },
        ),
      ),
    );

    await tester.tap(find.text('First option'));
    await tester.enterText(find.byType(TextField), 'A short explanation.');
    await tester.tap(find.text(l10n.userQuestionSubmit));
    await tester.pumpAndSettle();

    expect(find.text(l10n.userQuestionSubmitFailed), findsOneWidget);
    final button = tester.widget<FilledButton>(
      find.ancestor(
        of: find.text(l10n.userQuestionSubmit),
        matching: find.byType(FilledButton),
      ),
    );
    expect(button.onPressed, isNotNull);
    expect(submitCount, 1);
  });
}

Widget _testApp(Locale locale, Widget child) => MaterialApp(
  locale: locale,
  localizationsDelegates: AppLocalizations.localizationsDelegates,
  supportedLocales: AppLocalizations.supportedLocales,
  theme: OpenChatTheme.light,
  home: Scaffold(body: SingleChildScrollView(child: child)),
);

Map<String, Object?> _questionGroupJson() => <String, Object?>{
  'groupId': 'group-1',
  'runId': 'run-1',
  'conversationId': 'conversation-1',
  'revision': 1,
  'status': 'pending',
  'answers': <Object?>[],
  'assistantMessageId': 'assistant-1',
  'toolCallId': 'call-1',
  'toolName': 'ask_user',
  'toolArguments': <String, Object?>{},
  'questions': <Object?>[
    <String, Object?>{
      'id': 'choice-1',
      'kind': 'choice',
      'title': 'Choose an option',
      'description': 'Pick one answer.',
      'options': <Object?>[
        <String, Object?>{'id': 'first', 'label': 'First option'},
        <String, Object?>{'id': 'second', 'label': 'Second option'},
      ],
      'required': true,
    },
    <String, Object?>{
      'id': 'text-1',
      'kind': 'text',
      'title': 'Add details',
      'options': <Object?>[],
      'required': true,
      'placeholder': 'Write a short explanation',
      'maxLength': 400,
    },
  ],
};
