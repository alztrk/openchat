enum ProviderRequestOutcome {
  completed,
  waitingForInput,
  stopped,
  authenticationFailed,
  rateLimited,
  networkFailed,
  failed,
}

final class ProviderRequestOutcomeRecord {
  const ProviderRequestOutcomeRecord({
    required this.outcome,
    required this.occurredAt,
  });

  final ProviderRequestOutcome outcome;
  final DateTime occurredAt;

  Map<String, Object?> toJson() => <String, Object?>{
    'outcome': outcome.name,
    'occurredAtUnixMs': occurredAt.toUtc().millisecondsSinceEpoch,
  };

  factory ProviderRequestOutcomeRecord.fromJson(Object? value) {
    if (value is! Map) {
      throw const FormatException('A provider request outcome was invalid.');
    }
    final outcomeValue = value['outcome'];
    final occurredAtUnixMs = value['occurredAtUnixMs'];
    final outcome = switch (outcomeValue) {
      'completed' => ProviderRequestOutcome.completed,
      'waitingForInput' => ProviderRequestOutcome.waitingForInput,
      'stopped' => ProviderRequestOutcome.stopped,
      'authenticationFailed' => ProviderRequestOutcome.authenticationFailed,
      'rateLimited' => ProviderRequestOutcome.rateLimited,
      'networkFailed' => ProviderRequestOutcome.networkFailed,
      'failed' => ProviderRequestOutcome.failed,
      _ => null,
    };
    if (outcome == null || occurredAtUnixMs is! int || occurredAtUnixMs < 0) {
      throw const FormatException('A provider request outcome was invalid.');
    }
    return ProviderRequestOutcomeRecord(
      outcome: outcome,
      occurredAt: DateTime.fromMillisecondsSinceEpoch(
        occurredAtUnixMs,
        isUtc: true,
      ),
    );
  }
}
