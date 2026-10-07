import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:intl/intl.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/data/conversation_memory_repository.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/domain/chat_attachment.dart';
import 'package:openchat/features/chat/domain/conversation_memory.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

class ContextUsageIndicator extends StatefulWidget {
  const ContextUsageIndicator({
    required this.controller,
    required this.messages,
    required this.providerId,
    required this.modelId,
    required this.supportsTools,
    required this.contextWindow,
    required this.repository,
    required this.conversationId,
    required this.isSending,
    required this.settingsPreferences,
    required this.toolPermissionMode,
    this.connectionId,
    this.workspaceId,
    this.pendingAttachments = const <ChatAttachment>[],
    super.key,
  });

  final TextEditingController controller;
  final List<ChatMessage> messages;
  final String? providerId;
  final String? modelId;
  final bool? supportsTools;
  final int? contextWindow;
  final ConversationMemoryRepository? repository;
  final String? conversationId;
  final bool isSending;
  final SettingsPreferences? settingsPreferences;
  final ToolPermissionMode toolPermissionMode;
  final String? connectionId;
  final String? workspaceId;
  final List<ChatAttachment> pendingAttachments;

  @override
  State<ContextUsageIndicator> createState() => _ContextUsageIndicatorState();
}

class _ContextUsageIndicatorState extends State<ContextUsageIndicator> {
  final GlobalKey<TooltipState> _tooltipKey = GlobalKey<TooltipState>();
  ConversationMemoryState? _memoryState;
  ContextUsageConfiguration? _usageConfiguration;
  String _sharedInstructions = '';
  int _inspectionGeneration = 0;
  int _configurationGeneration = 0;
  bool _inspectionFailed = false;
  bool _instructionsFailed = false;
  bool _configurationLoading = false;
  bool _configurationFailed = false;

  @override
  void initState() {
    super.initState();
    _loadMemoryState();
    _loadSharedInstructions();
  }

