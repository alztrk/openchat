import 'dart:async';
import 'dart:convert';
import 'dart:math' as math;

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:intl/intl.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/openchat_toast.dart';
import 'package:openchat/app/openchat_select.dart';
import 'package:openchat/features/settings/domain/usage_statistics.dart';
import 'package:openchat/features/settings/presentation/chat_gpt_usage_bucket_label.dart';
import 'package:openchat/features/settings/domain/chat_gpt_usage_snapshot.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

enum _StatisticsLoadState { loading, loaded, failed, unavailable }

class StatisticsSettingsSection extends StatefulWidget {
  const StatisticsSettingsSection({
    required this.serviceClient,
    this.onOpenConversation,
    super.key,
  });

  final OpenChatServiceClient? serviceClient;
  final ValueChanged<String>? onOpenConversation;

  @override
  State<StatisticsSettingsSection> createState() =>
      _StatisticsSettingsSectionState();
}

class _StatisticsSettingsSectionState extends State<StatisticsSettingsSection> {
  late DateTimeRange _dateRange;
  _StatisticsLoadState _loadState = _StatisticsLoadState.loading;
  UsageStatistics? _statistics;
  String? _providerId;
  String? _modelId;
  String? _operation;
  String? _reasoningEffort;
  bool? _fastMode;
  bool _showMonthlyTrend = false;
  bool _isRefreshing = false;
  bool _isExporting = false;
  int _pageIndex = 0;
  int _loadGeneration = 0;

  static const _pageSize = 25;
  static const _exportPageSize = 200;

  @override
  void initState() {
    super.initState();
    final today = DateUtils.dateOnly(DateTime.now());
    _dateRange = DateTimeRange(
      start: today.subtract(const Duration(days: 29)),
      end: today,
    );
    unawaited(_load());
  }

