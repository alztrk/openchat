import 'dart:async';
import 'dart:convert';
import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/semantics.dart' show SemanticsProperties;
import 'package:flutter/services.dart';
import 'package:intl/intl.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/openchat_toast.dart';
import 'package:openchat/app/openchat_dropdown.dart';
import 'package:openchat/features/settings/domain/usage_statistics.dart';
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
  bool _showMonthlyTrend = false;
  bool _isRefreshing = false;
  bool _isExporting = false;
  int _loadGeneration = 0;

  static const _statisticsLimit = 1;
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
        params: _queryParams(offset: 0, limit: _statisticsLimit),
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
    });
    unawaited(_load());
  }

  void _setFilter(void Function() update) {
    if (_isExporting) return;
    setState(() {
      update();
    });
    unawaited(_load());
  }

  void _clearFilters() {
    _setFilter(() {
      _providerId = null;
      _modelId = null;
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

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _buildHeader(statistics, l10n, palette),
        if (_isRefreshing) ...[
          const SizedBox(height: 10),
          const LinearProgressIndicator(minHeight: 2),
        ],
        if (_loadState == _StatisticsLoadState.failed) ...[
          const SizedBox(height: 10),
          _buildInlineError(l10n, palette),
        ],
        const SizedBox(height: 18),
        _buildOverview(statistics.summary, l10n, palette),
        const SizedBox(height: 20),
        _buildTrend(statistics, l10n, palette),
        const SizedBox(height: 14),
        _buildBreakdowns(statistics, l10n, palette),
        const SizedBox(height: 20),
        _buildConversations(statistics, l10n, palette),
        const SizedBox(height: 20),
      ],
    );
  }

  Widget _buildHeader(
    UsageStatistics statistics,
    AppLocalizations l10n,
    OpenChatPalette palette,
  ) {
    final locale = Localizations.localeOf(context).toString();
    final dateFormat = DateFormat.yMMMd(locale);
    final dateLabel =
        '${dateFormat.format(_dateRange.start)} - ${dateFormat.format(_dateRange.end)}';
    final activeFilters =
        (_providerId == null ? 0 : 1) + (_modelId == null ? 0 : 1);
    final dateItem = MenuItemButton(
      leadingIcon: const Icon(LucideIcons.calendarDays, size: 16),
      onPressed: _isExporting ? null : () => unawaited(_chooseDateRange()),
      child: Text(dateLabel),
    );
    final providerItems = <Widget>[
      MenuItemButton(
        trailingIcon: _providerId == null
            ? const Icon(LucideIcons.check, size: 15)
            : null,
        onPressed: _isExporting
            ? null
            : () => _setFilter(() => _providerId = null),
        child: Text(l10n.statisticsAll),
      ),
      for (final provider in statistics.filterOptions.providers)
        MenuItemButton(
          trailingIcon: _providerId == provider
              ? const Icon(LucideIcons.check, size: 15)
              : null,
          onPressed: _isExporting
              ? null
              : () => _setFilter(() => _providerId = provider),
          child: Text(_providerLabel(provider, l10n)),
        ),
    ];
    final modelItems = <Widget>[
      MenuItemButton(
        trailingIcon: _modelId == null
            ? const Icon(LucideIcons.check, size: 15)
            : null,
        onPressed: _isExporting
            ? null
            : () => _setFilter(() => _modelId = null),
        child: Text(l10n.statisticsAll),
      ),
      for (final model in statistics.filterOptions.models)
        MenuItemButton(
          trailingIcon: _modelId == model
              ? const Icon(LucideIcons.check, size: 15)
              : null,
          onPressed: _isExporting
              ? null
              : () => _setFilter(() => _modelId = model),
          child: Text(model),
        ),
    ];
    final actions = Wrap(
      spacing: 8,
      runSpacing: 8,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        OpenChatDropdown(
          palette: palette,
          menuChildren: [
            dateItem,
            SubmenuButton(
              menuChildren: providerItems,
              child: Text(l10n.statisticsProvider),
            ),
            SubmenuButton(
              menuChildren: modelItems,
              child: Text(l10n.statisticsModel),
            ),
            if (activeFilters > 0)
              MenuItemButton(
                leadingIcon: const Icon(LucideIcons.x, size: 15),
                onPressed: _isExporting ? null : _clearFilters,
                child: Text(l10n.statisticsClearFilters),
              ),
          ],
          builder: (context, controller, child, focusNode) =>
              OutlinedButton.icon(
                focusNode: focusNode,
                onPressed: _isExporting
                    ? null
                    : () => controller.isOpen
                          ? controller.close()
                          : controller.open(),
                icon: const Icon(LucideIcons.listFilter, size: 16),
                label: Text(
                  activeFilters == 0
                      ? l10n.statisticsFilters
                      : '${l10n.statisticsFilters} · $activeFilters',
                ),
              ),
        ),
        IconButton(
          tooltip: l10n.statisticsRetry,
          onPressed: _isRefreshing || _isExporting
              ? null
              : () => unawaited(_load()),
          icon: _isRefreshing
              ? const SizedBox.square(
                  dimension: 16,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Icon(LucideIcons.refreshCw, size: 17),
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
            _isExporting ? l10n.statisticsExporting : l10n.statisticsExportCsv,
          ),
        ),
      ],
    );
    final title = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(l10n.statistics, style: Theme.of(context).textTheme.titleMedium),
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

    return LayoutBuilder(
      builder: (context, constraints) {
        if (constraints.maxWidth < 680) {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              title,
              const SizedBox(height: 12),
              Align(alignment: Alignment.centerRight, child: actions),
            ],
          );
        }
        return Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(child: title),
            const SizedBox(width: 16),
            actions,
          ],
        );
      },
    );
  }

  Widget _buildOverview(
    UsageStatisticsSummary summary,
    AppLocalizations l10n,
    OpenChatPalette palette,
  ) {
    final locale = Localizations.localeOf(context).toString();
    final number = NumberFormat.decimalPattern(locale);
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
        icon: LucideIcons.messagesSquare,
      ),
    ];
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        LayoutBuilder(
          builder: (context, constraints) {
            final columns = constraints.maxWidth >= 560 ? 3 : 1;
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
          ],
        );
      },
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
    this.detail,
    required this.icon,
  });

  final String label;
  final String value;
  final String? detail;
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
            if (data.detail case final detail?) ...[
              const SizedBox(height: 4),
              Text(
                detail,
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  color: palette.secondaryText,
                  fontSize: OpenChatTypography.metadata,
                ),
              ),
            ],
          ],
        ),
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
    final colors = Theme.of(context).colorScheme;
    final locale = Localizations.localeOf(context).toString();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        SizedBox(
          height: 190,
          child: CustomPaint(
            painter: _UsageTrendLinePainter(
              points: points,
              monthly: monthly,
              locale: locale,
              inputColor: palette.accent,
              outputColor: colors.tertiary,
              gridColor: palette.border,
              labelColor: palette.secondaryText,
              inputLabel: l10n.statisticsInputTokens,
              outputLabel: l10n.statisticsOutputTokens,
              requestLabel: l10n.statisticsRequests,
            ),
            child: const SizedBox.expand(),
          ),
        ),
        const SizedBox(height: 10),
        Wrap(
          spacing: 16,
          runSpacing: 8,
          children: [
            _TrendLegend(
              color: palette.accent,
              label: l10n.statisticsInputTokens,
            ),
            _TrendLegend(
              color: colors.tertiary,
              label: l10n.statisticsOutputTokens,
            ),
          ],
        ),
      ],
    );
  }
}

