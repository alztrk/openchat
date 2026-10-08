class UsageStatisticsSummary {
  const UsageStatisticsSummary({
    required this.inputTokens,
    required this.outputTokens,
    required this.totalTokens,
    required this.reasoningTokens,
    required this.cachedInputTokens,
    required this.cacheWriteTokens,
    required this.providerReportedCostUsd,
    required this.costReportedRequests,
    required this.modelsDevCatalogCostUsd,
    required this.modelsDevCatalogCostRequests,
    required this.totalRequests,
    required this.successfulRequests,
    required this.failedRequests,
    required this.cancelledRequests,
    required this.interruptedRequests,
    required this.pendingRequests,
    required this.conversationCount,
    required this.fastRequestedRequests,
    required this.inputUsageRequests,
    required this.outputUsageRequests,
    required this.reasoningUsageRequests,
    required this.anyUsageRequests,
    required this.legacyOutputMessages,
  });

  final int inputTokens;
  final int outputTokens;
  final int totalTokens;
  final int reasoningTokens;
  final int cachedInputTokens;
  final int cacheWriteTokens;
  final double providerReportedCostUsd;
  final int costReportedRequests;
  final double modelsDevCatalogCostUsd;
  final int modelsDevCatalogCostRequests;
  final int totalRequests;
  final int successfulRequests;
  final int failedRequests;
  final int cancelledRequests;
  final int interruptedRequests;
  final int pendingRequests;
  final int conversationCount;
  final int fastRequestedRequests;
  final int inputUsageRequests;
  final int outputUsageRequests;
  final int reasoningUsageRequests;
  final int anyUsageRequests;
  final int legacyOutputMessages;

  factory UsageStatisticsSummary.fromJson(Map<String, Object?> json) {
    return UsageStatisticsSummary(
      inputTokens: _requiredInt(json, 'inputTokens'),
      outputTokens: _requiredInt(json, 'outputTokens'),
      totalTokens: _requiredInt(json, 'totalTokens'),
      reasoningTokens: _requiredInt(json, 'reasoningTokens'),
      cachedInputTokens: _requiredInt(json, 'cachedInputTokens'),
      cacheWriteTokens: _requiredInt(json, 'cacheWriteTokens'),
      providerReportedCostUsd: _requiredDouble(json, 'providerReportedCostUsd'),
      costReportedRequests: _requiredInt(json, 'costReportedRequests'),
      modelsDevCatalogCostUsd: _requiredDouble(json, 'modelsDevCatalogCostUsd'),
      modelsDevCatalogCostRequests: _requiredInt(
        json,
        'modelsDevCatalogCostRequests',
      ),
      totalRequests: _requiredInt(json, 'totalRequests'),
      successfulRequests: _requiredInt(json, 'successfulRequests'),
      failedRequests: _requiredInt(json, 'failedRequests'),
      cancelledRequests: _requiredInt(json, 'cancelledRequests'),
      interruptedRequests: _requiredInt(json, 'interruptedRequests'),
      pendingRequests: _requiredInt(json, 'pendingRequests'),
      conversationCount: _requiredInt(json, 'conversationCount'),
      fastRequestedRequests: _requiredInt(json, 'fastRequestedRequests'),
      inputUsageRequests: _requiredInt(json, 'inputUsageRequests'),
      outputUsageRequests: _requiredInt(json, 'outputUsageRequests'),
      reasoningUsageRequests: _requiredInt(json, 'reasoningUsageRequests'),
      anyUsageRequests: _requiredInt(json, 'anyUsageRequests'),
      legacyOutputMessages: _requiredInt(json, 'legacyOutputMessages'),
    );
  }
}

class UsageStatisticsBreakdown {
  const UsageStatisticsBreakdown({
    required this.key,
    required this.requests,
    required this.inputTokens,
    required this.outputTokens,
    required this.reasoningTokens,
    required this.cachedInputTokens,
    required this.cacheWriteTokens,
    required this.providerReportedCostUsd,
    required this.costReportedRequests,
    required this.modelsDevCatalogCostUsd,
    required this.modelsDevCatalogCostRequests,
  });

  final String key;
  final int requests;
  final int inputTokens;
  final int outputTokens;
  final int reasoningTokens;
  final int cachedInputTokens;
  final int cacheWriteTokens;
  final double providerReportedCostUsd;
  final int costReportedRequests;
  final double modelsDevCatalogCostUsd;
  final int modelsDevCatalogCostRequests;