  @override
  void didUpdateWidget(covariant StatisticsSettingsSection oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.serviceClient != widget.serviceClient) {
      unawaited(_load());
    }
  }

  @override
  void dispose() {
    _loadGeneration++;
    super.dispose();
  }

  Map<String, Object?> _queryParams({required int offset, required int limit}) {
    final start = DateTime(
      _dateRange.start.year,
      _dateRange.start.month,
      _dateRange.start.day,
    ).millisecondsSinceEpoch;
    final endExclusive = DateTime(
      _dateRange.end.year,
      _dateRange.end.month,
      _dateRange.end.day + 1,
    ).millisecondsSinceEpoch;
    final now = DateTime.now().millisecondsSinceEpoch + 1;
    final end = math.min(endExclusive, now);
    return <String, Object?>{
      'fromUnixMs': start,
      'toUnixMs': math.max(start + 1, end),
      'providerId': _providerId,
      'modelId': _modelId,
      'operation': _operation,
      'reasoningEffort': _reasoningEffort,
      'fastMode': _fastMode,
      'offset': offset,
      'limit': limit,
    };
  }

  Future<void> _load() async {
    final service = widget.serviceClient;
    final generation = ++_loadGeneration;
    if (service == null) {
      if (mounted) {
        setState(() {
          _loadState = _StatisticsLoadState.unavailable;
          _isRefreshing = false;
        });
      }
      return;
    }
    setState(() {
      _isRefreshing = true;
      if (_statistics == null) _loadState = _StatisticsLoadState.loading;
    });

    try {
      final response = await service.call(
        'statistics.get',
        params: _queryParams(offset: _pageIndex * _pageSize, limit: _pageSize),
      );
      final statistics = UsageStatistics.fromJson(response);
      if (!mounted || generation != _loadGeneration) return;
      setState(() {
        _statistics = statistics;
        _loadState = _StatisticsLoadState.loaded;
      });
    } on OpenChatServiceException catch (error, stackTrace) {
      _reportLoadFailure(error, stackTrace, generation);
    } on FormatException catch (error, stackTrace) {
      _reportLoadFailure(error, stackTrace, generation);
    } on Object catch (error, stackTrace) {
      _reportLoadFailure(error, stackTrace, generation);
    } finally {
      if (mounted && generation == _loadGeneration) {
        setState(() => _isRefreshing = false);
      }
    }
  }

  void _reportLoadFailure(Object error, StackTrace stackTrace, int generation) {
    FlutterError.reportError(
      FlutterErrorDetails(
        exception: error,
        stack: stackTrace,
        library: 'settings',
        context: ErrorDescription('while loading usage statistics'),
      ),
    );
    if (!mounted || generation != _loadGeneration) return;
    setState(() => _loadState = _StatisticsLoadState.failed);
  }

  Future<void> _chooseDateRange() async {
    final today = DateUtils.dateOnly(DateTime.now());
    final selected = await showDateRangePicker(
      context: context,
      firstDate: DateTime(2000),
      lastDate: today,
      initialDateRange: _dateRange,
      helpText: context.openchatL10n.statisticsDateRange,
    );
    if (!mounted || selected == null) return;
    setState(() {
      _dateRange = DateTimeRange(
        start: DateUtils.dateOnly(selected.start),
        end: DateUtils.dateOnly(selected.end),
      );
      _pageIndex = 0;
    });
    unawaited(_load());
  }

  void _setFilter(void Function() update) {
    if (_isExporting) return;
    setState(() {
      update();
      _pageIndex = 0;
    });
    unawaited(_load());
  }

  void _clearFilters() {
    _setFilter(() {
      _providerId = null;
      _modelId = null;
      _operation = null;
      _reasoningEffort = null;
      _fastMode = null;
    });
  }

  Future<void> _exportCsv() async {
    final service = widget.serviceClient;
    if (service == null || _isExporting) return;
    final l10n = context.openchatL10n;
    final query = _queryParams(offset: 0, limit: _exportPageSize);
    setState(() => _isExporting = true);
    try {
      final csv = StringBuffer('\uFEFF');
      final headers = <String>[
        l10n.statisticsRequestTime,
        l10n.statisticsConversationTitle,
        l10n.statisticsProvider,
        l10n.statisticsModel,
        l10n.statisticsOperation,
        l10n.statisticsReasoningEffort,
        l10n.statisticsFastModeFilter,
        l10n.statisticsStatus,
        l10n.statisticsInputTokens,
        l10n.statisticsOutputTokens,
        l10n.statisticsReasoningTokens,
        l10n.statisticsCachedInputTokens,
        l10n.statisticsCacheWriteTokens,
        l10n.statisticsProviderReportedCost,
        l10n.statisticsModelsDevCatalogCost,
        l10n.statisticsUsageSource,
        l10n.statisticsRunId,
      ];
      csv.writeln(headers.map(_csvField).join(','));
      var offset = 0;
      var total = 0;
      while (true) {
        final page = UsageStatistics.fromJson(
          await service.call(
            'statistics.get',
            params: <String, Object?>{
              ...query,
              'offset': offset,
              'limit': _exportPageSize,
            },
          ),
        );
        total = page.requestDetailCount;
        for (final request in page.requests) {
          csv.writeln(_csvRequestRow(request, l10n));
        }
        offset += page.requests.length;
        if (page.requests.isEmpty || offset >= total) break;
      }
      if (!mounted) return;
      final savedPath = await FilePicker.saveFile(
        fileName: 'openchat-usage-statistics.csv',
        bytes: Uint8List.fromList(utf8.encode(csv.toString())),
        mimeType: 'text/csv',
        dialogTitle: l10n.statisticsExportCsv,
        type: FileType.custom,
        allowedExtensions: const ['csv'],
        windowsOptions: const WindowsOptions(lockParentWindow: true),
        linuxOptions: const LinuxOptions(lockParentWindow: true),
      );
      if (!mounted || savedPath == null) return;
      showOpenChatToast(
        context,
        l10n.statisticsExported,
        type: OpenChatToastType.success,
      );
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while exporting usage statistics'),
        ),
      );
      if (mounted) {
        showOpenChatToast(
          context,
          context.openchatL10n.statisticsExportFailed,
          type: OpenChatToastType.error,
        );
      }
    } finally {
      if (mounted) setState(() => _isExporting = false);
    }
  }

  String _csvRequestRow(UsageStatisticsRequest request, AppLocalizations l10n) {
    final providerId = request.providerId;
    final reasoningEffort = request.reasoningEffort;
    final fastRequested = request.fastRequested;
    final values = <String>[
      DateTime.fromMillisecondsSinceEpoch(request.startedAtUnixMs)
          .toLocal()
          .toIso8601String(),
      request.conversationTitle,
      providerId == null
          ? l10n.statisticsNotReported
          : _providerLabel(providerId, l10n),
      request.modelId ?? l10n.statisticsNotReported,
      _operationLabel(request.operation, l10n),
      reasoningEffort == null
          ? l10n.statisticsNotReported
          : _reasoningLabel(reasoningEffort, l10n),
      fastRequested == null
          ? l10n.statisticsNotReported
          : fastRequested
          ? l10n.statisticsRequested
          : l10n.statisticsNotRequested,
      _statusLabel(request, l10n),
      _csvNumber(request.inputTokens),
      _csvNumber(request.outputTokens),
      _csvNumber(request.reasoningTokens),
      _csvNumber(request.cachedInputTokens),
      _csvNumber(request.cacheWriteTokens),
      request.providerReportedCostUsd?.toStringAsFixed(9) ?? '',
      request.modelsDevCatalogCostUsd?.toStringAsFixed(9) ?? '',
      request.usageSource,
      request.runId ?? '',
    ];
    return values.map(_csvField).join(',');
  }

  String _csvField(String value) => '"${value.replaceAll('"', '""')}"';

  String _csvNumber(int? value) => value?.toString() ?? '';

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final statistics = _statistics;

    if (_loadState == _StatisticsLoadState.unavailable) {
      return _buildMessageState(
        message: l10n.statisticsUnavailable,
        actionLabel: null,
        onAction: null,
      );
    }
    if (_loadState == _StatisticsLoadState.loading && statistics == null) {
      return const Padding(
        padding: EdgeInsets.symmetric(vertical: 48),
        child: Center(child: CircularProgressIndicator()),
      );
    }
    if (_loadState == _StatisticsLoadState.failed && statistics == null) {
      return _buildMessageState(
        message: l10n.statisticsLoadFailed,
        actionLabel: l10n.statisticsRetry,
        onAction: () => unawaited(_load()),
      );
    }
    if (statistics == null) return const SizedBox.shrink();

    final detailsStart = _pageIndex * _pageSize;
    final detailsEnd = math.min(
      detailsStart + statistics.requests.length,
      statistics.requestDetailCount,
    );
    final showDetails = statistics.requestDetailCount > 0;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _buildHeader(l10n, palette),
        const SizedBox(height: 16),
        _buildFilters(statistics, l10n, palette),
        if (_isRefreshing) ...[
          const SizedBox(height: 10),
          const LinearProgressIndicator(minHeight: 2),
        ],
        if (_loadState == _StatisticsLoadState.failed) ...[
          const SizedBox(height: 10),
          _buildInlineError(l10n, palette),
        ],
        const SizedBox(height: 18),
        _buildOverview(statistics, l10n, palette),
        const SizedBox(height: 20),
        _buildTrend(statistics, l10n, palette),
        const SizedBox(height: 14),
        _buildBreakdowns(statistics, l10n, palette),
        const SizedBox(height: 20),
        _buildConversations(statistics, l10n, palette),
        const SizedBox(height: 20),
        _buildQuotaHistory(statistics.quotaHistory, l10n, palette),
        const SizedBox(height: 20),
        _buildRequestDetails(
          statistics,
          l10n,
          palette,
          showDetails: showDetails,
          detailsStart: detailsStart,
          detailsEnd: detailsEnd,
        ),
      ],
    );
  }

  Widget _buildHeader(AppLocalizations l10n, OpenChatPalette palette) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final stack = constraints.maxWidth < 560;
        final description = Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              l10n.statistics,
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const SizedBox(height: 4),
            Text(
              l10n.statisticsDescription,
              style: TextStyle(
                color: palette.secondaryText,
                fontSize: 13,
                height: 1.45,
              ),
            ),
          ],
        );
        final actions = Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            OutlinedButton.icon(
              onPressed: _isExporting ? null : () => unawaited(_load()),
              icon: _isRefreshing
                  ? const SizedBox.square(
                      dimension: 15,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(LucideIcons.refreshCw, size: 16),
              label: Text(l10n.statisticsRetry),
            ),
            FilledButton.icon(
              onPressed: _isExporting ? null : () => unawaited(_exportCsv()),
              icon: _isExporting
                  ? const SizedBox.square(
                      dimension: 15,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(LucideIcons.download, size: 16),
              label: Text(
                _isExporting
                    ? l10n.statisticsExporting
                    : l10n.statisticsExportCsv,
              ),
            ),
          ],
        );
        if (stack) {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [description, const SizedBox(height: 12), actions],
          );
        }
        return Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(child: description),
            const SizedBox(width: 12),
            actions,
          ],
        );
      },
    );
  }

  Widget _buildFilters(
    UsageStatistics statistics,
    AppLocalizations l10n,
    OpenChatPalette palette,
  ) {
    final fastMode = _fastMode;
    final locale = Localizations.localeOf(context).toString();
    final dateFormat = DateFormat.yMMMd(locale);
    final dateLabel =
        '${dateFormat.format(_dateRange.start)} - ${dateFormat.format(_dateRange.end)}';
    return _StatisticsPanel(
      palette: palette,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Wrap(
            spacing: 10,
            runSpacing: 10,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              OutlinedButton.icon(
                onPressed: _isExporting
                    ? null
                    : () => unawaited(_chooseDateRange()),
                icon: const Icon(LucideIcons.calendarDays, size: 16),
                label: Text('$dateLabel  ·  ${l10n.statisticsDateRange}'),
              ),
              TextButton.icon(
                onPressed: _isExporting ? null : _clearFilters,
                icon: const Icon(LucideIcons.x, size: 15),
                label: Text(l10n.statisticsClearFilters),
              ),
            ],
          ),
          const SizedBox(height: 12),
          LayoutBuilder(
            builder: (context, constraints) {
              final columns = constraints.maxWidth >= 720
                  ? 3
                  : constraints.maxWidth >= 460
                  ? 2
                  : 1;
              final itemWidth =
                  (constraints.maxWidth - (columns - 1) * 10) / columns;
              final effortOptions = [
                ...statistics.filterOptions.reasoningEfforts,
              ];
              return Wrap(
                spacing: 10,
                runSpacing: 10,
                children: [
                  SizedBox(
                    width: itemWidth,
                    child: _buildFilterSelect(
                      label: l10n.statisticsProvider,
                      value: _providerId ?? '',
                      options: [
                        ('', l10n.statisticsAll),
                        ...statistics.filterOptions.providers.map(
                          (value) => (value, _providerLabel(value, l10n)),
                        ),
                      ],
                      palette: palette,
                      onChanged: _isExporting
                          ? null
                          : (value) => _setFilter(
                              () => _providerId = value.isEmpty ? null : value,
                            ),
                    ),
                  ),
                  SizedBox(
                    width: itemWidth,
                    child: _buildFilterSelect(
                      label: l10n.statisticsModel,
                      value: _modelId ?? '',
                      options: [
                        ('', l10n.statisticsAll),
                        ...statistics.filterOptions.models.map(
                          (value) => (value, value),
                        ),
                      ],
                      palette: palette,
                      onChanged: _isExporting
                          ? null
                          : (value) => _setFilter(
                              () => _modelId = value.isEmpty ? null : value,
                            ),
                    ),
                  ),
                  SizedBox(
                    width: itemWidth,
                    child: _buildFilterSelect(
                      label: l10n.statisticsOperation,
                      value: _operation ?? '',
                      options: [
                        ('', l10n.statisticsAll),
                        ('chat', l10n.statisticsOperationChat),
                        (
                          'tool_follow_up',
                          l10n.statisticsOperationToolFollowUp,
                        ),
                        ('compaction', l10n.statisticsOperationCompaction),
                        (
                          'title_generation',
                          l10n.statisticsOperationTitleGeneration,
                        ),
                      ],
                      palette: palette,
                      onChanged: _isExporting
                          ? null
                          : (value) => _setFilter(
                              () => _operation = value.isEmpty ? null : value,
                            ),
                    ),
                  ),
                  SizedBox(
                    width: itemWidth,
                    child: _buildFilterSelect(
                      label: l10n.statisticsReasoningEffort,
                      value: _reasoningEffort ?? '',
                      options: [
                        ('', l10n.statisticsAll),
                        ...effortOptions.map(
                          (value) => (value, _reasoningLabel(value, l10n)),
                        ),
                      ],
                      palette: palette,
                      onChanged: _isExporting
                          ? null
                          : (value) => _setFilter(
                              () => _reasoningEffort = value.isEmpty
                                  ? null
                                  : value,
                            ),
                    ),
                  ),
                  SizedBox(
                    width: itemWidth,
                    child: _buildFilterSelect(
                      label: l10n.statisticsFastModeFilter,
                      value: fastMode == null
                          ? ''
                          : fastMode
                          ? 'true'
                          : 'false',
                      options: [
                        ('', l10n.statisticsAll),
                        ('true', l10n.statisticsRequested),
                        ('false', l10n.statisticsNotRequested),
                      ],
                      palette: palette,
                      onChanged: _isExporting
                          ? null
                          : (value) => _setFilter(() {
                              _fastMode = switch (value) {
                                'true' => true,
                                'false' => false,
                                _ => null,
                              };
                            }),
                    ),
                  ),
                ],
              );
            },
          ),
        ],
      ),
    );
  }

  Widget _buildFilterSelect({
    required String label,
    required String value,
    required List<(String, String)> options,
    required OpenChatPalette palette,
    required ValueChanged<String>? onChanged,
  }) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          label,
          style: TextStyle(
            color: palette.secondaryText,
            fontSize: 12,
            fontWeight: FontWeight.w500,
          ),
        ),
        const SizedBox(height: 5),
        OpenChatSelect<String>(
          options: options
              .map(
                (option) => OpenChatSelectOption<String>(
                  value: option.$1,
                  label: option.$2,
                ),
              )
              .toList(growable: false),
          value: value,
          onChanged: onChanged,
          palette: palette,
          width: double.infinity,
          height: 40,
          compact: true,
        ),
      ],
    );
  }

  Widget _buildOverview(
    UsageStatistics statistics,
    AppLocalizations l10n,
    OpenChatPalette palette,
  ) {
    final summary = statistics.summary;
    final locale = Localizations.localeOf(context).toString();
    final number = NumberFormat.decimalPattern(locale);
    final reportedCostValue = summary.costReportedRequests == 0
        ? l10n.statisticsNotReported
        : _formatCostUsd(summary.providerReportedCostUsd, locale);
    final catalogCostValue = summary.modelsDevCatalogCostRequests == 0
        ? l10n.statisticsNotReported
        : _formatCostUsd(summary.modelsDevCatalogCostUsd, locale);
    final metrics = <_StatisticsMetricData>[
      _StatisticsMetricData(
        label: l10n.statisticsTotalTokens,
        value: number.format(summary.totalTokens),
        detail:
            '${l10n.statisticsInputTokens}: ${number.format(summary.inputTokens)} · ${l10n.statisticsOutputTokens}: ${number.format(summary.outputTokens)}',
        icon: LucideIcons.coins,
      ),
      _StatisticsMetricData(
        label: l10n.statisticsRequests,
        value: number.format(summary.totalRequests),
        detail:
            '${l10n.statisticsSuccessfulRequests}: ${number.format(summary.successfulRequests)}',
        icon: LucideIcons.activity,
      ),
      _StatisticsMetricData(
        label: l10n.statisticsConversations,
        value: number.format(summary.conversationCount),
        detail: l10n.statisticsConversations,
        icon: LucideIcons.messagesSquare,
      ),
      _StatisticsMetricData(
        label: l10n.statisticsReasoningTokens,
        value: number.format(summary.reasoningTokens),
        detail: l10n.statisticsReasoningLevels,
        icon: LucideIcons.brain,
      ),
      _StatisticsMetricData(
        label: l10n.statisticsCachedInputTokens,
        value: number.format(summary.cachedInputTokens),
        detail:
            '${l10n.statisticsCacheWriteTokens}: ${number.format(summary.cacheWriteTokens)}',
        icon: LucideIcons.database,
      ),
      _StatisticsMetricData(
        label: l10n.statisticsProviderReportedCost,
        value: reportedCostValue,
        detail: l10n.statisticsCostCoverage(summary.costReportedRequests),
        icon: LucideIcons.circleDollarSign,
      ),
      _StatisticsMetricData(
        label: l10n.statisticsModelsDevCatalogCost,
        value: catalogCostValue,
        detail: l10n.statisticsModelsDevCatalogCostCoverage(
          summary.modelsDevCatalogCostRequests,
        ),
        icon: LucideIcons.receipt,
      ),
    ];
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        LayoutBuilder(
          builder: (context, constraints) {
            final columns = constraints.maxWidth >= 700
                ? 3
                : constraints.maxWidth >= 430
                ? 2
                : 1;
            final cardWidth =
                (constraints.maxWidth - (columns - 1) * 10) / columns;
            return Wrap(
              spacing: 10,
              runSpacing: 10,
              children: [
                for (final metric in metrics)
                  SizedBox(
                    width: cardWidth,
                    child: _StatisticsMetricCard(
                      data: metric,
                      palette: palette,
                    ),
                  ),
              ],
            );
          },
        ),
        const SizedBox(height: 12),
        _StatisticsPanel(
          palette: palette,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(
                l10n.statisticsCoverage,
                style: Theme.of(context).textTheme.titleSmall,
              ),
              const SizedBox(height: 5),
              Text(
                l10n.statisticsCoverageText(
                  summary.totalRequests,
                  summary.anyUsageRequests,
                ),
                style: TextStyle(color: palette.secondaryText, fontSize: 12),
              ),
              const SizedBox(height: 12),
              Wrap(
                spacing: 10,
                runSpacing: 8,
                children: [
                  _StatisticsCountPill(
                    label: l10n.statisticsSuccessfulRequests,
                    count: summary.successfulRequests,
                    palette: palette,
                  ),
                  _StatisticsCountPill(
                    label: l10n.statisticsFailedRequests,
                    count: summary.failedRequests,
                    palette: palette,
                  ),
                  _StatisticsCountPill(
                    label: l10n.statisticsCancelledRequests,
                    count: summary.cancelledRequests,
                    palette: palette,
                  ),
                  _StatisticsCountPill(
                    label: l10n.statisticsInterruptedRequests,
                    count: summary.interruptedRequests,
                    palette: palette,
                  ),
                  _StatisticsCountPill(
                    label: l10n.statisticsPendingRequests,
                    count: summary.pendingRequests,
                    palette: palette,
                  ),
                ],
              ),
              const SizedBox(height: 8),
              Wrap(
                spacing: 10,
                runSpacing: 8,
                children: [
                  _StatisticsCountPill(
                    label: l10n.statisticsInputCoverage,
                    count: summary.inputUsageRequests,
                    palette: palette,
                  ),
                  _StatisticsCountPill(
                    label: l10n.statisticsOutputCoverage,
                    count: summary.outputUsageRequests,
                    palette: palette,
                  ),
                  _StatisticsCountPill(
                    label: l10n.statisticsReasoningCoverage,
                    count: summary.reasoningUsageRequests,
                    palette: palette,
                  ),
                ],
              ),
              const SizedBox(height: 10),
              Text(
                l10n.statisticsModelsDevPricingNote,
                style: TextStyle(color: palette.secondaryText, fontSize: 12),
              ),
              const SizedBox(height: 4),
              Text(
                _modelsDevPricingStatus(
                  statistics.modelsDevPricing,
                  l10n,
                  locale,
                ),
                style: TextStyle(color: palette.secondaryText, fontSize: 12),
              ),
              if (summary.legacyOutputMessages > 0) ...[
                const SizedBox(height: 8),
                Text(
                  '${l10n.statisticsLegacyOutput}: ${number.format(summary.legacyOutputMessages)}',
                  style: TextStyle(color: palette.secondaryText, fontSize: 12),
                ),
                const SizedBox(height: 4),
                Text(
                  l10n.statisticsLegacyDataNote,
                  style: TextStyle(color: palette.secondaryText, fontSize: 12),
                ),
              ],
            ],
          ),
        ),
      ],
    );
  }

  Widget _buildTrend(
    UsageStatistics statistics,
    AppLocalizations l10n,
    OpenChatPalette palette,
  ) {
    final points = _showMonthlyTrend
        ? statistics.monthlyTrend
        : statistics.dailyTrend;
    return _StatisticsPanel(
      palette: palette,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Expanded(
                child: Text(
                  l10n.statisticsUsageTrend,
                  style: Theme.of(context).textTheme.titleSmall,
                ),
              ),
              SegmentedButton<String>(
                segments: [
                  ButtonSegment<String>(
                    value: 'day',
                    label: Text(l10n.statisticsDaily),
                  ),
                  ButtonSegment<String>(
                    value: 'month',
                    label: Text(l10n.statisticsMonthly),
                  ),
                ],
                selected: {_showMonthlyTrend ? 'month' : 'day'},
                showSelectedIcon: false,
                onSelectionChanged: _isExporting
                    ? null
                    : (value) => setState(
                        () => _showMonthlyTrend = value.contains('month'),
                      ),
              ),
            ],
          ),
          const SizedBox(height: 14),
          if (points.isEmpty)
            Text(
              l10n.statisticsNoBreakdownData,
              style: TextStyle(color: palette.secondaryText, fontSize: 13),
            )
          else
            _UsageTrendChart(
              points: points,
              monthly: _showMonthlyTrend,
              palette: palette,
              l10n: l10n,
            ),
        ],
      ),
    );
  }

  Widget _buildBreakdowns(
    UsageStatistics statistics,
    AppLocalizations l10n,
    OpenChatPalette palette,
  ) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final columns = constraints.maxWidth >= 700 ? 2 : 1;
        final width = (constraints.maxWidth - (columns - 1) * 12) / columns;
        return Wrap(
          spacing: 12,
          runSpacing: 12,
          children: [
            SizedBox(
              width: width,
              child: _StatisticsBreakdownPanel(
                title: l10n.statisticsProviders,
                items: statistics.providers,
                palette: palette,
                emptyLabel: l10n.statisticsNoBreakdownData,
                labelFor: (key) => _providerLabel(key, l10n),
              ),
            ),
            SizedBox(
              width: width,
              child: _StatisticsBreakdownPanel(
                title: l10n.statisticsModels,
                items: statistics.models,
                palette: palette,
                emptyLabel: l10n.statisticsNoBreakdownData,
                labelFor: (key) => key,
              ),
            ),
            SizedBox(
              width: width,
              child: _StatisticsBreakdownPanel(
                title: l10n.statisticsReasoningLevels,
                items: statistics.reasoning,
                palette: palette,
                emptyLabel: l10n.statisticsNoBreakdownData,
                labelFor: (key) => _reasoningLabel(key, l10n),
                compact: true,
              ),
            ),
            SizedBox(
              width: width,
              child: _StatisticsBreakdownPanel(
                title: l10n.statisticsOperations,
                items: statistics.operations,
                palette: palette,
                emptyLabel: l10n.statisticsNoBreakdownData,
                labelFor: (key) => _operationLabel(key, l10n),
                compact: true,
              ),
            ),
            SizedBox(
              width: width,
              child: _buildFastModePanel(statistics.fastMode, l10n, palette),
            ),
            SizedBox(
              width: width,
              child: _StatisticsBreakdownPanel(
                title: l10n.statisticsServiceTiers,
                items: statistics.serviceTiers,
                palette: palette,
                emptyLabel: l10n.statisticsNoBreakdownData,
                labelFor: (key) => key,
                compact: true,
              ),
            ),
          ],
        );
      },
    );
  }

  Widget _buildFastModePanel(
    List<UsageStatisticsFastMode> values,
    AppLocalizations l10n,
    OpenChatPalette palette,
  ) {
    final number = NumberFormat.decimalPattern(
      Localizations.localeOf(context).toString(),
    );
    final locale = Localizations.localeOf(context).toString();
    return _StatisticsPanel(
      palette: palette,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(
            l10n.statisticsFastModeUsage,
            style: Theme.of(context).textTheme.titleSmall,
          ),
          const SizedBox(height: 12),
          if (values.isEmpty)
            Text(
              l10n.statisticsNoBreakdownData,
              style: TextStyle(color: palette.secondaryText, fontSize: 12),
            )
          else
            for (var index = 0; index < values.length; index++) ...[
              if (index > 0) const SizedBox(height: 10),
              Row(
                children: [
                  Expanded(
                    child: Text(
                      values[index].fastRequested == true
                          ? l10n.statisticsRequested
                          : values[index].fastRequested == false
                          ? l10n.statisticsNotRequested
                          : l10n.statisticsNotReported,
                      style: TextStyle(color: palette.text, fontSize: 13),
                    ),
                  ),
                  Text(
                    number.format(values[index].requests),
                    style: TextStyle(
                      color: palette.secondaryText,
                      fontSize: 12,
                      fontFeatures: const [FontFeature.tabularFigures()],
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 3),
              Text(
                <String>[
                  '${l10n.statisticsInputTokens}: ${number.format(values[index].inputTokens)}',
                  '${l10n.statisticsOutputTokens}: ${number.format(values[index].outputTokens)}',
                  if (values[index].costReportedRequests > 0)
                    '${l10n.statisticsProviderReportedCost}: ${_formatCostUsd(values[index].providerReportedCostUsd, locale)} · ${l10n.statisticsCostCoverage(values[index].costReportedRequests)}',
                  if (values[index].modelsDevCatalogCostRequests > 0)
                    '${l10n.statisticsModelsDevCatalogCost}: ${_formatCostUsd(values[index].modelsDevCatalogCostUsd, locale)} · ${l10n.statisticsModelsDevCatalogCostCoverage(values[index].modelsDevCatalogCostRequests)}',
                ].join(' · '),
                style: TextStyle(color: palette.secondaryText, fontSize: 11),
              ),
            ],
        ],
      ),
    );
  }

  Widget _buildConversations(
    UsageStatistics statistics,
    AppLocalizations l10n,
    OpenChatPalette palette,
  ) {
    final locale = Localizations.localeOf(context).toString();
    final number = NumberFormat.decimalPattern(locale);
    final onOpenConversation = widget.onOpenConversation;
    return _StatisticsPanel(
      palette: palette,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(
            l10n.statisticsConversations,
            style: Theme.of(context).textTheme.titleSmall,
          ),
          const SizedBox(height: 8),
          if (statistics.conversations.isEmpty)
            Text(
              l10n.statisticsNoConversationData,
              style: TextStyle(color: palette.secondaryText, fontSize: 13),
            )
          else
            for (
              var index = 0;
              index < statistics.conversations.length;
              index++
            )
              _ConversationUsageRow(
                item: statistics.conversations[index],
                numberFormat: number,
                palette: palette,
                l10n: l10n,
                onTap: onOpenConversation == null
                    ? null
                    : () => onOpenConversation(
                        statistics.conversations[index].conversationId,
                      ),
              ),
        ],
      ),
    );
  }

  Widget _buildQuotaHistory(
    List<UsageQuotaSnapshot> snapshots,
    AppLocalizations l10n,
    OpenChatPalette palette,
  ) {
    final locale = Localizations.localeOf(context).toString();
    final dateFormat = DateFormat.yMMMd(locale).add_jm();
    final number = NumberFormat.decimalPattern(locale);
    final percent = NumberFormat.percentPattern(locale);
    return _StatisticsPanel(
      palette: palette,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(
            l10n.statisticsQuotaHistory,
            style: Theme.of(context).textTheme.titleSmall,
          ),
          const SizedBox(height: 8),
          if (snapshots.isEmpty)
            Text(
              l10n.statisticsNoQuotaHistory,
              style: TextStyle(color: palette.secondaryText, fontSize: 13),
            )
          else
            for (var index = 0; index < snapshots.length; index++) ...[
              if (index > 0)
                Padding(
                  padding: const EdgeInsets.symmetric(vertical: 10),
                  child: Divider(height: 1, color: palette.border),
                ),
              _QuotaHistorySnapshotCard(
                snapshot: snapshots[index],
                dateLabel: dateFormat.format(
                  DateTime.fromMillisecondsSinceEpoch(
                    snapshots[index].fetchedAtUnixMs,
                  ).toLocal(),
                ),
                l10n: l10n,
                palette: palette,
                numberFormat: number,
                percentFormat: percent,
              ),
            ],
        ],
      ),
    );
  }

  Widget _buildRequestDetails(
    UsageStatistics statistics,
    AppLocalizations l10n,
    OpenChatPalette palette, {
    required bool showDetails,
    required int detailsStart,
    required int detailsEnd,
  }) {
    final locale = Localizations.localeOf(context).toString();
    final dateFormat = DateFormat.yMMMd(locale).add_jm();
    final number = NumberFormat.decimalPattern(locale);
    final pageCount = statistics.requestDetailCount == 0
        ? 0
        : (statistics.requestDetailCount / _pageSize).ceil();
    return _StatisticsPanel(
      palette: palette,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          LayoutBuilder(
            builder: (context, constraints) {
              final title = Text(
                l10n.statisticsRequestDetails,
                style: Theme.of(context).textTheme.titleSmall,
              );
              final rows = Text(
                showDetails
                    ? l10n.statisticsShowingRows(
                        detailsStart + 1,
                        detailsEnd,
                        statistics.requestDetailCount,
                      )
                    : '0',
                style: TextStyle(color: palette.secondaryText, fontSize: 12),
              );
              if (constraints.maxWidth < 450) {
                return Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [title, const SizedBox(height: 4), rows],
                );
              }
              return Row(
                children: [
                  Expanded(child: title),
                  rows,
                ],
              );
            },
          ),
          const SizedBox(height: 10),
          Text(
            l10n.statisticsLegacyDataNote,
            style: TextStyle(color: palette.secondaryText, fontSize: 12),
          ),
          const SizedBox(height: 12),
          if (!showDetails)
            Text(
              l10n.statisticsNoRequestData,
              style: TextStyle(color: palette.secondaryText, fontSize: 13),
            )
          else if (statistics.requests.isEmpty)
            const Center(
              child: Padding(
                padding: EdgeInsets.symmetric(vertical: 20),
                child: CircularProgressIndicator(),
              ),
            )
          else ...[
            for (
              var index = 0;
              index < statistics.requests.length;
              index++
            ) ...[
              if (index > 0)
                Padding(
                  padding: const EdgeInsets.symmetric(vertical: 8),
                  child: Divider(height: 1, color: palette.border),
                ),
              _UsageRequestDetail(
                request: statistics.requests[index],
                dateLabel: dateFormat.format(
                  DateTime.fromMillisecondsSinceEpoch(
                    statistics.requests[index].startedAtUnixMs,
                  ).toLocal(),
                ),
                numberFormat: number,
                l10n: l10n,
                palette: palette,
              ),
            ],
            if (pageCount > 1) ...[
              const SizedBox(height: 12),
              Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  IconButton(
                    tooltip: MaterialLocalizations.of(context)
                        .previousPageTooltip,
                    onPressed: _pageIndex == 0 || _isExporting
                        ? null
                        : () {
                            setState(() => _pageIndex--);
                            unawaited(_load());
                          },
                    icon: const Icon(LucideIcons.chevronLeft, size: 18),
                  ),
                  Text(
                    '${_pageIndex + 1} / $pageCount',
                    style: TextStyle(
                      color: palette.secondaryText,
                      fontSize: 12,
                    ),
                  ),
                  IconButton(
                    tooltip: MaterialLocalizations.of(context).nextPageTooltip,
                    onPressed: _pageIndex + 1 >= pageCount || _isExporting
                        ? null
                        : () {
                            setState(() => _pageIndex++);
                            unawaited(_load());
                          },
                    icon: const Icon(LucideIcons.chevronRight, size: 18),
                  ),
                ],
              ),
            ],
          ],
        ],
      ),
    );
  }

  Widget _buildInlineError(AppLocalizations l10n, OpenChatPalette palette) {
    return Semantics(
      liveRegion: true,
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
        decoration: BoxDecoration(
          color: Theme.of(context).colorScheme.errorContainer,
          borderRadius: BorderRadius.circular(8),
          border: Border.all(color: Theme.of(context).colorScheme.error),
        ),
        child: Row(
          children: [
            Icon(
              LucideIcons.circleAlert,
              size: 17,
              color: Theme.of(context).colorScheme.error,
            ),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                l10n.statisticsLoadFailed,
                style: TextStyle(color: palette.text, fontSize: 12),
              ),
            ),
            TextButton(
              onPressed: () => unawaited(_load()),
              child: Text(l10n.statisticsRetry),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildMessageState({
    required String message,
    required String? actionLabel,
    required VoidCallback? onAction,
  }) {
    final palette = OpenChatPalette.of(context);
    return Center(
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 48, horizontal: 16),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              LucideIcons.chartNoAxesCombined,
              size: 24,
              color: palette.secondaryIcon,
            ),
            const SizedBox(height: 12),
            Text(
              message,
              textAlign: TextAlign.center,
              style: TextStyle(color: palette.secondaryText, fontSize: 13),
            ),
            if (actionLabel != null && onAction != null) ...[
              const SizedBox(height: 12),
              OutlinedButton.icon(
                onPressed: onAction,
                icon: const Icon(LucideIcons.refreshCw, size: 16),
                label: Text(actionLabel),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _StatisticsMetricData {
  const _StatisticsMetricData({
    required this.label,
    required this.value,
    required this.detail,
    required this.icon,
  });

  final String label;
  final String value;
  final String detail;
  final IconData icon;
}

class _StatisticsMetricCard extends StatelessWidget {
  const _StatisticsMetricCard({required this.data, required this.palette});

  final _StatisticsMetricData data;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Semantics(
      container: true,
      label: '${data.label}: ${data.value}. ${data.detail}',
      child: Container(
        constraints: const BoxConstraints(minHeight: 112),
        decoration: BoxDecoration(
          color: theme.colorScheme.surfaceContainerLow,
          border: Border.all(color: palette.border),
          borderRadius: BorderRadius.circular(9),
        ),
        padding: const EdgeInsets.all(13),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(data.icon, size: 16, color: palette.accentIcon),
                const SizedBox(width: 7),
                Expanded(
                  child: Text(
                    data.label,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      color: palette.secondaryText,
                      fontSize: 12,
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 10),
            Text(
              data.value,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                color: palette.text,
                fontSize: 20,
                height: 1.2,
                fontWeight: FontWeight.w600,
                fontFeatures: const [FontFeature.tabularFigures()],
              ),
            ),
            const SizedBox(height: 4),
            Text(
              data.detail,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(color: palette.secondaryText, fontSize: 11),
            ),
          ],
        ),
      ),
    );
  }
}

class _StatisticsCountPill extends StatelessWidget {
  const _StatisticsCountPill({
    required this.label,
    required this.count,
    required this.palette,
  });

  final String label;
  final int count;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final number = NumberFormat.decimalPattern(
      Localizations.localeOf(context).toString(),
    );
    return Container(
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surfaceContainerHighest
            .withValues(alpha: 0.45),
        borderRadius: BorderRadius.circular(6),
      ),
      padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 6),
      child: Text(
        '$label: ${number.format(count)}',
        style: TextStyle(color: palette.text, fontSize: 11),
      ),
    );
  }
}