  @override
  void didUpdateWidget(covariant ContextUsageIndicator oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.repository != widget.repository ||
        oldWidget.conversationId != widget.conversationId) {
      _memoryState = null;
      _inspectionFailed = false;
      _loadMemoryState();
    } else if (oldWidget.isSending && !widget.isSending) {
      _loadMemoryState();
    }
    final usageRouteChanged =
        oldWidget.repository != widget.repository ||
        oldWidget.conversationId != widget.conversationId ||
        oldWidget.providerId != widget.providerId ||
        oldWidget.modelId != widget.modelId ||
        oldWidget.supportsTools != widget.supportsTools ||
        oldWidget.toolPermissionMode != widget.toolPermissionMode;
    if (oldWidget.settingsPreferences != widget.settingsPreferences) {
      _sharedInstructions = '';
      _instructionsFailed = false;
      _loadSharedInstructions();
    } else if (usageRouteChanged) {
      _usageConfiguration = null;
      _configurationFailed = false;
      _loadUsageConfiguration(_sharedInstructions);
    }
  }

  Future<void> _loadSharedInstructions() async {
    final preferences = widget.settingsPreferences;
    if (preferences == null) {
      _loadUsageConfiguration('');
      return;
    }
    try {
      final instructions = await preferences.readSharedInstructions();
      if (!mounted || preferences != widget.settingsPreferences) return;
      setState(() {
        _sharedInstructions = instructions;
        _instructionsFailed = false;
      });
      _loadUsageConfiguration(instructions);
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'context_usage',
          context: ErrorDescription('while loading shared instructions'),
        ),
      );
      if (!mounted || preferences != widget.settingsPreferences) return;
      setState(() => _instructionsFailed = true);
      _loadUsageConfiguration('');
    }
  }

  Future<void> _loadUsageConfiguration(String customInstructions) async {
    final repository = widget.repository;
    final providerId = widget.providerId;
    final generation = ++_configurationGeneration;
    if (providerId == null) {
      if (!mounted) return;
      setState(() {
        _usageConfiguration = null;
        _configurationLoading = false;
        _configurationFailed = false;
      });
      return;
    }
    if (repository == null) {
      if (!mounted) return;
      setState(() {
        _usageConfiguration = null;
        _configurationLoading = false;
        _configurationFailed = true;
      });
      return;
    }

    setState(() {
      _usageConfiguration = null;
      _configurationLoading = true;
      _configurationFailed = false;
    });
    try {
      final configuration = await repository.estimateContextUsage(
        providerId: providerId,
        modelId: widget.modelId,
        supportsTools: widget.supportsTools,
        toolPermissionMode: widget.toolPermissionMode.serviceValue,
        customInstructions: customInstructions,
        conversationId: widget.conversationId,
      );
      if (!mounted || generation != _configurationGeneration) return;
      setState(() {
        _usageConfiguration = configuration;
        _configurationLoading = false;
      });
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'context_usage',
          context: ErrorDescription('while estimating prompt configuration'),
        ),
      );
      if (!mounted || generation != _configurationGeneration) return;
      setState(() {
        _usageConfiguration = null;
        _configurationLoading = false;
        _configurationFailed = true;
      });
    }
  }

  Future<void> _loadMemoryState() async {
    final repository = widget.repository;
    final conversationId = widget.conversationId;
    final generation = ++_inspectionGeneration;
    if (repository == null || conversationId == null) return;
    try {
      final state = await repository.inspect(conversationId);
      if (!mounted || generation != _inspectionGeneration) return;
      setState(() => _memoryState = state);
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'context_usage',
          context: ErrorDescription('while loading context usage details'),
        ),
      );
      if (!mounted || generation != _inspectionGeneration) return;
      setState(() => _inspectionFailed = true);
    }
  }

  _ContextUsageSnapshot _snapshot(String draft) {
    final memory = _memoryState;
    final prompt = memory?.lastPrompt;
    final matchingPrompt =
        prompt != null &&
        prompt.providerId == widget.providerId &&
        prompt.modelId == widget.modelId &&
        prompt.connectionId == widget.connectionId &&
        prompt.workspaceId == widget.workspaceId;
    final promptMessageIndex = matchingPrompt && prompt.messageId != null
        ? widget.messages.indexWhere(
            (message) => message.id == prompt.messageId,
          )
        : -1;
    final draftTokens = _estimateText(draft);
    final compactionMatchesRoute =
        memory?.compactionKind != null &&
        memory?.compactionProviderId == widget.providerId &&
        memory?.compactionModelId == widget.modelId &&
        memory?.compactionConnectionId == widget.connectionId &&
        memory?.compactionWorkspaceId == widget.workspaceId;
    final boundaryIndex =
        compactionMatchesRoute && memory?.compactedThroughMessageId != null
        ? widget.messages.indexWhere(
            (message) => message.id == memory!.compactedThroughMessageId,
          )
        : -1;
    final contextMessages = widget.messages.skip(
      boundaryIndex >= 0 ? boundaryIndex + 1 : 0,
    );
    final activeMessages = _estimateMessages(contextMessages);
    final summaryTokens = switch (compactionMatchesRoute
        ? memory?.compactionKind
        : null) {
      'summary' => _estimateText(memory?.summary ?? ''),
      'responses_checkpoint' => _opaqueCheckpointEstimate,
      _ => 0,
    };

    final breakdown = _ContextUsageBreakdown(
      instructionsTokens: _usageConfiguration?.instructionsTokens ?? 0,
      toolDefinitionTokens:
          _usageConfiguration?.toolDefinitions.fold<int>(
            0,
            (sum, definition) => sum + definition.tokens,
          ) ??
          0,
      userMessageTokens: activeMessages.userTokens,
      assistantMessageTokens: activeMessages.assistantTokens,
      attachmentUsage: <_AttachmentUsageTokens>[
        ...activeMessages.attachmentUsage,
        for (final attachment in widget.pendingAttachments)
          _estimateAttachment(attachment, isDraft: true),
      ],
      toolUsage: <_ToolUsageTokens>[
        for (final entry in activeMessages.toolUsage.entries)
          _ToolUsageTokens(entry.key, entry.value),
      ],
      memoryTokens: summaryTokens,
      draftTokens: draftTokens,
    );

    if (promptMessageIndex >= 0 && prompt != null) {
      final additions = _estimateMessages(
        widget.messages.skip(promptMessageIndex + 1),
      );
      return _ContextUsageSnapshot(
        totalTokens: prompt.inputTokens + additions.totalTokens + draftTokens,
        breakdown: breakdown,
      );
    }

    return _ContextUsageSnapshot(
      totalTokens: breakdown.totalTokens,
      breakdown: breakdown,
    );
  }

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final l10n = context.openchatL10n;
    final contextWindow = widget.contextWindow;
    final percentFormat = NumberFormat.percentPattern(
      Localizations.localeOf(context).toString(),
    )..maximumFractionDigits = 2;

    return ValueListenableBuilder<TextEditingValue>(
      valueListenable: widget.controller,
      builder: (context, value, _) {
        final snapshot = _snapshot(value.text);
        final fraction = contextWindow != null && contextWindow > 0
            ? snapshot.totalTokens / contextWindow
            : null;
        final percentLabel = fraction == null
            ? null
            : percentFormat.format(fraction);
        final touch = switch (Theme.of(context).platform) {
          TargetPlatform.android ||
          TargetPlatform.iOS ||
          TargetPlatform.fuchsia => true,
          _ => false,
        };
        final buttonSize = touch ? 44.0 : 36.0;
        final focusRing = OpenChatSemanticColors.of(context).focusRing;
        final detailsUnavailable =
            _inspectionFailed || _instructionsFailed || _configurationFailed;
        final statusLabel = _configurationLoading
            ? l10n.contextUsageConfigurationLoading
            : detailsUnavailable
            ? l10n.contextUsageConfigurationUnavailable
            : percentLabel ?? l10n.contextUsageNoModelLimit;
        final barColor = fraction != null && fraction >= 0.9
            ? Theme.of(context).colorScheme.error
            : palette.accent;
        final tooltip = Tooltip(
          key: _tooltipKey,
          richMessage: _tooltipMessage(context, snapshot, contextWindow),
          excludeFromSemantics: true,
          waitDuration: const Duration(milliseconds: 400),
          showDuration: const Duration(seconds: 8),
          triggerMode: TooltipTriggerMode.tap,
          preferBelow: false,
          verticalOffset: 18,
          padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
          constraints: const BoxConstraints(maxWidth: 560),
          decoration: BoxDecoration(
            color: palette.navigation,
            border: Border.all(color: palette.border),
            borderRadius: BorderRadius.circular(10),
            boxShadow: const <BoxShadow>[
              BoxShadow(
                color: Color(0x26000000),
                blurRadius: 12,
                offset: Offset(0, 5),
              ),
            ],
          ),
          child: Semantics(
            label: '${l10n.contextUsageTitle}: $statusLabel',
            child: IconButton(
              onPressed: () => _tooltipKey.currentState?.ensureTooltipVisible(),
              padding: const EdgeInsets.all(6),
              constraints: BoxConstraints.tightFor(
                width: buttonSize,
                height: buttonSize,
              ),
              style:
                  IconButton.styleFrom(
                    foregroundColor: palette.secondaryIcon,
                    minimumSize: Size.square(buttonSize),
                    maximumSize: Size.square(buttonSize),
                    tapTargetSize: touch
                        ? MaterialTapTargetSize.padded
                        : MaterialTapTargetSize.shrinkWrap,
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(
                        OpenChatRadii.control,
                      ),
                    ),
                  ).copyWith(
                    side: WidgetStateProperty.resolveWith(
                      (states) => states.contains(WidgetState.focused)
                          ? BorderSide(color: focusRing, width: 1.4)
                          : BorderSide.none,
                    ),
                  ),
              icon: ExcludeSemantics(
                child: _configurationLoading
                    ? const SizedBox.square(
                        dimension: 20,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : detailsUnavailable
                    ? Icon(
                        widget.repository == null
                            ? LucideIcons.cloudOff
                            : LucideIcons.circleAlert,
                        size: 20,
                        color: widget.repository == null
                            ? palette.secondaryIcon
                            : Theme.of(context).colorScheme.error,
                      )
                    : percentLabel == null
                    ? const Icon(LucideIcons.chartNoAxesCombined, size: 20)
                    : Stack(
                        alignment: Alignment.center,
                        children: [
                          SizedBox.expand(
                            child: CircularProgressIndicator(
                              value: fraction?.clamp(0, 1) ?? 0,
                              strokeWidth: 3,
                              strokeCap: StrokeCap.round,
                              backgroundColor: palette.border,
                              color: barColor,
                            ),
                          ),
                          FittedBox(
                            fit: BoxFit.scaleDown,
                            child: Text(
                              percentLabel,
                              style: Theme.of(context).textTheme.labelSmall
                                  ?.copyWith(
                                    color: palette.text,
                                    fontWeight: FontWeight.w600,
                                  ),
                            ),
                          ),
                        ],
                      ),
              ),
            ),
          ),
        );

        return Focus(
          skipTraversal: true,
          onKeyEvent: (node, event) {
            if (event is! KeyDownEvent) return KeyEventResult.ignored;
            if (event.logicalKey == LogicalKeyboardKey.escape) {
              Tooltip.dismissAllToolTips();
              return KeyEventResult.handled;
            }
            return KeyEventResult.ignored;
          },
          child: tooltip,
        );
      },
    );
  }

  InlineSpan _tooltipMessage(
    BuildContext context,
    _ContextUsageSnapshot snapshot,
    int? contextWindow,
  ) {
    final tooltipContentWidth = (MediaQuery.sizeOf(context).width - 64)
        .clamp(1.0, 520.0)
        .toDouble();
    final media = MediaQuery.of(context);
    final tooltipContentHeight =
        (media.size.height -
                media.viewInsets.vertical -
                media.padding.vertical -
                96)
            .clamp(48.0, 560.0)
            .toDouble();
    return WidgetSpan(
      alignment: PlaceholderAlignment.middle,
      child: ConstrainedBox(
        constraints: BoxConstraints(
          maxWidth: tooltipContentWidth,
          maxHeight: tooltipContentHeight,
        ),
        child: SingleChildScrollView(
          child: SizedBox(
            width: tooltipContentWidth,
            child: _ContextUsagePopover(
              providerId: widget.providerId,
              modelId: widget.modelId,
              snapshot: snapshot,
              contextWindow: contextWindow,
              inspectionFailed: _inspectionFailed,
              instructionsFailed: _instructionsFailed,
              configurationLoading: _configurationLoading,
              configurationFailed: _configurationFailed,
            ),
          ),
        ),
      ),
    );
  }
}