  factory UsageStatisticsBreakdown.fromJson(
    Map<String, Object?> json,
    String keyName,
  ) {
    return UsageStatisticsBreakdown(
      key: _requiredString(json, keyName),
      requests: _requiredInt(json, 'requests'),
      inputTokens: _requiredInt(json, 'inputTokens'),
      outputTokens: _requiredInt(json, 'outputTokens'),
      reasoningTokens: _requiredInt(json, 'reasoningTokens'),
      cachedInputTokens: _requiredInt(json, 'cachedInputTokens'),
      cacheWriteTokens: _requiredInt(json, 'cacheWriteTokens'),
      providerReportedCostUsd: _requiredDouble(json, 'providerReportedCostUsd'),
      costReportedRequests: _requiredInt(json, 'costReportedRequests'),
      modelsDevCatalogCostUsd: _requiredDouble(json, 'modelsDevCatalogCostUsd'),
      modelsDevCatalogCostRequests: _requiredInt(
        json,
        'modelsDevCatalogCostRequests',
      ),
    );
  }
}

class UsageStatisticsFastMode {
  const UsageStatisticsFastMode({
    required this.fastRequested,
    required this.requests,
    required this.inputTokens,
    required this.outputTokens,
    required this.providerReportedCostUsd,
    required this.costReportedRequests,
    required this.modelsDevCatalogCostUsd,
    required this.modelsDevCatalogCostRequests,
  });

  final bool? fastRequested;
  final int requests;
  final int inputTokens;
  final int outputTokens;
  final double providerReportedCostUsd;
  final int costReportedRequests;
  final double modelsDevCatalogCostUsd;
  final int modelsDevCatalogCostRequests;

  factory UsageStatisticsFastMode.fromJson(Map<String, Object?> json) {
    final fastRequested = json['fastRequested'];
    if (fastRequested != null && fastRequested is! bool) {
      throw const FormatException('The Fast mode statistics were invalid.');
    }
    return UsageStatisticsFastMode(
      fastRequested: fastRequested as bool?,
      requests: _requiredInt(json, 'requests'),
      inputTokens: _requiredInt(json, 'inputTokens'),
      outputTokens: _requiredInt(json, 'outputTokens'),
      providerReportedCostUsd: _requiredDouble(json, 'providerReportedCostUsd'),
      costReportedRequests: _requiredInt(json, 'costReportedRequests'),
      modelsDevCatalogCostUsd: _requiredDouble(json, 'modelsDevCatalogCostUsd'),
      modelsDevCatalogCostRequests: _requiredInt(
        json,
        'modelsDevCatalogCostRequests',
      ),
    );
  }
}

class UsageStatisticsTrend {
  const UsageStatisticsTrend({
    required this.bucket,
    required this.inputTokens,
    required this.outputTokens,
    required this.modelsDevCatalogCostUsd,
    required this.modelsDevCatalogCostRequests,
    required this.requests,
  });

  final String bucket;
  final int inputTokens;
  final int outputTokens;
  final double modelsDevCatalogCostUsd;
  final int modelsDevCatalogCostRequests;
  final int requests;

  factory UsageStatisticsTrend.fromJson(
    Map<String, Object?> json,
    String keyName,
  ) {
    return UsageStatisticsTrend(
      bucket: _requiredString(json, keyName),
      inputTokens: _requiredInt(json, 'inputTokens'),
      outputTokens: _requiredInt(json, 'outputTokens'),
      modelsDevCatalogCostUsd: _requiredDouble(json, 'modelsDevCatalogCostUsd'),
      modelsDevCatalogCostRequests: _requiredInt(
        json,
        'modelsDevCatalogCostRequests',
      ),
      requests: _requiredInt(json, 'requests'),
    );
  }
}

class UsageStatisticsConversation {
  const UsageStatisticsConversation({
    required this.conversationId,
    required this.title,
    required this.requests,
    required this.inputTokens,
    required this.outputTokens,
    required this.modelsDevCatalogCostUsd,
    required this.modelsDevCatalogCostRequests,
    required this.lastRequestAtUnixMs,
  });

  final String conversationId;
  final String title;
  final int requests;
  final int inputTokens;
  final int outputTokens;
  final double modelsDevCatalogCostUsd;
  final int modelsDevCatalogCostRequests;
  final int lastRequestAtUnixMs;

