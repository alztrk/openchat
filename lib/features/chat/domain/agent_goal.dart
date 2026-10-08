class AgentGoal {
  const AgentGoal({
    required this.conversationId,
    required this.runId,
    required this.objective,
    required this.progress,
    required this.startedAt,
    required this.status,
    this.pauseReason,
  });

  final String conversationId;
  final String runId;
  final String objective;
  final String progress;
  final DateTime startedAt;
  final AgentGoalStatus status;
  final String? pauseReason;

  AgentGoal copyWith({
    String? progress,
    AgentGoalStatus? status,
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
      pauseReason: pauseReason as String?,
    );
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
