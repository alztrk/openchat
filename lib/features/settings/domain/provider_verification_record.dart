enum ProviderVerificationOutcome {
  verified,
  emptyCatalog,
  staleCatalog,
  authenticationFailed,
  rateLimited,
  networkFailed,
  failed,
}

final class ProviderVerificationRecord {
  const ProviderVerificationRecord({
    required this.outcome,
    required this.checkedAt,
  });

  final ProviderVerificationOutcome outcome;
  final DateTime checkedAt;

  Map<String, Object?> toJson() => <String, Object?>{
    'outcome': outcome.name,
    'checkedAtUnixMs': checkedAt.toUtc().millisecondsSinceEpoch,
  };

  factory ProviderVerificationRecord.fromJson(Object? value) {
    if (value is! Map) {
      throw const FormatException(
        'A provider verification record was invalid.',
      );
    }
    final outcomeValue = value['outcome'];
    final checkedAtUnixMs = value['checkedAtUnixMs'];
    final outcome = switch (outcomeValue) {
      'verified' => ProviderVerificationOutcome.verified,
      'emptyCatalog' => ProviderVerificationOutcome.emptyCatalog,
      'staleCatalog' => ProviderVerificationOutcome.staleCatalog,
      'authenticationFailed' =>
        ProviderVerificationOutcome.authenticationFailed,
      'rateLimited' => ProviderVerificationOutcome.rateLimited,
      'networkFailed' => ProviderVerificationOutcome.networkFailed,
      'failed' => ProviderVerificationOutcome.failed,
      _ => null,
    };
    if (outcome == null || checkedAtUnixMs is! int || checkedAtUnixMs < 0) {
      throw const FormatException(
        'A provider verification record was invalid.',
      );
    }
    return ProviderVerificationRecord(
      outcome: outcome,
      checkedAt: DateTime.fromMillisecondsSinceEpoch(
        checkedAtUnixMs,
        isUtc: true,
      ),
    );
  }
}