class _ContextUsagePopover extends StatelessWidget {
  const _ContextUsagePopover({
    required this.providerId,
    required this.modelId,
    required this.snapshot,
    required this.contextWindow,
    required this.inspectionFailed,
    required this.instructionsFailed,
    required this.configurationLoading,
    required this.configurationFailed,
  });

  final String? providerId;
  final String? modelId;
  final _ContextUsageSnapshot snapshot;
  final int? contextWindow;
  final bool inspectionFailed;
  final bool instructionsFailed;
  final bool configurationLoading;
  final bool configurationFailed;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final theme = Theme.of(context);
    final locale = Localizations.localeOf(context).toString();
    final numberFormat = NumberFormat.decimalPattern(locale);
    final percentFormat = NumberFormat.percentPattern(locale)
      ..maximumFractionDigits = 2;
    final requestedWindow = contextWindow;
    final window = requestedWindow != null && requestedWindow > 0
        ? requestedWindow
        : null;
    final contextLimitLabel = window != null
        ? l10n.contextUsageModelLimit(numberFormat.format(window))
        : l10n.contextUsageNoModelLimit;
    final usageSummary = window == null
        ? l10n.contextUsageUsed(numberFormat.format(snapshot.totalTokens))
        : l10n.contextUsageSummary(
            numberFormat.format(snapshot.totalTokens),
            numberFormat.format(window),
            percentFormat.format(snapshot.totalTokens / window),
          );
    final breakdown = snapshot.breakdown.scaledTo(snapshot.totalTokens);
    final colors = <Color>[
      theme.colorScheme.primary,
      theme.colorScheme.secondary,
      theme.colorScheme.tertiary,
      theme.colorScheme.inversePrimary,
      theme.colorScheme.onSurfaceVariant,
      theme.colorScheme.primaryContainer,
      theme.colorScheme.secondaryContainer,
    ];