class _StatisticsPanel extends StatelessWidget {
  const _StatisticsPanel({required this.palette, required this.child});

  final OpenChatPalette palette;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surfaceContainerLow,
        border: Border.all(color: palette.border),
        borderRadius: BorderRadius.circular(9),
      ),
      padding: const EdgeInsets.all(14),
      child: child,
    );
  }
}

class _UsageTrendChart extends StatelessWidget {
  const _UsageTrendChart({
    required this.points,
    required this.monthly,
    required this.palette,
    required this.l10n,
  });

  final List<UsageStatisticsTrend> points;
  final bool monthly;
  final OpenChatPalette palette;
  final AppLocalizations l10n;

  @override
  Widget build(BuildContext context) {
    final maxTokens = points.fold<int>(
      0,
      (current, point) => math.max(
        current,
        point.inputTokens.saturatingAdd(point.outputTokens),
      ),
    );
    final chartColors = Theme.of(context).colorScheme;
    final locale = Localizations.localeOf(context).toString();
    final number = NumberFormat.decimalPattern(locale);
    final labelStep = math.max(1, (points.length / 7).ceil());
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        LayoutBuilder(
          builder: (context, constraints) {
            final itemExtent = math
                .max(constraints.maxWidth / points.length, 22.0)
                .toDouble();
            return Scrollbar(
              child: SizedBox(
                height: 156,
                child: ListView.builder(
                  scrollDirection: Axis.horizontal,
                  itemExtent: itemExtent,
                  itemCount: points.length,
                  itemBuilder: (context, index) => _TrendBar(
                    point: points[index],
                    maxTokens: maxTokens,
                    showLabel:
                        index % labelStep == 0 || index == points.length - 1,
                    monthly: monthly,
                    locale: locale,
                    numberFormat: number,
                    inputColor: palette.accent,
                    outputColor: chartColors.tertiary,
                    palette: palette,
                  ),
                ),
              ),
            );
          },
        ),
        const SizedBox(height: 12),
        Wrap(
          spacing: 16,
          runSpacing: 8,
          children: [
            _TrendLegend(
              color: palette.accent,
              label: l10n.statisticsInputTokens,
            ),
            _TrendLegend(
              color: chartColors.tertiary,
              label: l10n.statisticsOutputTokens,
            ),
          ],
        ),
      ],
    );
  }
}

