import 'dart:async';

import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

class AgentRunManagerDialog extends StatefulWidget {
  const AgentRunManagerDialog({
    required this.serviceClient,
    required this.onOpenConversation,
    super.key,
  });

  final OpenChatServiceClient serviceClient;
  final ValueChanged<String> onOpenConversation;

  @override
  State<AgentRunManagerDialog> createState() => _AgentRunManagerDialogState();
}

class _AgentRunManagerDialogState extends State<AgentRunManagerDialog> {
  List<_AgentRunSummary> _runs = const <_AgentRunSummary>[];
  bool _isLoading = true;
  String? _errorCode;
  int _requestGeneration = 0;
  final Map<String, _LiveSubagentProgress> _liveSubagentProgress =
      <String, _LiveSubagentProgress>{};
  StreamSubscription<OpenChatServiceEvent>? _serviceEvents;

  @override
  void initState() {
    super.initState();
    _serviceEvents = widget.serviceClient.events.listen(_handleServiceEvent);
    _loadRuns();
  }

  void _handleServiceEvent(OpenChatServiceEvent event) {
    if (!mounted) return;
    if (event.name != 'chat.subagent.updated') return;
    final runId = event.data['runId'];
    final phase = event.data['phase'];
    final toolName = event.data['toolName'];
    if (runId is! String ||
        runId.isEmpty ||
        phase is! String ||
        (toolName != null && toolName is! String)) {
      return;
    }
    if (!_liveSubagentProgress.containsKey(runId) &&
        _liveSubagentProgress.length >= 50) {
      _liveSubagentProgress.remove(_liveSubagentProgress.keys.first);
    }
    setState(() {
      _liveSubagentProgress[runId] = _LiveSubagentProgress(
        phase: phase,
        toolName: toolName is String ? toolName : null,
      );
    });
  }

  @override
  void dispose() {
    unawaited(_serviceEvents?.cancel());
    super.dispose();
  }

  Future<void> _loadRuns() async {
    final generation = ++_requestGeneration;
    setState(() {
      _isLoading = true;
      _errorCode = null;
    });
    try {
      final response = await widget.serviceClient.call('chat.runs.list');
      final rawRuns = response['runs'];
      if (rawRuns is! List<Object?>) {
        throw const FormatException('The run list was invalid.');
      }
      final runs = rawRuns
          .map(_AgentRunSummary.fromObject)
          .toList(growable: false);
      if (mounted && generation == _requestGeneration) {
        setState(() {
          _runs = runs;
          _isLoading = false;
        });
      }
    } on OpenChatServiceException catch (error) {
      if (mounted && generation == _requestGeneration) {
        setState(() {
          _errorCode = error.code;
          _isLoading = false;
        });
      }
    } on Exception {
      if (mounted && generation == _requestGeneration) {
        setState(() {
          _errorCode = 'invalid_response';
          _isLoading = false;
        });
      }
    }
  }

  String _statusLabel(BuildContext context, String status) {
    final l10n = context.openchatL10n;
    return switch (status) {
      'running' => l10n.agentRunStatusRunning,
      'paused' => l10n.agentRunStatusPaused,
      'interrupted' => l10n.agentRunStatusInterrupted,
      'completed' => l10n.agentRunStatusCompleted,
      'failed' => l10n.agentRunStatusFailed,
      'cancelled' => l10n.agentRunStatusCancelled,
      _ => l10n.agentRunStatusUnavailable,
    };
  }

  String _providerLabel(BuildContext context, String providerId) {
    final l10n = context.openchatL10n;
    return switch (providerId) {
      'chatgpt' => l10n.chatGptProvider,
      'opencode' => 'OpenCode',
      'gemini' => 'Gemini',
      'groq' => 'Groq',
      'cerebras' => 'Cerebras',
      'openrouter' => 'OpenRouter',
      'mistral' => 'Mistral',
      'llama_cpp' => 'llama.cpp',
      'vllm' => 'vLLM',
      'exllama' => 'ExLlama',
      _ => providerId,
    };
  }

