class AgentGoal {
  const AgentGoal({
    required this.conversationId,
    required this.runId,
    required this.objective,
    required this.progress,
    required this.startedAt,
    required this.status,
    this.todos = const <AgentGoalTodo>[],
    this.pauseReason,
  });

  final String conversationId;
  final String runId;
  final String objective;
  final String progress;
  final DateTime startedAt;
  final AgentGoalStatus status;
  final List<AgentGoalTodo> todos;
  final String? pauseReason;

  AgentGoal copyWith({
    String? progress,
    AgentGoalStatus? status,
    List<AgentGoalTodo>? todos,
    String? pauseReason,
    bool clearPauseReason = false,
  }) {
    return AgentGoal(
      conversationId: conversationId,
      runId: runId,
      objective: objective,
      progress: progress ?? this.progress,
      startedAt: startedAt,
      status: status ?? this.status,
      todos: todos ?? this.todos,
      pauseReason: clearPauseReason ? null : pauseReason ?? this.pauseReason,
    );
  }

  static AgentGoal? fromJson(Object? value) {
    if (value == null) return null;
    if (value is! Map<String, Object?>) {
      throw const FormatException('The active goal response was invalid.');
    }
    final conversationId = value['conversationId'];
    final runId = value['runId'];
    final objective = value['objective'];
    final progress = value['progress'];
    final startedAtUnixMs = value['startedAtUnixMs'];
    final statusValue = value['status'];
    final pauseReason = value['pauseReason'];
    final rawTodos = value['todos'];
    final todos = rawTodos == null
        ? const <AgentGoalTodo>[]
        : rawTodos is List<Object?>
        ? rawTodos.map(AgentGoalTodo.fromObject).toList(growable: false)
        : throw const FormatException('The active goal response was invalid.');
    if (conversationId is! String ||
        conversationId.isEmpty ||
        runId is! String ||
        runId.isEmpty ||
        objective is! String ||
        objective.trim().isEmpty ||
        progress is! String ||
        startedAtUnixMs is! int ||
        startedAtUnixMs < 0 ||
        (pauseReason != null && pauseReason is! String)) {
      throw const FormatException('The active goal response was invalid.');
    }
    final status = switch (statusValue) {
      'running' => AgentGoalStatus.running,
      'paused' => AgentGoalStatus.paused,
      'interrupted' => AgentGoalStatus.interrupted,
      'completed' => AgentGoalStatus.completed,
      'cancelled' => AgentGoalStatus.cancelled,
      'failed' => AgentGoalStatus.failed,
      _ => throw const FormatException('The active goal status was invalid.'),
    };
    return AgentGoal(
      conversationId: conversationId,
      runId: runId,
      objective: objective,
      progress: progress,
      startedAt: DateTime.fromMillisecondsSinceEpoch(
        startedAtUnixMs,
        isUtc: true,
      ),
      status: status,
      todos: todos,
      pauseReason: pauseReason as String?,
    );
  }
}

class AgentGoalTodo {
  const AgentGoalTodo({required this.text, required this.completed});

  final String text;
  final bool completed;

  factory AgentGoalTodo.fromObject(Object? value) {
    if (value is! Map<String, Object?>) {
      throw const FormatException('The active goal task was invalid.');
    }
    final text = value['text'];
    final completed = value['completed'];
    if (text is! String || text.trim().isEmpty || completed is! bool) {
      throw const FormatException('The active goal task was invalid.');
    }
    return AgentGoalTodo(text: text, completed: completed);
  }
}

enum AgentGoalStatus {
  running,
  paused,
  interrupted,
  completed,
  cancelled,
  failed,
}