  factory UsageStatisticsConversation.fromJson(Map<String, Object?> json) {
    return UsageStatisticsConversation(
      conversationId: _requiredString(json, 'conversationId'),
      title: _requiredString(json, 'title'),
      requests: _requiredInt(json, 'requests'),
      inputTokens: _requiredInt(json, 'inputTokens'),
      outputTokens: _requiredInt(json, 'outputTokens'),
      modelsDevCatalogCostUsd: _requiredDouble(json, 'modelsDevCatalogCostUsd'),
      modelsDevCatalogCostRequests: _requiredInt(
        json,
        'modelsDevCatalogCostRequests',
      ),
      lastRequestAtUnixMs: _requiredInt(json, 'lastRequestAtUnixMs'),
    );
  }
}

class UsageStatisticsRequest {
  const UsageStatisticsRequest({
    required this.eventId,
    required this.eventKind,
    required this.conversationId,
    required this.conversationTitle,
    required this.operation,
    required this.status,
    required this.usageSource,
    required this.startedAtUnixMs,
    this.assistantMessageId,
    this.providerId,
    this.modelId,
    this.reasoningEffort,
    this.fastRequested,
    this.serviceTier,
    this.inputTokens,
    this.outputTokens,
    this.reasoningTokens,
    this.cachedInputTokens,
    this.cacheWriteTokens,
    this.providerReportedCostUsd,
    this.modelsDevCatalogCostUsd,
    this.costSource,
    this.finishedAtUnixMs,
    this.requestManifest,
    this.runId,
  });

  final String eventId;
  final String eventKind;
  final String conversationId;
  final String conversationTitle;
  final String operation;
  final String status;
  final String usageSource;
  final int startedAtUnixMs;
  final String? assistantMessageId;
  final String? providerId;
  final String? modelId;
  final String? reasoningEffort;
  final bool? fastRequested;
  final String? serviceTier;
  final int? inputTokens;
  final int? outputTokens;
  final int? reasoningTokens;
  final int? cachedInputTokens;
  final int? cacheWriteTokens;
  final double? providerReportedCostUsd;
  final double? modelsDevCatalogCostUsd;
  final String? costSource;
  final int? finishedAtUnixMs;
  final UsageRequestManifest? requestManifest;
  final String? runId;

  factory UsageStatisticsRequest.fromJson(Map<String, Object?> json) {
    final fastRequested = json['fastRequested'];
    if (fastRequested != null && fastRequested is! bool) {
      throw const FormatException('The request Fast mode value was invalid.');
    }
    return UsageStatisticsRequest(
      eventId: _requiredString(json, 'eventId'),
      eventKind: _requiredString(json, 'eventKind'),
      conversationId: _requiredString(json, 'conversationId'),
      conversationTitle: _requiredString(json, 'conversationTitle'),
      operation: _requiredString(json, 'operation'),
      status: _requiredString(json, 'status'),
      usageSource: _requiredString(json, 'usageSource'),
      startedAtUnixMs: _requiredInt(json, 'startedAtUnixMs'),
      assistantMessageId: _optionalString(json, 'assistantMessageId'),
      providerId: _optionalString(json, 'providerId'),
      modelId: _optionalString(json, 'modelId'),
      reasoningEffort: _optionalString(json, 'reasoningEffort'),
      fastRequested: fastRequested as bool?,
      serviceTier: _optionalString(json, 'serviceTier'),
      inputTokens: _optionalInt(json, 'inputTokens'),
      outputTokens: _optionalInt(json, 'outputTokens'),
      reasoningTokens: _optionalInt(json, 'reasoningTokens'),
      cachedInputTokens: _optionalInt(json, 'cachedInputTokens'),
      cacheWriteTokens: _optionalInt(json, 'cacheWriteTokens'),
      providerReportedCostUsd: _optionalDouble(json, 'providerReportedCostUsd'),
      modelsDevCatalogCostUsd: _optionalDouble(json, 'modelsDevCatalogCostUsd'),
      costSource: _optionalString(json, 'costSource'),
      finishedAtUnixMs: _optionalInt(json, 'finishedAtUnixMs'),
      requestManifest: json['requestManifest'] == null
          ? null
          : UsageRequestManifest.fromJson(_objectMap(json['requestManifest'])),
      runId: _optionalString(json, 'runId'),
    );
  }
}