class _UsageTrendLinePainter extends CustomPainter {
  const _UsageTrendLinePainter({
    required this.points,
    required this.monthly,
    required this.locale,
    required this.inputColor,
    required this.outputColor,
    required this.gridColor,
    required this.labelColor,
    required this.inputLabel,
    required this.outputLabel,
    required this.requestLabel,
  });

  final List<UsageStatisticsTrend> points;
  final bool monthly;
  final String locale;
  final Color inputColor;
  final Color outputColor;
  final Color gridColor;
  final Color labelColor;
  final String inputLabel;
  final String outputLabel;
  final String requestLabel;

  static const _left = 48.0;
  static const _right = 8.0;
  static const _top = 10.0;
  static const _bottom = 30.0;

  Rect _plotRect(Size size) => Rect.fromLTRB(
    _left,
    _top,
    math.max(_left + 1, size.width - _right),
    math.max(_top + 1, size.height - _bottom),
  );

  double _maxTokens() => points
      .fold<int>(
        0,
        (maximum, point) =>
            math.max(maximum, math.max(point.inputTokens, point.outputTokens)),
      )
      .toDouble();

  double _xForIndex(int index, Rect plot) => points.length == 1
      ? plot.center.dx
      : plot.left + plot.width * index / (points.length - 1);

  double _yForValue(int value, Rect plot, double maximum) =>
      plot.bottom - plot.height * value / (maximum == 0 ? 1 : maximum);

  String _dateLabel(String bucket) {
    final date = DateTime.tryParse(monthly ? '$bucket-01' : bucket);
    if (date == null) return bucket;
    return monthly
        ? DateFormat.MMM(locale).format(date)
        : DateFormat.Md(locale).format(date);
  }