    String categoryPercent(int count) {
      if (window == null) return '—';
      return percentFormat.format(count / window);
    }

    final messageTokens =
        breakdown.userMessageTokens + breakdown.assistantMessageTokens;
    final toolUsageTokens = breakdown.toolUsageTokens;
    final categories = <_ContextUsageCategory>[
      _ContextUsageCategory(
        l10n.contextUsageInstructionsEstimate(
          '≈ ${numberFormat.format(breakdown.instructionsTokens)}',
          categoryPercent(breakdown.instructionsTokens),
        ),
        breakdown.instructionsTokens,
        colors[0],
      ),
      _ContextUsageCategory(
        l10n.contextUsageToolDefinitionsEstimate(
          '≈ ${numberFormat.format(breakdown.toolDefinitionTokens)}',
          categoryPercent(breakdown.toolDefinitionTokens),
        ),
        breakdown.toolDefinitionTokens,
        colors[1],
      ),
      _ContextUsageCategory(
        l10n.contextUsageMessagesEstimate(
          '≈ ${numberFormat.format(messageTokens)}',
          categoryPercent(messageTokens),
        ),
        messageTokens,
        colors[2],
        details: <_ContextUsageDetail>[
          _ContextUsageDetail(
            l10n.contextUsageUserMessagesEstimate(
              '≈ ${numberFormat.format(breakdown.userMessageTokens)}',
              categoryPercent(breakdown.userMessageTokens),
            ),
          ),
          _ContextUsageDetail(
            l10n.contextUsageAssistantMessagesEstimate(
              '≈ ${numberFormat.format(breakdown.assistantMessageTokens)}',
              categoryPercent(breakdown.assistantMessageTokens),
            ),
          ),
        ],
      ),
      _ContextUsageCategory(
        l10n.contextUsageAttachmentsEstimate(
          '≈ ${numberFormat.format(breakdown.attachmentTokens)}',
          categoryPercent(breakdown.attachmentTokens),
        ),
        breakdown.attachmentTokens,
        colors[5],
        details: <_ContextUsageDetail>[
          for (final attachment in breakdown.attachmentUsage)
            _ContextUsageDetail(
              l10n.contextUsageAttachmentEstimate(
                attachment.isDraft
                    ? l10n.contextUsageDraftAttachment(attachment.name)
                    : attachment.name,
                '≈ ${numberFormat.format(attachment.tokens)}',
                categoryPercent(attachment.tokens),
              ),
            ),
        ],
      ),
      _ContextUsageCategory(
        l10n.contextUsageToolsEstimate(
          '≈ ${numberFormat.format(toolUsageTokens)}',
          categoryPercent(toolUsageTokens),
        ),
        toolUsageTokens,
        colors[3],
        details: <_ContextUsageDetail>[
          for (final usage in breakdown.toolUsage)
            _ContextUsageDetail(
              l10n.contextUsageToolUsageEstimate(
                usage.name,
                '≈ ${numberFormat.format(usage.tokens)}',
                categoryPercent(usage.tokens),
              ),
            ),
        ],
      ),
      _ContextUsageCategory(
        l10n.contextUsageMemoryEstimate(
          '≈ ${numberFormat.format(breakdown.memoryTokens)}',
          categoryPercent(breakdown.memoryTokens),
        ),
        breakdown.memoryTokens,
        colors[4],
      ),
      _ContextUsageCategory(
        l10n.contextUsageDraftEstimate(
          '≈ ${numberFormat.format(breakdown.draftTokens)}',
          categoryPercent(breakdown.draftTokens),
        ),
        breakdown.draftTokens,
        colors[6],
      ),
    ];
    final visibleCategories = categories
        .where((category) => category.tokens > 0)
        .toList(growable: false);
    final titleStyle = theme.textTheme.labelLarge?.copyWith(
      color: palette.text,
      fontWeight: FontWeight.w700,
    );
    final detailStyle = theme.textTheme.bodySmall?.copyWith(
      color: palette.text,
      fontSize: 12,
      height: 1.4,
    );
    final mutedStyle = theme.textTheme.bodySmall?.copyWith(
      color: palette.secondaryText,
      fontSize: 11,
      height: 1.35,
    );
    final contentWidth = (MediaQuery.sizeOf(context).width - 64)
        .clamp(1.0, 520.0)
        .toDouble();
    final summary = Text(
      usageSummary,
      textAlign: TextAlign.end,
      style: detailStyle,
    );
    final modelLabel = _qualifiedModelId;
    final model = Text(
      modelLabel ?? '',
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
      style: mutedStyle,
    );

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(l10n.contextUsageTitle, style: titleStyle),
        const SizedBox(height: 4),
        Semantics(
          label: contextLimitLabel,
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(
                LucideIcons.chartNoAxesCombined,
                size: 14,
                color: palette.secondaryText,
              ),
              const SizedBox(width: 5),
              Flexible(child: Text(contextLimitLabel, style: detailStyle)),
            ],
          ),
        ),
        const SizedBox(height: 8),
        if (contentWidth < 420) ...[
          if (modelLabel != null) model,
          Align(alignment: Alignment.centerLeft, child: summary),
        ] else
          Row(
            children: [
              Expanded(child: model),
              const SizedBox(width: 8),
              Flexible(child: summary),
            ],
          ),
        const SizedBox(height: 12),
        if (window case final contextWindow?)
          _buildUsageOverview(
            context,
            contextWindow,
            snapshot,
            categories,
            visibleCategories,
            numberFormat,
            percentFormat,
            detailStyle,
            contentWidth,
          )
        else
          _buildCategoryList(visibleCategories, detailStyle, palette),
        if (window != null && snapshot.totalTokens > window)
          Padding(
            padding: const EdgeInsets.only(top: 10),
            child: Text(l10n.contextUsageOverLimit, style: titleStyle),
          ),
        if (inspectionFailed)
          Padding(
            padding: const EdgeInsets.only(top: 4),
            child: Text(
              l10n.contextUsageMeasurementUnavailable,
              style: mutedStyle,
            ),
          ),
        if (instructionsFailed)
          Padding(
            padding: const EdgeInsets.only(top: 4),
            child: Text(
              l10n.contextUsageInstructionUnavailable,
              style: mutedStyle,
            ),
          ),
        if (configurationLoading)
          Padding(
            padding: const EdgeInsets.only(top: 4),
            child: Text(
              l10n.contextUsageConfigurationLoading,
              style: mutedStyle,
            ),
          ),
        if (configurationFailed)
          Padding(
            padding: const EdgeInsets.only(top: 4),
            child: Text(
              l10n.contextUsageConfigurationUnavailable,
              style: mutedStyle,
            ),
          ),
      ],
    );
  }

  String? get _qualifiedModelId {
    final model = modelId?.trim();
    if (model == null || model.isEmpty) return null;
    final provider = providerId?.trim();
    if (provider == null ||
        provider.isEmpty ||
        model.startsWith('$provider/')) {
      return model;
    }
    return '$provider/$model';
  }

  Widget _buildUsageOverview(
    BuildContext context,
    int window,
    _ContextUsageSnapshot snapshot,
    List<_ContextUsageCategory> categories,
    List<_ContextUsageCategory> visibleCategories,
    NumberFormat numberFormat,
    NumberFormat percentFormat,
    TextStyle? detailStyle,
    double contentWidth,
  ) {
    final freeTokens = (window - snapshot.totalTokens).clamp(0, window).toInt();
    final freeSpace = _ContextUsageCategory(
      context.openchatL10n.contextUsageFreeSpaceEstimate(
        '≈ ${numberFormat.format(freeTokens)}',
        percentFormat.format(freeTokens / window),
      ),
      freeTokens,
      OpenChatPalette.of(context).secondaryText,
      isFreeSpace: true,
    );
    final palette = OpenChatPalette.of(context);
    final filledCellValue = (snapshot.totalTokens / window * 100)
        .clamp(0.0, 100.0)
        .toDouble();
    final fullCellCount = filledCellValue.floor();
    final filledCellCount = filledCellValue.ceil();
    final partialCellFill = filledCellValue - fullCellCount;
    final categoryCellCounts = _allocateProportionally(
      categories.map((category) => category.tokens).toList(growable: false),
      filledCellCount,
    );
    final filledCellColors = <Color>[
      for (var index = 0; index < categories.length; index++)
        for (var cell = 0; cell < categoryCellCounts[index]; cell++)
          categories[index].color,
    ];
    final grid = ExcludeSemantics(
      child: SizedBox(
        width: 147,
        child: Wrap(
          spacing: 3,
          runSpacing: 3,
          children: List<Widget>.generate(100, (index) {
            final color = index < filledCellColors.length
                ? filledCellColors[index]
                : null;
            final fillFraction = index < fullCellCount
                ? 1.0
                : index == fullCellCount
                ? partialCellFill
                : 0.0;
            return SizedBox.square(
              dimension: 12,
              child: DecoratedBox(
                decoration: BoxDecoration(
                  borderRadius: BorderRadius.circular(2),
                  border: Border.all(
                    color: palette.secondaryText.withValues(alpha: 0.48),
                    width: 0.8,
                  ),
                ),
                child: ClipRRect(
                  borderRadius: BorderRadius.circular(1.4),
                  child: Align(
                    alignment: Alignment.centerLeft,
                    child: FractionallySizedBox(
                      widthFactor: fillFraction,
                      heightFactor: 1,
                      child: DecoratedBox(
                        decoration: BoxDecoration(color: color),
                        child: const SizedBox.expand(),
                      ),
                    ),
                  ),
                ),
              ),
            );
          }),
        ),
      ),
    );
    final legend = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        _buildCategoryList(visibleCategories, detailStyle, palette),
        const SizedBox(height: 5),
        _buildCategoryRow(freeSpace, detailStyle, palette: palette),
      ],
    );

    return contentWidth < 420
        ? Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Center(child: grid),
              const SizedBox(height: 12),
              legend,
            ],
          )
        : Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              grid,
              const SizedBox(width: 14),
              Expanded(child: legend),
            ],
          );
  }

  Widget _buildCategoryList(
    List<_ContextUsageCategory> categories,
    TextStyle? detailStyle,
    OpenChatPalette palette,
  ) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        for (final category in categories) ...[
          _buildCategoryRow(category, detailStyle, palette: palette),
          if (category.details.isNotEmpty)
            _buildDetailsList(category.details, detailStyle, category.color),
        ],
      ],
    );
  }

  Widget _buildDetailsList(
    List<_ContextUsageDetail> details,
    TextStyle? detailStyle,
    Color parentColor,
  ) {
    return Padding(
      padding: const EdgeInsets.only(left: 12, top: 1, bottom: 3),
      child: DecoratedBox(
        decoration: BoxDecoration(
          border: Border(
            left: BorderSide(color: parentColor.withValues(alpha: 0.45)),
          ),
        ),
        child: Padding(
          padding: const EdgeInsets.only(left: 10),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              for (final detail in details)
                Padding(
                  padding: const EdgeInsets.symmetric(vertical: 2),
                  child: Text(detail.label, style: detailStyle),
                ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildCategoryRow(
    _ContextUsageCategory category,
    TextStyle? detailStyle, {
    required OpenChatPalette palette,
  }) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.only(top: 4, right: 7),
            child: DecoratedBox(
              decoration: BoxDecoration(
                color: category.isFreeSpace ? null : category.color,
                border: category.isFreeSpace
                    ? Border.all(color: palette.secondaryText)
                    : null,
                borderRadius: BorderRadius.circular(2),
              ),
              child: const SizedBox.square(dimension: 8),
            ),
          ),
          Expanded(child: Text(category.label, style: detailStyle)),
        ],
      ),
    );
  }
}