class _TrendBar extends StatelessWidget {
  const _TrendBar({
    required this.point,
    required this.maxTokens,
    required this.showLabel,
    required this.monthly,
    required this.locale,
    required this.numberFormat,
    required this.inputColor,
    required this.outputColor,
    required this.palette,
  });

  final UsageStatisticsTrend point;
  final int maxTokens;
  final bool showLabel;
  final bool monthly;
  final String locale;
  final NumberFormat numberFormat;
  final Color inputColor;
  final Color outputColor;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    const barAreaHeight = 118.0;
    final total = point.inputTokens.saturatingAdd(point.outputTokens);
    final fraction = maxTokens == 0 ? 0.0 : total / maxTokens;
    final inputHeight = total == 0
        ? 0.0
        : barAreaHeight * fraction * point.inputTokens / total;
    final outputHeight = total == 0
        ? 0.0
        : barAreaHeight * fraction * point.outputTokens / total;
    final labelDate = DateTime.tryParse(
      monthly ? '${point.bucket}-01' : point.bucket,
    );
    final tickLabel = labelDate == null
        ? point.bucket
        : monthly
        ? DateFormat.MMM(locale).format(labelDate)
        : DateFormat.Md(locale).format(labelDate);
    final catalogCost = point.modelsDevCatalogCostRequests == 0
        ? null
        : '${_localizedCatalogCostLabel(context)}: ${_formatCostUsd(point.modelsDevCatalogCostUsd, locale)}';
    final tooltip = <String>[
      point.bucket,
      '${numberFormat.format(point.inputTokens)} ${_localizedInputLabel(context)}',
      '${numberFormat.format(point.outputTokens)} ${_localizedOutputLabel(context)}',
      '${numberFormat.format(point.requests)} ${_localizedRequestLabel(context)}',
      ?catalogCost,
    ].join('\n');
    return Tooltip(
      message: tooltip,
      child: Semantics(
        label: tooltip,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 1),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [
              Expanded(
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.end,
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Flexible(
                      child: Container(
                        width: 7,
                        height: inputHeight,
                        decoration: BoxDecoration(
                          color: inputColor,
                          borderRadius: const BorderRadius.vertical(
                            top: Radius.circular(2),
                          ),
                        ),
                      ),
                    ),
                    const SizedBox(width: 1),
                    Flexible(
                      child: Container(
                        width: 7,
                        height: outputHeight,
                        decoration: BoxDecoration(
                          color: outputColor,
                          borderRadius: const BorderRadius.vertical(
                            top: Radius.circular(2),
                          ),
                        ),
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 5),
              SizedBox(
                height: 18,
                child: showLabel
                    ? Text(
                        tickLabel,
                        maxLines: 1,
                        overflow: TextOverflow.clip,
                        style: TextStyle(
                          color: palette.secondaryText,
                          fontSize: 9,
                        ),
                      )
                    : null,
              ),
            ],
          ),
        ),
      ),
    );
  }

  String _localizedInputLabel(BuildContext context) =>
      context.openchatL10n.statisticsInputTokens;

  String _localizedOutputLabel(BuildContext context) =>
      context.openchatL10n.statisticsOutputTokens;

  String _localizedRequestLabel(BuildContext context) =>
      context.openchatL10n.statisticsRequests;

  String _localizedCatalogCostLabel(BuildContext context) =>
      context.openchatL10n.statisticsModelsDevCatalogCost;
}

