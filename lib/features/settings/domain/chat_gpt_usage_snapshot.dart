class ChatGptUsageBucket {
  const ChatGptUsageBucket({
    required this.limitId,
    this.usedPercent,
    this.windowSeconds,
    this.resetAtUnixMs,
  });

  final String limitId;
  final double? usedPercent;
  final int? windowSeconds;
  final int? resetAtUnixMs;

  factory ChatGptUsageBucket.fromJson(Map<String, Object?> json) {
    final usedPercent = json['usedPercent'];
    final windowSeconds = json['windowSeconds'];
    final resetAtUnixMs = json['resetAtUnixMs'];
    if ((usedPercent != null && usedPercent is! num) ||
        (windowSeconds != null && windowSeconds is! int) ||
        (resetAtUnixMs != null && resetAtUnixMs is! int)) {
      throw const FormatException('A ChatGPT usage bucket was invalid.');
    }
    return ChatGptUsageBucket(
      limitId: _requiredString(json, 'limitId'),
      usedPercent: (usedPercent as num?)?.toDouble(),
      windowSeconds: windowSeconds as int?,
      resetAtUnixMs: resetAtUnixMs as int?,
    );
  }
}

class ChatGptResetCredit {
  const ChatGptResetCredit({
    required this.id,
    this.resetType,
    this.status,
    this.grantedAtUnixMs,
    this.expiresAtUnixMs,
    this.title,
    this.description,
  });

  final String id;
  final String? resetType;
  final String? status;
  final int? grantedAtUnixMs;
  final int? expiresAtUnixMs;
  final String? title;
  final String? description;

  factory ChatGptResetCredit.fromJson(Map<String, Object?> json) {
    final grantedAtUnixMs = json['grantedAtUnixMs'];
    final expiresAtUnixMs = json['expiresAtUnixMs'];
    if ((grantedAtUnixMs != null && grantedAtUnixMs is! int) ||
        (expiresAtUnixMs != null && expiresAtUnixMs is! int)) {
      throw const FormatException('A ChatGPT reset credit was invalid.');
    }
    return ChatGptResetCredit(
      id: _requiredString(json, 'id'),
      resetType: _optionalString(json, 'resetType'),
      status: _optionalString(json, 'status'),
      grantedAtUnixMs: grantedAtUnixMs as int?,
      expiresAtUnixMs: expiresAtUnixMs as int?,
      title: _optionalString(json, 'title'),
      description: _optionalString(json, 'description'),
    );
  }
}

class ChatGptUsageSnapshot {
  const ChatGptUsageSnapshot({
    required this.fetchedAtUnixMs,
    required this.freshness,
    required this.resetCreditDetailsState,
    required this.buckets,
    required this.resetCredits,
    this.ordinaryUsageAllowed,
    this.resetCreditCount,
  });

  final int fetchedAtUnixMs;
  final String freshness;
  final bool? ordinaryUsageAllowed;
  final int? resetCreditCount;
  final String resetCreditDetailsState;
  final List<ChatGptUsageBucket> buckets;
  final List<ChatGptResetCredit> resetCredits;

  factory ChatGptUsageSnapshot.fromJson(Map<String, Object?> json) {
    final fetchedAt = json['fetchedAtUnixMs'];
    final allowed = json['ordinaryUsageAllowed'];
    final resetCount = json['resetCreditCount'];
    final buckets = json['buckets'];
    final credits = json['resetCredits'];
    if (fetchedAt is! int ||
        (allowed != null && allowed is! bool) ||
        (resetCount != null && resetCount is! int) ||
        buckets is! List<Object?> ||
        credits is! List<Object?>) {
      throw const FormatException('The ChatGPT usage snapshot was invalid.');
    }
    return ChatGptUsageSnapshot(
      fetchedAtUnixMs: fetchedAt,
      freshness: _requiredString(json, 'freshness'),
      ordinaryUsageAllowed: allowed as bool?,
      resetCreditCount: resetCount as int?,
      resetCreditDetailsState: _requiredString(json, 'resetCreditDetailsState'),
      buckets: buckets
          .map((bucket) => ChatGptUsageBucket.fromJson(_objectMap(bucket)))
          .toList(growable: false),
      resetCredits: credits
          .map((credit) => ChatGptResetCredit.fromJson(_objectMap(credit)))
          .toList(growable: false),
    );
  }
}

Map<String, Object?> _objectMap(Object? value) {
  if (value is! Map) {
    throw const FormatException('A ChatGPT response item was invalid.');
  }
  final result = <String, Object?>{};
  for (final entry in value.entries) {
    if (entry.key is! String) {
      throw const FormatException('A ChatGPT response key was invalid.');
    }
    result[entry.key as String] = entry.value;
  }
  return result;
}

String _requiredString(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value is! String || value.isEmpty) {
    throw FormatException('The ChatGPT response did not contain $key.');
  }
  return value;
}

String? _optionalString(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value == null) return null;
  if (value is! String) {
    throw FormatException('The ChatGPT response field $key was invalid.');
  }
  return value;
}