class _ContextUsageCategory {
  const _ContextUsageCategory(
    this.label,
    this.tokens,
    this.color, {
    this.isFreeSpace = false,
    this.details = const <_ContextUsageDetail>[],
  });

  final String label;
  final int tokens;
  final Color color;
  final bool isFreeSpace;
  final List<_ContextUsageDetail> details;
}

class _ContextUsageDetail {
  const _ContextUsageDetail(this.label);

  final String label;
}

List<int> _allocateProportionally(List<int> weights, int total) {
  if (weights.isEmpty || total <= 0) {
    return List<int>.filled(weights.length, 0, growable: false);
  }
  final weightTotal = weights.fold<int>(0, (sum, weight) => sum + weight);
  if (weightTotal <= 0) {
    return List<int>.filled(weights.length, 0, growable: false);
  }

  final shares = weights
      .map((weight) => weight / weightTotal * total)
      .toList(growable: false);
  final allocated = shares.map((share) => share.floor()).toList();
  var remainder = total - allocated.fold<int>(0, (sum, count) => sum + count);
  final remainderOrder = List<int>.generate(weights.length, (index) => index)
    ..sort((left, right) {
      final leftRemainder = shares[left] - allocated[left];
      final rightRemainder = shares[right] - allocated[right];
      return rightRemainder.compareTo(leftRemainder);
    });
  for (final index in remainderOrder) {
    if (remainder == 0) break;
    if (weights[index] == 0) continue;
    allocated[index]++;
    remainder--;
  }
  return allocated;
}