class _TrendLegend extends StatelessWidget {
  const _TrendLegend({required this.color, required this.label});

  final Color color;
  final String label;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Container(
          width: 9,
          height: 9,
          decoration: BoxDecoration(
            color: color,
            borderRadius: BorderRadius.circular(2),
          ),
        ),
        const SizedBox(width: 6),
        Text(
          label,
          style: TextStyle(color: palette.secondaryText, fontSize: 11),
        ),
      ],
    );
  }
}

class _StatisticsBreakdownPanel extends StatelessWidget {
  const _StatisticsBreakdownPanel({
    required this.title,
    required this.items,
    required this.palette,
    required this.emptyLabel,
    required this.labelFor,
    this.compact = false,
  });

  final String title;
  final List<UsageStatisticsBreakdown> items;
  final OpenChatPalette palette;
  final String emptyLabel;
  final String Function(String key) labelFor;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final maxTokens = items.fold<int>(
      0,
      (current, item) =>
          math.max(current, item.inputTokens.saturatingAdd(item.outputTokens)),
    );
    return _StatisticsPanel(
      palette: palette,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(title, style: Theme.of(context).textTheme.titleSmall),
          const SizedBox(height: 12),
          if (items.isEmpty)
            Text(
              emptyLabel,
              style: TextStyle(color: palette.secondaryText, fontSize: 12),
            )
          else
            for (var index = 0; index < items.length; index++) ...[
              if (index > 0) const SizedBox(height: 12),
              _StatisticsBreakdownRow(
                item: items[index],
                label: labelFor(items[index].key),
                maxTokens: maxTokens,
                palette: palette,
                compact: compact,
              ),
            ],
        ],
      ),
    );
  }
}