  @override
  void paint(Canvas canvas, Size size) {
    if (points.isEmpty) return;
    final plot = _plotRect(size);
    final maximum = _maxTokens();
    final number = NumberFormat.compact(locale: locale);
    final gridPaint = Paint()
      ..color = gridColor.withValues(alpha: 0.42)
      ..strokeWidth = 1;

    for (var tick = 0; tick <= 4; tick++) {
      final fraction = tick / 4;
      final y = plot.bottom - plot.height * fraction;
      canvas.drawLine(Offset(plot.left, y), Offset(plot.right, y), gridPaint);
      final label = TextPainter(
        text: TextSpan(
          text: number.format(maximum * fraction),
          style: TextStyle(color: labelColor, fontSize: 10),
        ),
        textDirection: ui.TextDirection.ltr,
      )..layout(maxWidth: _left - 8);
      label.paint(
        canvas,
        Offset(_left - 8 - label.width, y - label.height / 2),
      );
    }

    _drawSeries(
      canvas,
      plot,
      maximum,
      inputColor,
      (point) => point.inputTokens,
    );
    _drawSeries(
      canvas,
      plot,
      maximum,
      outputColor,
      (point) => point.outputTokens,
    );

    final step = math.max(1, (points.length / 6).ceil());
    for (var index = 0; index < points.length; index++) {
      if (index % step != 0 && index != points.length - 1) continue;
      final label = TextPainter(
        text: TextSpan(
          text: _dateLabel(points[index].bucket),
          style: TextStyle(color: labelColor, fontSize: 10),
        ),
        textDirection: ui.TextDirection.ltr,
      )..layout();
      final x = _xForIndex(index, plot);
      final labelX = (x - label.width / 2)
          .clamp(plot.left, math.max(plot.left, plot.right - label.width))
          .toDouble();
      label.paint(canvas, Offset(labelX, plot.bottom + 8));
    }
  }

  void _drawSeries(
    Canvas canvas,
    Rect plot,
    double maximum,
    Color color,
    int Function(UsageStatisticsTrend) valueFor,
  ) {
    final seriesPaint = Paint()
      ..color = color
      ..strokeWidth = 2
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round
      ..style = PaintingStyle.stroke;
    final markerPaint = Paint()
      ..color = color
      ..style = PaintingStyle.fill;
    final path = Path();
    for (var index = 0; index < points.length; index++) {
      final point = Offset(
        _xForIndex(index, plot),
        _yForValue(valueFor(points[index]), plot, maximum),
      );
      if (index == 0) {
        path.moveTo(point.dx, point.dy);
      } else {
        path.lineTo(point.dx, point.dy);
      }
      canvas.drawCircle(point, 2.5, markerPaint);
    }
    if (points.length > 1) canvas.drawPath(path, seriesPaint);
  }

  @override
  bool shouldRepaint(covariant _UsageTrendLinePainter oldDelegate) =>
      oldDelegate.points != points ||
      oldDelegate.monthly != monthly ||
      oldDelegate.locale != locale ||
      oldDelegate.inputColor != inputColor ||
      oldDelegate.outputColor != outputColor ||
      oldDelegate.gridColor != gridColor ||
      oldDelegate.labelColor != labelColor;

  @override
  SemanticsBuilderCallback get semanticsBuilder => (size) {
    if (points.isEmpty) return const <CustomPainterSemantics>[];
    final plot = _plotRect(size);
    final number = NumberFormat.decimalPattern(locale);
    final pointWidth = math.max(24.0, plot.width / points.length);
    return [
      for (var index = 0; index < points.length; index++)
        CustomPainterSemantics(
          rect: Rect.fromLTWH(
            (_xForIndex(index, plot) - pointWidth / 2).clamp(
              plot.left,
              math.max(plot.left, plot.right - pointWidth),
            ),
            0,
            pointWidth,
            size.height,
          ),
          properties: SemanticsProperties(
            textDirection: ui.TextDirection.ltr,
            label:
                '${_dateLabel(points[index].bucket)}, '
                '${number.format(points[index].inputTokens)} $inputLabel, '
                '${number.format(points[index].outputTokens)} $outputLabel, '
                '${number.format(points[index].requests)} $requestLabel',
          ),
        ),
    ];
  };

  @override
  bool shouldRebuildSemantics(covariant _UsageTrendLinePainter oldDelegate) =>
      shouldRepaint(oldDelegate) ||
      oldDelegate.inputLabel != inputLabel ||
      oldDelegate.outputLabel != outputLabel ||
      oldDelegate.requestLabel != requestLabel;
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
          style: TextStyle(
            color: palette.secondaryText,
            fontSize: OpenChatTypography.metadata,
          ),
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
  });

  final String title;
  final List<UsageStatisticsBreakdown> items;
  final OpenChatPalette palette;
  final String emptyLabel;
  final String Function(String key) labelFor;

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
  });

  final UsageStatisticsBreakdown item;
  final String label;
  final int maxTokens;
  final OpenChatPalette palette;

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
                  maxLines: 2,
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
            style: TextStyle(
              color: palette.secondaryText,
              fontSize: OpenChatTypography.metadata,
            ),
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
        borderRadius: BorderRadius.circular(OpenChatRadii.button),
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
                        fontSize: OpenChatTypography.metadata,
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
                          fontSize: OpenChatTypography.metadata,
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

String _formatCostUsd(double amount, String locale) => NumberFormat.currency(
  locale: locale,
  name: 'USD',
  symbol: r'$ ',
  decimalDigits: amount.abs() < 0.01 ? 6 : 4,
).format(amount);

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
