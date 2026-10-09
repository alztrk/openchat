import 'dart:async';

import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/agent_goal.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

class GoalStatusBar extends StatefulWidget {
  const GoalStatusBar({
    required this.goal,
    required this.isBusy,
    required this.onPauseOrResume,
    required this.onStop,
    super.key,
  });

  final AgentGoal goal;
  final bool isBusy;
  final VoidCallback onPauseOrResume;
  final VoidCallback onStop;

  @override
  State<GoalStatusBar> createState() => _GoalStatusBarState();
}

class _GoalStatusBarState extends State<GoalStatusBar> {
  Timer? _elapsedTimer;
  Duration _elapsed = Duration.zero;
  bool _showTasks = false;

  @override
  void initState() {
    super.initState();
    _updateElapsed();
    _startTimerIfRunning();
  }

  @override
  void didUpdateWidget(covariant GoalStatusBar oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.goal.startedAt != widget.goal.startedAt ||
        oldWidget.goal.status != widget.goal.status) {
      _updateElapsed();
      _startTimerIfRunning();
    }
  }

  @override
  void dispose() {
    _elapsedTimer?.cancel();
    super.dispose();
  }

  void _startTimerIfRunning() {
    _elapsedTimer?.cancel();
    if (widget.goal.status != AgentGoalStatus.running) return;
    _elapsedTimer = Timer.periodic(const Duration(seconds: 1), (_) {
      if (!mounted) return;
      setState(_updateElapsed);
    });
  }

  void _updateElapsed() {
    final elapsed = DateTime.now().toUtc().difference(widget.goal.startedAt);
    _elapsed = elapsed.isNegative ? Duration.zero : elapsed;
  }

  String _formatElapsed(Duration duration) {
    final totalSeconds = duration.inSeconds;
    final hours = totalSeconds ~/ 3600;
    final minutes = totalSeconds ~/ 60 % 60;
    final seconds = totalSeconds % 60;
    if (hours > 0) {
      return '${hours.toString().padLeft(2, '0')}:${minutes.toString().padLeft(2, '0')}:${seconds.toString().padLeft(2, '0')}';
    }
    return '${totalSeconds ~/ 60}:${seconds.toString().padLeft(2, '0')}';
  }

  String _statusLabel(BuildContext context) {
    final l10n = context.openchatL10n;
    return switch (widget.goal.status) {
      AgentGoalStatus.running => l10n.goalWorking,
      AgentGoalStatus.paused => l10n.goalPaused,
      AgentGoalStatus.interrupted => l10n.goalInterrupted,
      AgentGoalStatus.completed => l10n.goalCompleted,
      AgentGoalStatus.cancelled => l10n.goalStopped,
      AgentGoalStatus.failed => l10n.goalFailed,
    };
  }

  String? _pauseDescription(BuildContext context) {
    final l10n = context.openchatL10n;
    return switch (widget.goal.pauseReason) {
      'quota' => l10n.goalPausedForQuota,
      'blocked' => l10n.goalPausedForBlocker,
      'needs_user' => l10n.goalPausedForUserInput,
      'user_paused' => l10n.goalPausedByUser,
      'interrupted' => l10n.goalInterrupted,
      'request_failed' => l10n.goalPausedAfterError,
      _ => null,
    };
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final theme = Theme.of(context);
    final palette = OpenChatPalette.of(context);
    final isRunning = widget.goal.status == AgentGoalStatus.running;
    final canResume =
        widget.goal.status == AgentGoalStatus.paused ||
        widget.goal.status == AgentGoalStatus.interrupted;
    final progress = widget.goal.progress.trim();
    final description = _pauseDescription(context);
    final detail =
        description ?? (progress.isNotEmpty ? progress : widget.goal.objective);

    return Container(
      padding: const EdgeInsetsDirectional.only(start: 12, top: 7, bottom: 7),
      decoration: BoxDecoration(
        color: palette.raisedSurface,
        border: Border.all(color: palette.border.withValues(alpha: 0.72)),
        borderRadius: BorderRadius.circular(OpenChatRadii.control),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Icon(
                isRunning ? LucideIcons.circleDashed : LucideIcons.circlePause,
                size: 16,
                color: palette.accentIcon,
              ),
              const SizedBox(width: 9),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Semantics(
                      liveRegion: true,
                      label: _statusLabel(context),
                      child: ExcludeSemantics(
                        child: Text(
                          _statusLabel(context),
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: theme.textTheme.labelMedium?.copyWith(
                            color: palette.text,
                            fontWeight: FontWeight.w600,
                          ),
                        ),
                      ),
                    ),
                    const SizedBox(height: 1),
                    Text(
                      detail,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: palette.secondaryText,
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(width: 8),
              Semantics(
                label: l10n.goalElapsedTime(_formatElapsed(_elapsed)),
                child: Text(
                  _formatElapsed(_elapsed),
                  style: theme.textTheme.labelSmall?.copyWith(
                    color: palette.secondaryText,
                    fontFeatures: const [FontFeature.tabularFigures()],
                  ),
                ),
              ),
              const SizedBox(width: 4),
              IconButton(
                tooltip: canResume
                    ? l10n.goalResumeAction
                    : l10n.goalPauseAction,
                onPressed: widget.isBusy || (!isRunning && !canResume)
                    ? null
                    : widget.onPauseOrResume,
                visualDensity: VisualDensity.standard,
                padding: EdgeInsets.zero,
                constraints: const BoxConstraints.tightFor(
                  width: 44,
                  height: 44,
                ),
                icon: Icon(
                  canResume ? LucideIcons.play : LucideIcons.pause,
                  size: 16,
                ),
              ),
              IconButton(
                tooltip: l10n.goalStopAction,
                onPressed: widget.isBusy ? null : widget.onStop,
                visualDensity: VisualDensity.standard,
                padding: EdgeInsets.zero,
                constraints: const BoxConstraints.tightFor(
                  width: 44,
                  height: 44,
                ),
                icon: const Icon(LucideIcons.square, size: 15),
              ),
            ],
          ),
          if (widget.goal.todos.isNotEmpty) ...[
            Align(
              alignment: AlignmentDirectional.centerStart,
              child: TextButton.icon(
                onPressed: () => setState(() => _showTasks = !_showTasks),
                icon: Icon(
                  _showTasks ? LucideIcons.chevronUp : LucideIcons.chevronDown,
                  size: 15,
                ),
                label: Text(
                  l10n.goalTasksCount(
                    widget.goal.todos.where((todo) => todo.completed).length,
                    widget.goal.todos.length,
                  ),
                ),
              ),
            ),
            if (_showTasks)
              ...widget.goal.todos.map(
                (todo) => Semantics(
                  checked: todo.completed,
                  child: Padding(
                    padding: const EdgeInsetsDirectional.only(
                      start: 28,
                      bottom: 4,
                    ),
                    child: Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Icon(
                          todo.completed
                              ? LucideIcons.circleCheck
                              : LucideIcons.circle,
                          size: 15,
                          color: todo.completed
                              ? theme.colorScheme.primary
                              : palette.secondaryIcon,
                        ),
                        const SizedBox(width: 8),
                        Expanded(child: Text(todo.text)),
                      ],
                    ),
                  ),
                ),
              ),
          ],
        ],
      ),
    );
  }
}