class _StatisticsBreakdownRow extends StatelessWidget {
  const _StatisticsBreakdownRow({
    required this.item,
    required this.label,
    required this.maxTokens,
    required this.palette,
    required this.compact,
  });

  final UsageStatisticsBreakdown item;
  final String label;
  final int maxTokens;
  final OpenChatPalette palette;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final locale = Localizations.localeOf(context).toString();
    final number = NumberFormat.decimalPattern(locale);
    final l10n = context.openchatL10n;
    final tokens = item.inputTokens.saturatingAdd(item.outputTokens);
    final value = maxTokens == 0 ? 0.0 : tokens / maxTokens;
    final theme = Theme.of(context);
    final costLabels = <String>[
      if (item.costReportedRequests > 0)
        '${l10n.statisticsProviderReportedCost}: ${_formatCostUsd(item.providerReportedCostUsd, locale)} · ${l10n.statisticsCostCoverage(item.costReportedRequests)}',
      if (item.modelsDevCatalogCostRequests > 0)
        '${l10n.statisticsModelsDevCatalogCost}: ${_formatCostUsd(item.modelsDevCatalogCostUsd, locale)} · ${l10n.statisticsModelsDevCatalogCostCoverage(item.modelsDevCatalogCostRequests)}',
    ];
    return Semantics(
      label:
          '$label, ${number.format(tokens)} ${l10n.statisticsTotalTokens}, ${number.format(item.requests)} ${l10n.statisticsRequests}${costLabels.isEmpty ? '' : ', ${costLabels.join(', ')}'}',
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Expanded(
                child: Text(
                  label,
                  maxLines: compact ? 1 : 2,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(color: palette.text, fontSize: 12),
                ),
              ),
              const SizedBox(width: 8),
              Text(
                number.format(tokens),
                style: TextStyle(
                  color: palette.text,
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                  fontFeatures: const [FontFeature.tabularFigures()],
                ),
              ),
            ],
          ),
          const SizedBox(height: 5),
          ClipRRect(
            borderRadius: BorderRadius.circular(3),
            child: LinearProgressIndicator(
              value: value.clamp(0.0, 1.0),
              minHeight: 4,
              backgroundColor: theme.colorScheme.surfaceContainerHighest,
              color: palette.accent,
            ),
          ),
          const SizedBox(height: 4),
          Text(
            <String>[
              '${number.format(item.requests)} ${l10n.statisticsRequests}',
              '${l10n.statisticsInputTokens}: ${number.format(item.inputTokens)}',
              '${l10n.statisticsOutputTokens}: ${number.format(item.outputTokens)}',
              ...costLabels,
            ].join(' · '),
            maxLines: 3,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(color: palette.secondaryText, fontSize: 10),
          ),
        ],
      ),
    );
  }
}

class _ConversationUsageRow extends StatelessWidget {
  const _ConversationUsageRow({
    required this.item,
    required this.numberFormat,
    required this.palette,
    required this.l10n,
    required this.onTap,
  });