  String _updatedLabel(BuildContext context, int timestamp) {
    final date = DateTime.fromMillisecondsSinceEpoch(timestamp).toLocal();
    final localizations = MaterialLocalizations.of(context);
    final time = localizations.formatTimeOfDay(TimeOfDay.fromDateTime(date));
    return '${localizations.formatMediumDate(date)} · $time';
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    return Dialog(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 560, maxHeight: 640),
        child: Padding(
          padding: const EdgeInsets.fromLTRB(20, 16, 20, 16),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Row(
                children: [
                  Expanded(
                    child: Text(
                      l10n.agentRunManagerTitle,
                      style: Theme.of(context).textTheme.titleLarge,
                    ),
                  ),
                  IconButton(
                    tooltip: l10n.agentRunRefresh,
                    onPressed: _isLoading ? null : _loadRuns,
                    icon: const Icon(LucideIcons.refreshCw),
                  ),
                  IconButton(
                    tooltip: l10n.close,
                    onPressed: () => Navigator.of(context).pop(),
                    icon: const Icon(LucideIcons.x),
                  ),
                ],
              ),
              const SizedBox(height: 4),
              Text(
                l10n.agentRunManagerDescription,
                style: Theme.of(context).textTheme.bodyMedium
                    ?.copyWith(color: palette.secondaryText),
              ),
              const SizedBox(height: 16),
              Flexible(child: _buildRunList(context)),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildRunList(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    if (_isLoading) {
      return Center(
        child: Semantics(
          label: l10n.agentRunLoading,
          child: const CircularProgressIndicator(),
        ),
      );
    }
    if (_errorCode != null) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(l10n.agentRunLoadFailed, textAlign: TextAlign.center),
            const SizedBox(height: 12),
            OutlinedButton.icon(
              onPressed: _loadRuns,
              icon: const Icon(LucideIcons.refreshCw),
              label: Text(l10n.agentRunRefresh),
            ),
          ],
        ),
      );
    }
    if (_runs.isEmpty) {
      return Center(
        child: Text(
          l10n.agentRunEmpty,
          textAlign: TextAlign.center,
          style: TextStyle(color: palette.secondaryText),
        ),
      );
    }
    return ListView.separated(
      shrinkWrap: true,
      itemCount: _runs.length,
      separatorBuilder: (context, index) =>
          Divider(height: 1, color: palette.border.withValues(alpha: 0.6)),
      itemBuilder: (context, index) {
        final run = _runs[index];
        final providerLabel = switch (run.providerId) {
          final String providerId => _providerLabel(context, providerId),
          null => null,
        };
        final route = <String>[?providerLabel, ?run.modelId].join(' · ');
        final title = run.runKind == 'subagent' && run.objective != null
            ? l10n.agentRunSubagentTask(run.objective!)
            : run.conversationTitle ?? l10n.newChat;
        final liveProgress = _liveSubagentProgress[run.runId];
        final progressPhase = liveProgress?.phase ?? run.progressPhase;
        final progressToolName = liveProgress?.toolName ?? run.progressToolName;
        final progressLabel = run.status != 'running'
            ? null
            : switch (progressPhase) {
                'starting' => l10n.agentRunLiveStarting,
                'thinking' => l10n.agentRunLiveThinking,
                'tool' => switch (progressToolName) {
                  final toolName? => l10n.agentRunLiveUsingTool(toolName),
                  _ => l10n.agentRunLiveThinking,
                },
                _ => null,
              };
        final icon = switch (run.status) {
          'running' => LucideIcons.loaderCircle,
          'paused' || 'interrupted' => LucideIcons.pause,
          'completed' => LucideIcons.circleCheck,
          'failed' => LucideIcons.circleX,
          'cancelled' => LucideIcons.circleSlash,
          _ => LucideIcons.circleHelp,
        };
        return Semantics(
          key: ValueKey<String>(run.runId),
          container: true,
          label: '$title, ${_statusLabel(context, run.status)}',
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 10),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Padding(
                      padding: const EdgeInsets.only(top: 2, right: 10),
                      child: Icon(icon, color: palette.secondaryIcon),
                    ),
                    Expanded(
                      child: Text(
                        title,
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                        style: Theme.of(context).textTheme.titleSmall,
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 6),
                Text(_statusLabel(context, run.status)),
                if (progressLabel != null)
                  Semantics(liveRegion: true, child: Text(progressLabel)),
                Text(_updatedLabel(context, run.updatedAtUnixMs)),
                if (route.isNotEmpty)
                  Text(route, maxLines: 2, overflow: TextOverflow.ellipsis),
                Align(
                  alignment: AlignmentDirectional.centerEnd,
                  child: TextButton(
                    onPressed: () {
                      widget.onOpenConversation(run.conversationId);
                      Navigator.of(context).pop();
                    },
                    child: Text(l10n.agentRunOpenConversation),
                  ),
                ),
              ],
            ),
          ),
        );
      },
    );
  }
}