const _opaqueCheckpointEstimate = 4096;

class _ContextUsageSnapshot {
  const _ContextUsageSnapshot({
    required this.totalTokens,
    required this.breakdown,
  });

  final int totalTokens;
  final _ContextUsageBreakdown breakdown;
}

class _ContextUsageBreakdown {
  const _ContextUsageBreakdown({
    required this.instructionsTokens,
    required this.toolDefinitionTokens,
    required this.userMessageTokens,
    required this.assistantMessageTokens,
    required this.attachmentUsage,
    required this.toolUsage,
    required this.memoryTokens,
    required this.draftTokens,
  });

  final int instructionsTokens;
  final int toolDefinitionTokens;
  final int userMessageTokens;
  final int assistantMessageTokens;
  final List<_AttachmentUsageTokens> attachmentUsage;
  final List<_ToolUsageTokens> toolUsage;
  final int memoryTokens;
  final int draftTokens;

  int get toolUsageTokens =>
      toolUsage.fold<int>(0, (sum, usage) => sum + usage.tokens);

  int get attachmentTokens =>
      attachmentUsage.fold<int>(0, (sum, usage) => sum + usage.tokens);

  int get messageTokens => userMessageTokens + assistantMessageTokens;

  int get totalTokens =>
      instructionsTokens +
      toolDefinitionTokens +
      messageTokens +
      toolUsageTokens +
      attachmentTokens +
      memoryTokens +
      draftTokens;