  final UsageStatisticsConversation item;
  final NumberFormat numberFormat;
  final OpenChatPalette palette;
  final AppLocalizations l10n;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final locale = Localizations.localeOf(context).toString();
    final date = DateFormat.yMMMd(locale).add_jm().format(
      DateTime.fromMillisecondsSinceEpoch(item.lastRequestAtUnixMs).toLocal(),
    );
    final label =
        '${item.title}, ${numberFormat.format(item.inputTokens.saturatingAdd(item.outputTokens))} ${l10n.statisticsTotalTokens}, ${numberFormat.format(item.requests)} ${l10n.statisticsRequests}';
    final catalogCostLabel = item.modelsDevCatalogCostRequests == 0
        ? null
        : '${l10n.statisticsModelsDevCatalogCost}: ${_formatCostUsd(item.modelsDevCatalogCostUsd, locale)}';
    final semanticLabel = catalogCostLabel == null
        ? label
        : '$label, $catalogCostLabel';
    return Semantics(
      button: onTap != null,
      label: onTap == null
          ? semanticLabel
          : '$semanticLabel, ${l10n.statisticsOpenConversation}',
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(6),
        child: Padding(
          padding: const EdgeInsets.symmetric(vertical: 9, horizontal: 4),
          child: Row(
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      item.title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(color: palette.text, fontSize: 13),
                    ),
                    const SizedBox(height: 3),
                    Text(
                      '$date · ${numberFormat.format(item.requests)} ${l10n.statisticsRequests}',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: palette.secondaryText,
                        fontSize: 11,
                      ),
                    ),
                    if (catalogCostLabel != null) ...[
                      const SizedBox(height: 3),
                      Text(
                        catalogCostLabel,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          color: palette.secondaryText,
                          fontSize: 11,
                        ),
                      ),
                    ],
                  ],
                ),
              ),
              const SizedBox(width: 10),
              Text(
                numberFormat.format(
                  item.inputTokens.saturatingAdd(item.outputTokens),
                ),
                style: TextStyle(
                  color: palette.text,
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                  fontFeatures: const [FontFeature.tabularFigures()],
                ),
              ),
              if (onTap != null) ...[
                const SizedBox(width: 4),
                Tooltip(
                  message: l10n.statisticsOpenConversation,
                  child: Icon(
                    LucideIcons.chevronRight,
                    size: 16,
                    color: palette.secondaryIcon,
                  ),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

class _QuotaHistorySnapshotCard extends StatelessWidget {
  const _QuotaHistorySnapshotCard({
    required this.snapshot,
    required this.dateLabel,
    required this.l10n,
    required this.palette,
    required this.numberFormat,
    required this.percentFormat,
  });

  final UsageQuotaSnapshot snapshot;
  final String dateLabel;
  final AppLocalizations l10n;
  final OpenChatPalette palette;
  final NumberFormat numberFormat;
  final NumberFormat percentFormat;

  @override
  Widget build(BuildContext context) {
    final account = snapshot.connectionName?.trim();
    final workspace = snapshot.workspaceName?.trim();
    final resetCreditCount = snapshot.resetCreditCount;
    final statusLabel = switch (snapshot.ordinaryUsageAllowed) {
      true => l10n.statisticsQuotaAllowed,
      false => l10n.statisticsQuotaBlocked,
      null => l10n.statisticsQuotaUnknown,
    };
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Wrap(
          spacing: 8,
          runSpacing: 4,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            Text(
              dateLabel,
              style: TextStyle(
                color: palette.text,
                fontSize: 12,
                fontWeight: FontWeight.w600,
              ),
            ),
            if (account != null && account.isNotEmpty)
              Text(
                account,
                style: TextStyle(color: palette.secondaryText, fontSize: 11),
              ),
            if (workspace != null && workspace.isNotEmpty)
              Text(
                workspace,
                style: TextStyle(color: palette.secondaryText, fontSize: 11),
              ),
            Text(switch (snapshot.freshness) {
              'current' => l10n.statisticsQuotaFreshnessCurrent,
              'stale' => l10n.statisticsQuotaFreshnessStale,
              _ => l10n.statisticsQuotaFreshnessUnknown,
            }, style: TextStyle(color: palette.secondaryText, fontSize: 11)),
            Text(
              statusLabel,
              style: TextStyle(color: palette.secondaryText, fontSize: 11),
            ),
            if (resetCreditCount != null)
              Text(
                l10n.resetCreditsAvailable(resetCreditCount),
                style: TextStyle(color: palette.secondaryText, fontSize: 11),
              ),
          ],
        ),
        const SizedBox(height: 8),
        if (snapshot.buckets.isEmpty)
          Text(
            l10n.statisticsNotReported,
            style: TextStyle(color: palette.secondaryText, fontSize: 12),
          )
        else
          for (var index = 0; index < snapshot.buckets.length; index++) ...[
            if (index > 0) const SizedBox(height: 8),
            _QuotaHistoryBucket(
              bucket: snapshot.buckets[index],
              l10n: l10n,
              palette: palette,
              numberFormat: numberFormat,
              percentFormat: percentFormat,
            ),
          ],
      ],
    );
  }
}

class _QuotaHistoryBucket extends StatelessWidget {
  const _QuotaHistoryBucket({
    required this.bucket,
    required this.l10n,
    required this.palette,
    required this.numberFormat,
    required this.percentFormat,
  });

  final UsageQuotaBucket bucket;
  final AppLocalizations l10n;
  final OpenChatPalette palette;
  final NumberFormat numberFormat;
  final NumberFormat percentFormat;

  @override
  Widget build(BuildContext context) {
    final percent = bucket.usedPercent;
    final labelBucket = ChatGptUsageBucket(
      limitId: bucket.limitId,
      windowSeconds: bucket.windowSeconds,
    );
    final reset = bucket.resetAtUnixMs;
    final resetLabel = reset == null
        ? null
        : DateFormat.yMMMd(Localizations.localeOf(context).toString())
              .add_jm()
              .format(DateTime.fromMillisecondsSinceEpoch(reset).toLocal());
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            Expanded(
              child: Text(
                chatGptUsageBucketLabel(labelBucket, l10n),
                style: TextStyle(color: palette.text, fontSize: 12),
              ),
            ),
            Text(
              percent == null
                  ? l10n.statisticsNotReported
                  : percentFormat.format((percent / 100).clamp(0.0, 1.0)),
              style: TextStyle(
                color: palette.secondaryText,
                fontSize: 11,
                fontFeatures: const [FontFeature.tabularFigures()],
              ),
            ),
          ],
        ),
        const SizedBox(height: 4),
        if (percent != null)
          ClipRRect(
            borderRadius: BorderRadius.circular(3),
            child: LinearProgressIndicator(
              value: (percent / 100).clamp(0.0, 1.0),
              minHeight: 4,
              backgroundColor: theme.colorScheme.surfaceContainerHighest,
              color: palette.accent,
            ),
          ),
        if (resetLabel != null) ...[
          const SizedBox(height: 3),
          Text(
            '${l10n.statisticsResetAt}: $resetLabel',
            style: TextStyle(color: palette.secondaryText, fontSize: 10),
          ),
        ],
      ],
    );
  }
}

class _UsageRequestDetail extends StatelessWidget {
  const _UsageRequestDetail({
    required this.request,
    required this.dateLabel,
    required this.numberFormat,
    required this.l10n,
    required this.palette,
  });

  final UsageStatisticsRequest request;
  final String dateLabel;
  final NumberFormat numberFormat;
  final AppLocalizations l10n;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final providerId = request.providerId;
    final reasoningEffort = request.reasoningEffort;
    final fastRequested = request.fastRequested;
    final serviceTier = request.serviceTier;
    final provider = providerId == null
        ? l10n.statisticsNotReported
        : _providerLabel(providerId, l10n);
    final model = request.modelId ?? l10n.statisticsNotReported;
    final title = request.conversationTitle.trim().isEmpty
        ? l10n.statisticsNotReported
        : request.conversationTitle;
    final status = _statusLabel(request, l10n);
    final tokens = <String>[
      '${l10n.statisticsInputTokens}: ${_count(request.inputTokens)}',
      '${l10n.statisticsOutputTokens}: ${_count(request.outputTokens)}',
      '${l10n.statisticsReasoningTokens}: ${_count(request.reasoningTokens)}',
      '${l10n.statisticsCachedInputTokens}: ${_count(request.cachedInputTokens)}',
      '${l10n.statisticsCacheWriteTokens}: ${_count(request.cacheWriteTokens)}',
    ];
    final metadata = <String>[
      provider,
      model,
      _operationLabel(request.operation, l10n),
      '${l10n.statisticsReasoningEffort}: ${_reasoningLabel(reasoningEffort ?? 'unspecified', l10n)}',
      '${l10n.statisticsFastModeFilter}: ${switch (fastRequested) {
        true => l10n.statisticsRequested,
        false => l10n.statisticsNotRequested,
        null => l10n.statisticsNotReported,
      }}',
      '${l10n.statisticsServiceTier}: ${serviceTier ?? l10n.statisticsNotReported}',
      if (request.runId case final runId?) l10n.statisticsRunIdValue(runId),
    ];
    return Semantics(
      container: true,
      label:
          '$title, $dateLabel, $provider, $model, $status, ${metadata.join(', ')}, ${tokens.join(', ')}',
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Wrap(
            spacing: 8,
            runSpacing: 5,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              Text(
                title,
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  color: palette.text,
                  fontSize: 13,
                  fontWeight: FontWeight.w600,
                ),
              ),
              _StatusLabel(
                label: status,
                status: request.status,
                palette: palette,
              ),
            ],
          ),
          const SizedBox(height: 4),
          Text(
            '$dateLabel · ${metadata.join(' · ')}',
            maxLines: 3,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(color: palette.secondaryText, fontSize: 10),
          ),
          const SizedBox(height: 5),
          Wrap(
            spacing: 10,
            runSpacing: 4,
            children: [
              for (final token in tokens)
                Text(
                  token,
                  style: TextStyle(
                    color: palette.text,
                    fontSize: 11,
                    fontFeatures: const [FontFeature.tabularFigures()],
                  ),
                ),
              if (request.providerReportedCostUsd case final cost?)
                Text(
                  '${l10n.statisticsProviderReportedCost}: ${_formatCostUsd(cost, Localizations.localeOf(context).toString())}',
                  style: TextStyle(color: palette.secondaryText, fontSize: 11),
                ),
              if (request.modelsDevCatalogCostUsd case final cost?)
                Text(
                  '${l10n.statisticsModelsDevCatalogCost}: ${_formatCostUsd(cost, Localizations.localeOf(context).toString())}',
                  style: TextStyle(color: palette.secondaryText, fontSize: 11),
                ),
            ],
          ),
          const SizedBox(height: 5),
          _UsageRequestManifestView(
            manifest: request.requestManifest,
            l10n: l10n,
            palette: palette,
          ),
        ],
      ),
    );
  }

  String _count(int? value) =>
      value == null ? l10n.statisticsNotReported : numberFormat.format(value);
}