class UsageRequestManifest {
  const UsageRequestManifest({
    required this.available,
    required this.messageCount,
    required this.messageRoles,
    required this.imageCount,
    required this.toolResultCount,
    required this.instructionBytes,
    required this.instructionSources,
    required this.toolDefinitions,
    required this.cacheControls,
    required this.sourceMessageIds,
    required this.archivedMessageIds,
    required this.summarizedThroughMessageId,
    required this.sourceAttachments,
    required this.sourceDetailsTruncated,
  });

  final bool available;
  final int messageCount;
  final Map<String, int> messageRoles;
  final int imageCount;
  final int toolResultCount;
  final int instructionBytes;
  final List<String> instructionSources;
  final List<String> toolDefinitions;
  final List<String> cacheControls;
  final List<String> sourceMessageIds;
  final List<String> archivedMessageIds;
  final String? summarizedThroughMessageId;
  final List<UsageRequestAttachment> sourceAttachments;
  final bool sourceDetailsTruncated;

  factory UsageRequestManifest.fromJson(Map<String, Object?> json) {
    final available = json['available'];
    final roles = _objectMap(json['messageRoles']);
    if (available is! bool) {
      throw const FormatException('The request data manifest was invalid.');
    }
    final messageRoles = <String, int>{};
    for (final entry in roles.entries) {
      final count = entry.value;
      if (count is! int || count < 0) {
        throw const FormatException('The request data manifest was invalid.');
      }
      messageRoles[entry.key] = count;
    }
    int nonnegativeCount(String key) {
      final count = _requiredInt(json, key);
      if (count < 0) {
        throw const FormatException('The request data manifest was invalid.');
      }
      return count;
    }

    return UsageRequestManifest(
      available: available,
      messageCount: nonnegativeCount('messageCount'),
      messageRoles: messageRoles,
      imageCount: nonnegativeCount('imageCount'),
      toolResultCount: nonnegativeCount('toolResultCount'),
      instructionBytes: nonnegativeCount('instructionBytes'),
      instructionSources: _requiredStringList(json, 'instructionSources'),
      toolDefinitions: _requiredStringList(json, 'toolDefinitions'),
      cacheControls: _requiredStringList(json, 'cacheControls'),
      sourceMessageIds: _optionalStringList(json, 'sourceMessageIds'),
      archivedMessageIds: _optionalStringList(json, 'archivedMessageIds'),
      summarizedThroughMessageId: _optionalString(
        json,
        'summarizedThroughMessageId',
      ),
      sourceAttachments: _optionalObjectList(
        json,
        'sourceAttachments',
      ).map(UsageRequestAttachment.fromJson).toList(growable: false),
      sourceDetailsTruncated: _sourceDetailsTruncated(json['sourceLimits']),
    );
  }
}

class UsageRequestAttachment {
  const UsageRequestAttachment({
    required this.id,
    required this.messageId,
    required this.name,
    required this.kind,
    required this.mimeType,
  });

  final String id;
  final String messageId;
  final String name;
  final String kind;
  final String mimeType;

  factory UsageRequestAttachment.fromJson(Map<String, Object?> json) =>
      UsageRequestAttachment(
        id: _requiredString(json, 'id'),
        messageId: _requiredString(json, 'messageId'),
        name: _requiredString(json, 'name'),
        kind: _requiredString(json, 'kind'),
        mimeType: _requiredString(json, 'mimeType'),
      );
}

class UsageQuotaBucket {
  const UsageQuotaBucket({
    required this.limitId,
    this.usedPercent,
    this.windowSeconds,
    this.resetAtUnixMs,
  });

  final String limitId;
  final double? usedPercent;
  final int? windowSeconds;
  final int? resetAtUnixMs;

  factory UsageQuotaBucket.fromJson(Map<String, Object?> json) {
    return UsageQuotaBucket(
      limitId: _requiredString(json, 'limitId'),
      usedPercent: _optionalDouble(json, 'usedPercent'),
      windowSeconds: _optionalInt(json, 'windowSeconds'),
      resetAtUnixMs: _optionalInt(json, 'resetAtUnixMs'),
    );
  }
}

class UsageQuotaSnapshot {
  const UsageQuotaSnapshot({
    required this.connectionId,
    required this.workspaceId,
    this.connectionName,
    this.workspaceName,
    required this.fetchedAtUnixMs,
    required this.freshness,
    required this.resetCreditDetailsState,
    required this.buckets,
    this.ordinaryUsageAllowed,
    this.resetCreditCount,
  });

