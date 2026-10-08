import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/features/chat/domain/agent_goal.dart';

void main() {
  final base = <String, Object?>{
    'conversationId': 'conversation-1',
    'runId': 'run-1',
    'objective': 'Finish the task',
    'progress': 'Reviewed the changes',
    'startedAtUnixMs': 1,
    'status': 'running',
    'pauseReason': null,
  };

  test('older goal responses without tasks remain valid', () {
    final goal = AgentGoal.fromJson(base);

    expect(goal, isNotNull);
    expect(goal!.todos, isEmpty);
  });

  test('parses persisted task state', () {
    final goal = AgentGoal.fromJson({
      ...base,
      'todos': [
        {'text': 'Check behavior', 'completed': true},
        {'text': 'Update docs', 'completed': false},
      ],
    });

    expect(goal!.todos, hasLength(2));
    expect(goal.todos.first.completed, isTrue);
    expect(goal.todos.last.text, 'Update docs');
  });

  test('rejects malformed task state', () {
    expect(
      () => AgentGoal.fromJson({
        ...base,
        'todos': [
          {'text': ' ', 'completed': false},
        ],
      }),
      throwsFormatException,
    );
  });
}