  _ContextUsageBreakdown scaledTo(int total) {
    final weights = <int>[
      instructionsTokens,
      toolDefinitionTokens,
      userMessageTokens,
      assistantMessageTokens,
      ...toolUsage.map((usage) => usage.tokens),
      ...attachmentUsage.map((usage) => usage.tokens),
      memoryTokens,
      draftTokens,
    ];
    final scaled = _allocateProportionally(weights, total);
    var index = 0;
    final scaledInstructions = scaled[index++];
    final scaledDefinitions = scaled[index++];
    final scaledUserMessages = scaled[index++];
    final scaledAssistantMessages = scaled[index++];
    final scaledTools = <_ToolUsageTokens>[
      for (final usage in toolUsage)
        _ToolUsageTokens(usage.name, scaled[index++]),
    ];
    final scaledAttachments = <_AttachmentUsageTokens>[
      for (final usage in attachmentUsage)
        _AttachmentUsageTokens(
          usage.name,
          scaled[index++],
          isDraft: usage.isDraft,
        ),
    ];
    final scaledMemory = scaled[index++];
    final scaledDraft = scaled[index];
    return _ContextUsageBreakdown(
      instructionsTokens: scaledInstructions,
      toolDefinitionTokens: scaledDefinitions,
      userMessageTokens: scaledUserMessages,
      assistantMessageTokens: scaledAssistantMessages,
      attachmentUsage: List<_AttachmentUsageTokens>.unmodifiable(
        scaledAttachments,
      ),
      toolUsage: List<_ToolUsageTokens>.unmodifiable(scaledTools),
      memoryTokens: scaledMemory,
      draftTokens: scaledDraft,
    );
  }
}