  final String connectionId;
  final String workspaceId;
  final String? connectionName;
  final String? workspaceName;
  final int fetchedAtUnixMs;
  final String freshness;
  final bool? ordinaryUsageAllowed;
  final int? resetCreditCount;
  final String resetCreditDetailsState;
  final List<UsageQuotaBucket> buckets;

  factory UsageQuotaSnapshot.fromJson(Map<String, Object?> json) {
    final ordinaryUsageAllowed = json['ordinaryUsageAllowed'];
    final buckets = json['buckets'];
    if ((ordinaryUsageAllowed != null && ordinaryUsageAllowed is! bool) ||
        buckets is! List<Object?>) {
      throw const FormatException('The quota history snapshot was invalid.');
    }
    return UsageQuotaSnapshot(
      connectionId: _requiredString(json, 'connectionId'),
      workspaceId: _requiredString(json, 'workspaceId'),
      connectionName: _optionalString(json, 'connectionName'),
      workspaceName: _optionalString(json, 'workspaceName'),
      fetchedAtUnixMs: _requiredInt(json, 'fetchedAtUnixMs'),
      freshness: _requiredString(json, 'freshness'),
      ordinaryUsageAllowed: ordinaryUsageAllowed as bool?,
      resetCreditCount: _optionalInt(json, 'resetCreditCount'),
      resetCreditDetailsState: _requiredString(json, 'resetCreditDetailsState'),
      buckets: buckets
          .map((bucket) => UsageQuotaBucket.fromJson(_objectMap(bucket)))
          .toList(growable: false),
    );
  }
}

class UsageStatisticsFilterOptions {
  const UsageStatisticsFilterOptions({
    required this.providers,
    required this.models,
    required this.reasoningEfforts,
  });

  final List<String> providers;
  final List<String> models;
  final List<String> reasoningEfforts;

  factory UsageStatisticsFilterOptions.fromJson(Map<String, Object?> json) {
    return UsageStatisticsFilterOptions(
      providers: _requiredStringList(json, 'providers'),
      models: _requiredStringList(json, 'models'),
      reasoningEfforts: _requiredStringList(json, 'reasoningEfforts'),
    );
  }
}

class UsageStatisticsPricingCatalog {
  const UsageStatisticsPricingCatalog({
    required this.source,
    required this.state,
    this.fetchedAtUnixMs,
  });

  final String source;
  final String state;
  final int? fetchedAtUnixMs;

  factory UsageStatisticsPricingCatalog.fromJson(Map<String, Object?> json) {
    return UsageStatisticsPricingCatalog(
      source: _requiredString(json, 'source'),
      state: _requiredString(json, 'state'),
      fetchedAtUnixMs: _optionalInt(json, 'fetchedAtUnixMs'),
    );
  }
}

class UsageStatistics {
  const UsageStatistics({
    required this.summary,
    required this.modelsDevPricing,
    required this.providers,
    required this.models,
    required this.reasoning,
    required this.operations,
    required this.fastMode,
    required this.serviceTiers,
    required this.dailyTrend,
    required this.monthlyTrend,
    required this.conversations,
    required this.requestDetailCount,
    required this.requests,
    required this.filterOptions,
    required this.quotaHistory,
  });

  final UsageStatisticsSummary summary;
  final UsageStatisticsPricingCatalog modelsDevPricing;
  final List<UsageStatisticsBreakdown> providers;
  final List<UsageStatisticsBreakdown> models;
  final List<UsageStatisticsBreakdown> reasoning;
  final List<UsageStatisticsBreakdown> operations;
  final List<UsageStatisticsFastMode> fastMode;
  final List<UsageStatisticsBreakdown> serviceTiers;
  final List<UsageStatisticsTrend> dailyTrend;
  final List<UsageStatisticsTrend> monthlyTrend;
  final List<UsageStatisticsConversation> conversations;
  final int requestDetailCount;
  final List<UsageStatisticsRequest> requests;
  final UsageStatisticsFilterOptions filterOptions;
  final List<UsageQuotaSnapshot> quotaHistory;