class _UsageRequestManifestView extends StatelessWidget {
  const _UsageRequestManifestView({
    required this.manifest,
    required this.l10n,
    required this.palette,
  });

  final UsageRequestManifest? manifest;
  final AppLocalizations l10n;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) => ExpansionTile(
    tilePadding: EdgeInsets.zero,
    childrenPadding: const EdgeInsets.only(bottom: 6),
    visualDensity: VisualDensity.compact,
    title: Text(
      l10n.statisticsRequestPayload,
      style: TextStyle(color: palette.text, fontSize: 12),
    ),
    children: [
      if (manifest case final requestManifest?
          when requestManifest.available) ...[
        _ManifestLine(
          label: l10n.statisticsRequestMessageCount(
            requestManifest.messageCount,
          ),
          palette: palette,
        ),
        if (requestManifest.messageRoles.isNotEmpty)
          _ManifestLine(
            label: l10n.statisticsRequestRoles(
              requestManifest.messageRoles.entries
                  .map((entry) => '${entry.key}: ${entry.value}')
                  .join(', '),
            ),
            palette: palette,
          ),
        if (requestManifest.sourceMessageIds.isNotEmpty)
          _ManifestLine(
            label: l10n.statisticsRequestSourceMessages(
              _manifestIds(requestManifest.sourceMessageIds),
            ),
            palette: palette,
          ),
        if (requestManifest.archivedMessageIds.isNotEmpty)
          _ManifestLine(
            label: l10n.statisticsRequestArchivedMessages(
              _manifestIds(requestManifest.archivedMessageIds),
            ),
            palette: palette,
          ),
        if (requestManifest.summarizedThroughMessageId case final boundaryId?)
          _ManifestLine(
            label: l10n.statisticsRequestSummaryBoundary(boundaryId),
            palette: palette,
          ),
        if (requestManifest.sourceAttachments.isNotEmpty)
          _ManifestLine(
            label: l10n.statisticsRequestSourceAttachments(
              requestManifest.sourceAttachments
                  .take(8)
                  .map(
                    (attachment) =>
                        '${attachment.name} (${attachment.mimeType}; ${attachment.id})',
                  )
                  .join(', '),
            ),
            palette: palette,
          ),
        if (requestManifest.sourceDetailsTruncated)
          _ManifestLine(
            label: l10n.statisticsRequestSourcesTruncated,
            palette: palette,
          ),
        _ManifestLine(
          label: l10n.statisticsRequestImageCount(requestManifest.imageCount),
          palette: palette,
        ),
        _ManifestLine(
          label: l10n.statisticsRequestToolResultCount(
            requestManifest.toolResultCount,
          ),
          palette: palette,
        ),
        _ManifestLine(
          label: l10n.statisticsRequestInstructionBytes(
            requestManifest.instructionBytes,
          ),
          palette: palette,
        ),
        _ManifestLine(
          label: l10n.statisticsRequestTools(
            requestManifest.toolDefinitions.isEmpty
                ? l10n.statisticsNone
                : requestManifest.toolDefinitions.join(', '),
          ),
          palette: palette,
        ),
        _ManifestLine(
          label: l10n.statisticsRequestCacheControls(
            requestManifest.cacheControls.isEmpty
                ? l10n.statisticsNone
                : requestManifest.cacheControls.join(', '),
          ),
          palette: palette,
        ),
      ] else
        Align(
          alignment: Alignment.centerLeft,
          child: Text(
            l10n.statisticsRequestContextUnavailable,
            style: TextStyle(color: palette.secondaryText, fontSize: 11),
          ),
        ),
    ],
  );

  String _manifestIds(List<String> ids) {
    final visible = ids.take(12).join(', ');
    final remaining = ids.length - 12;
    return remaining > 0 ? '$visible, +$remaining' : visible;
  }
}

class _ManifestLine extends StatelessWidget {
  const _ManifestLine({required this.label, required this.palette});

  final String label;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) => Align(
    alignment: Alignment.centerLeft,
    child: Padding(
      padding: const EdgeInsets.only(bottom: 4),
      child: Text(
        label,
        style: TextStyle(color: palette.secondaryText, fontSize: 11),
      ),
    ),
  );
}

class _StatusLabel extends StatelessWidget {
  const _StatusLabel({
    required this.label,
    required this.status,
    required this.palette,
  });

  final String label;
  final String status;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final color = switch (status) {
      'completed' => Theme.of(context).colorScheme.tertiary,
      'failed' => Theme.of(context).colorScheme.error,
      _ => palette.secondaryText,
    };
    return Container(
      decoration: BoxDecoration(
        border: Border.all(color: color.withValues(alpha: 0.55)),
        borderRadius: BorderRadius.circular(5),
      ),
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      child: Text(label, style: TextStyle(color: color, fontSize: 10)),
    );
  }
}

String _formatCostUsd(double amount, String locale) => NumberFormat.currency(
  locale: locale,
  name: 'USD',
  symbol: r'$ ',
  decimalDigits: amount.abs() < 0.01 ? 6 : 4,
).format(amount);

String _modelsDevPricingStatus(
  UsageStatisticsPricingCatalog pricing,
  AppLocalizations l10n,
  String locale,
) {
  final fetchedAt = pricing.fetchedAtUnixMs;
  if (pricing.state == 'unavailable' || fetchedAt == null) {
    return l10n.statisticsModelsDevPricingUnavailable;
  }
  final formattedDate = DateFormat.yMMMd(locale)
      .add_jm()
      .format(DateTime.fromMillisecondsSinceEpoch(fetchedAt).toLocal());
  return switch (pricing.state) {
    'current' => l10n.statisticsModelsDevPricingCurrent(formattedDate),
    'stale' => l10n.statisticsModelsDevPricingStale(formattedDate),
    _ => l10n.statisticsModelsDevPricingUnavailable,
  };
}

String _providerLabel(String providerId, AppLocalizations l10n) =>
    switch (providerId) {
      'chatgpt' => l10n.statisticsChatGptOAuth,
      'chatgpt_api' => l10n.statisticsChatGptApi,
      'openrouter' => 'OpenRouter',
      'opencode' => 'OpenCode',
      'openai' => 'OpenAI',
      'anthropic' => 'Anthropic',
      'google' => 'Google',
      'mistral' => 'Mistral',
      'cerebras' => 'Cerebras',
      'llama_cpp' => 'llama.cpp',
      'vllm' => 'vLLM',
      _ => providerId,
    };

String _operationLabel(String operation, AppLocalizations l10n) =>
    switch (operation) {
      'chat' => l10n.statisticsOperationChat,
      'tool_follow_up' => l10n.statisticsOperationToolFollowUp,
      'compaction' => l10n.statisticsOperationCompaction,
      'title_generation' => l10n.statisticsOperationTitleGeneration,
      'legacy' => l10n.statisticsOperationLegacy,
      _ => operation,
    };

String _reasoningLabel(String effort, AppLocalizations l10n) =>
    effort == 'unspecified' ? l10n.statisticsUnspecified : effort;

String _statusLabel(UsageStatisticsRequest request, AppLocalizations l10n) {
  if (request.eventKind == 'legacy_output') return l10n.statisticsStatusLegacy;
  return switch (request.status) {
    'completed' => l10n.statisticsStatusCompleted,
    'failed' => l10n.statisticsStatusFailed,
    'cancelled' => l10n.statisticsStatusCancelled,
    'interrupted' => l10n.statisticsStatusInterrupted,
    'pending' => l10n.statisticsStatusPending,
    _ => l10n.statisticsNotReported,
  };
}

extension on int {
  int saturatingAdd(int other) {
    if (other > 0 && this > 0x7fffffffffffffff - other) {
      return 0x7fffffffffffffff;
    }
    return this + other;
  }
}