class _EstimatedMessages {
  const _EstimatedMessages({
    required this.userTokens,
    required this.assistantTokens,
    required this.attachmentUsage,
    required this.toolUsage,
  });

  final int userTokens;
  final int assistantTokens;
  final List<_AttachmentUsageTokens> attachmentUsage;
  final Map<String, int> toolUsage;

  int get toolTokens =>
      toolUsage.values.fold<int>(0, (sum, tokens) => sum + tokens);

  int get attachmentTokens =>
      attachmentUsage.fold<int>(0, (sum, usage) => sum + usage.tokens);

  int get totalTokens =>
      userTokens + assistantTokens + toolTokens + attachmentTokens;
}

_EstimatedMessages _estimateMessages(Iterable<ChatMessage> messages) {
  var userTokens = 0;
  var assistantTokens = 0;
  final attachmentUsage = <_AttachmentUsageTokens>[];
  final toolUsage = <String, int>{};
  for (final message in messages) {
    attachmentUsage.addAll(message.attachments.map(_estimateAttachment));
    if (message.role == ChatMessageRole.user) {
      userTokens += _estimateText(message.content);
      continue;
    }

    // Visible assistant text is estimated separately from tool call payloads.
    assistantTokens += _estimateText(message.content);
    for (final activity in message.toolActivities) {
      if (activity.status != ChatToolActivityStatus.completed) continue;
      var activityTokens = _estimateText(
        jsonEncode(<String, Object?>{
          'type': 'function_call',
          'call_id': activity.callId,
          'name': activity.name,
          'arguments': jsonEncode(activity.arguments),
        }),
      );
      if (activity.output case final output?) {
        activityTokens += _estimateText(
          jsonEncode(<String, Object?>{
            'type': 'function_call_output',
            'call_id': activity.callId,
            'output': jsonEncode(output),
          }),
        );
      }
      toolUsage.update(
        activity.name,
        (tokens) => tokens + activityTokens,
        ifAbsent: () => activityTokens,
      );
    }
  }
  return _EstimatedMessages(
    userTokens: userTokens,
    assistantTokens: assistantTokens,
    attachmentUsage: List<_AttachmentUsageTokens>.unmodifiable(attachmentUsage),
    toolUsage: Map<String, int>.unmodifiable(
      Map<String, int>.fromEntries(
        toolUsage.entries.toList()
          ..sort((left, right) => left.key.compareTo(right.key)),
      ),
    ),
  );
}

class _ToolUsageTokens {
  const _ToolUsageTokens(this.name, this.tokens);

  final String name;
  final int tokens;
}

int _estimateText(String text) => utf8.encode(text).length;

_AttachmentUsageTokens _estimateAttachment(
  ChatAttachment attachment, {
  bool isDraft = false,
}) => _AttachmentUsageTokens(
  attachment.name,
  attachment.isImage ? 2048 : (attachment.sizeBytes / 4).ceil(),
  isDraft: isDraft,
);

class _AttachmentUsageTokens {
  const _AttachmentUsageTokens(this.name, this.tokens, {this.isDraft = false});

  final String name;
  final int tokens;
  final bool isDraft;
}