  factory UsageStatistics.fromJson(Map<String, Object?> json) {
    return UsageStatistics(
      summary: UsageStatisticsSummary.fromJson(_objectMap(json['summary'])),
      modelsDevPricing: UsageStatisticsPricingCatalog.fromJson(
        _objectMap(json['modelsDevPricing']),
      ),
      providers: _breakdownList(json, 'providerBreakdown', 'providerId'),
      models: _breakdownList(json, 'modelBreakdown', 'modelId'),
      reasoning: _breakdownList(json, 'reasoningBreakdown', 'reasoningEffort'),
      operations: _breakdownList(json, 'operationBreakdown', 'operation'),
      fastMode: _objectList(
        json,
        'fastModeBreakdown',
      ).map(UsageStatisticsFastMode.fromJson).toList(growable: false),
      serviceTiers: _breakdownList(json, 'serviceTierBreakdown', 'serviceTier'),
      dailyTrend: _trendList(json, 'dailyTrend', 'day'),
      monthlyTrend: _trendList(json, 'monthlyTrend', 'month'),
      conversations: _objectList(
        json,
        'conversationRanking',
      ).map(UsageStatisticsConversation.fromJson).toList(growable: false),
      requestDetailCount: _requiredInt(json, 'requestDetailCount'),
      requests: _objectList(
        json,
        'requestDetails',
      ).map(UsageStatisticsRequest.fromJson).toList(growable: false),
      filterOptions: UsageStatisticsFilterOptions.fromJson(
        _objectMap(json['filterOptions']),
      ),
      quotaHistory: _objectList(
        json,
        'quotaHistory',
      ).map(UsageQuotaSnapshot.fromJson).toList(growable: false),
    );
  }
}

List<UsageStatisticsBreakdown> _breakdownList(
  Map<String, Object?> json,
  String key,
  String valueName,
) => _objectList(json, key)
    .map((item) => UsageStatisticsBreakdown.fromJson(item, valueName))
    .toList(growable: false);

List<UsageStatisticsTrend> _trendList(
  Map<String, Object?> json,
  String key,
  String valueName,
) => _objectList(json, key)
    .map((item) => UsageStatisticsTrend.fromJson(item, valueName))
    .toList(growable: false);

List<Map<String, Object?>> _objectList(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value is! List<Object?>) {
    throw FormatException('The usage statistics list $key was invalid.');
  }
  return value.map(_objectMap).toList(growable: false);
}

Map<String, Object?> _objectMap(Object? value) {
  if (value is! Map) {
    throw const FormatException('A usage statistics object was invalid.');
  }
  final result = <String, Object?>{};
  for (final entry in value.entries) {
    if (entry.key is! String) {
      throw const FormatException('A usage statistics key was invalid.');
    }
    result[entry.key as String] = entry.value;
  }
  return result;
}

List<String> _requiredStringList(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value is! List<Object?> || value.any((item) => item is! String)) {
    throw FormatException('The usage statistics values $key were invalid.');
  }
  return value.cast<String>();
}

List<String> _optionalStringList(Map<String, Object?> json, String key) {
  if (!json.containsKey(key)) return const <String>[];
  return _requiredStringList(json, key);
}

List<Map<String, Object?>> _optionalObjectList(
  Map<String, Object?> json,
  String key,
) {
  if (!json.containsKey(key)) return const <Map<String, Object?>>[];
  final value = json[key];
  if (value is! List<Object?>) {
    throw FormatException('The usage statistics list $key was invalid.');
  }
  return value.map(_objectMap).toList(growable: false);
}

bool _sourceDetailsTruncated(Object? value) {
  if (value == null) return false;
  final limits = _objectMap(value);
  for (final key in <String>[
    'messageIdsTruncated',
    'archivedMessageIdsTruncated',
    'attachmentsTruncated',
  ]) {
    final truncated = limits[key];
    if (truncated is! bool) {
      throw const FormatException('The request source limits were invalid.');
    }
    if (truncated) return true;
  }
  return false;
}

String _requiredString(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value is! String || value.isEmpty) {
    throw FormatException('The usage statistics field $key was invalid.');
  }
  return value;
}

String? _optionalString(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value == null) return null;
  if (value is! String) {
    throw FormatException('The usage statistics field $key was invalid.');
  }
  return value;
}

int _requiredInt(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value is! int) {
    throw FormatException('The usage statistics field $key was invalid.');
  }
  return value;
}

int? _optionalInt(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value == null) return null;
  if (value is! int) {
    throw FormatException('The usage statistics field $key was invalid.');
  }
  return value;
}

double _requiredDouble(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value is! num) {
    throw FormatException('The usage statistics field $key was invalid.');
  }
  return value.toDouble();
}

double? _optionalDouble(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value == null) return null;
  if (value is! num) {
    throw FormatException('The usage statistics field $key was invalid.');
  }
  return value.toDouble();
}