class _LiveSubagentProgress {
  const _LiveSubagentProgress({required this.phase, this.toolName});

  final String phase;
  final String? toolName;
}

class _AgentRunSummary {
  const _AgentRunSummary({
    required this.runId,
    required this.conversationId,
    required this.conversationTitle,
    required this.status,
    required this.updatedAtUnixMs,
    required this.providerId,
    required this.modelId,
    required this.runKind,
    required this.parentRunId,
    required this.objective,
    required this.progressPhase,
    required this.progressToolName,
  });

  final String runId;
  final String conversationId;
  final String? conversationTitle;
  final String status;
  final int updatedAtUnixMs;
  final String? providerId;
  final String? modelId;
  final String? runKind;
  final String? parentRunId;
  final String? objective;
  final String? progressPhase;
  final String? progressToolName;

  factory _AgentRunSummary.fromObject(Object? value) {
    if (value is! Map<String, Object?>) {
      throw const FormatException('A run summary was invalid.');
    }
    final runId = value['runId'];
    final conversationId = value['conversationId'];
    final conversationTitle = value['conversationTitle'];
    final status = value['status'];
    final updatedAtUnixMs = value['updatedAtUnixMs'];
    final providerIdValue = value['providerId'];
    final modelIdValue = value['modelId'];
    final runKindValue = value['runKind'];
    final parentRunIdValue = value['parentRunId'];
    final objectiveValue = value['objective'];
    final progressPhaseValue = value['progressPhase'];
    final progressToolNameValue = value['progressToolName'];
    final providerId = switch (providerIdValue) {
      null => null,
      final String id => id,
      _ => throw const FormatException('A run summary was invalid.'),
    };
    final modelId = switch (modelIdValue) {
      null => null,
      final String id => id,
      _ => throw const FormatException('A run summary was invalid.'),
    };
    final runKind = switch (runKindValue) {
      null => null,
      final String kind => kind,
      _ => throw const FormatException('A run summary was invalid.'),
    };
    final parentRunId = switch (parentRunIdValue) {
      null => null,
      final String id => id,
      _ => throw const FormatException('A run summary was invalid.'),
    };
    final objective = switch (objectiveValue) {
      null => null,
      final String value => value,
      _ => throw const FormatException('A run summary was invalid.'),
    };
    final progressPhase = switch (progressPhaseValue) {
      null => null,
      final String phase => phase,
      _ => throw const FormatException('A run summary was invalid.'),
    };
    final progressToolName = switch (progressToolNameValue) {
      null => null,
      final String name => name,
      _ => throw const FormatException('A run summary was invalid.'),
    };
    final title = switch (conversationTitle) {
      null => null,
      final String name => name,
      _ => throw const FormatException('A run summary was invalid.'),
    };
    if (runId is! String ||
        conversationId is! String ||
        status is! String ||
        updatedAtUnixMs is! int) {
      throw const FormatException('A run summary was invalid.');
    }
    return _AgentRunSummary(
      runId: runId,
      conversationId: conversationId,
      conversationTitle: title,
      status: status,
      updatedAtUnixMs: updatedAtUnixMs,
      providerId: providerId,
      modelId: modelId,
      runKind: runKind,
      parentRunId: parentRunId,
      objective: objective,
      progressPhase: progressPhase,
      progressToolName: progressToolName,
    );
  }
}
