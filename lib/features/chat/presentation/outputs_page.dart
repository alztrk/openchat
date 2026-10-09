import 'dart:async';

import 'package:flutter/material.dart';
import 'package:intl/intl.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_page_header.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/openchat_toast.dart';
import 'package:openchat/features/chat/data/chat_repository.dart';
import 'package:openchat/features/chat/domain/chat_saved_output.dart';
import 'package:openchat/features/chat/presentation/widgets/assistant_message.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

class OutputsPage extends StatefulWidget {
  const OutputsPage({
    required this.repository,
    required this.onOpenConversation,
    this.pageHeadingFocusNode,
    this.onRetry,
    super.key,
  });

  final ChatRepository? repository;
  final ValueChanged<ChatSavedOutput> onOpenConversation;
  final FocusNode? pageHeadingFocusNode;
  final VoidCallback? onRetry;

  @override
  State<OutputsPage> createState() => _OutputsPageState();
}

class _OutputsPageState extends State<OutputsPage> {
  int _streamGeneration = 0;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final repository = widget.repository;
    final compact = MediaQuery.sizeOf(context).width < 640;

    return SingleChildScrollView(
      padding: EdgeInsets.fromLTRB(
        compact ? OpenChatSpacing.md : OpenChatSpacing.xl,
        OpenChatSpacing.lg,
        compact ? OpenChatSpacing.md : OpenChatSpacing.xl,
        OpenChatSpacing.section,
      ),
      child: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 960),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              OpenChatPageHeader(
                title: l10n.outputs,
                description: l10n.outputsDescription,
                focusNode: widget.pageHeadingFocusNode,
              ),
              const SizedBox(height: OpenChatSpacing.xl),
              if (repository == null)
                _loadFailure(context)
              else
                StreamBuilder<List<ChatSavedOutput>>(
                  key: ValueKey<int>(_streamGeneration),
                  stream: repository.watchSavedOutputs(),
                  builder: (context, snapshot) {
                    if (snapshot.hasError) return _loadFailure(context);
                    if (!snapshot.hasData) {
                      return Center(
                        child: Padding(
                          padding: const EdgeInsets.all(OpenChatSpacing.xl),
                          child: Semantics(
                            liveRegion: true,
                            label: l10n.outputsLoading,
                            child: const CircularProgressIndicator(),
                          ),
                        ),
                      );
                    }
                    if (snapshot.data!.isEmpty) {
                      return const _EmptyOutputs();
                    }
                    return Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        for (final output in snapshot.data!) ...[
                          _SavedOutputCard(
                            output: output,
                            onOpen: () => widget.onOpenConversation(output),
                            onRemove: () => unawaited(_removeOutput(output)),
                          ),
                          const SizedBox(height: OpenChatSpacing.md),
                        ],
                      ],
                    );
                  },
                ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _loadFailure(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    return Container(
      padding: const EdgeInsets.all(OpenChatSpacing.lg),
      decoration: BoxDecoration(
        color: palette.surface,
        borderRadius: BorderRadius.circular(OpenChatRadii.panel),
        border: Border.all(color: palette.border),
      ),
      child: Wrap(
        alignment: WrapAlignment.spaceBetween,
        crossAxisAlignment: WrapCrossAlignment.center,
        spacing: OpenChatSpacing.md,
        runSpacing: OpenChatSpacing.md,
        children: [
          Text(
            l10n.outputsLoadFailed,
            style: Theme.of(context).textTheme.bodyMedium,
          ),
          OutlinedButton.icon(
            onPressed: widget.onRetry == null
                ? null
                : () {
                    setState(() => _streamGeneration++);
                    widget.onRetry?.call();
                  },
            icon: const Icon(LucideIcons.refreshCw),
            label: Text(l10n.retry),
          ),
        ],
      ),
    );
  }

  Future<void> _removeOutput(ChatSavedOutput output) async {
    final repository = widget.repository;
    if (repository == null) return;
    try {
      await repository.removeSavedOutput(
        conversationId: output.conversationId,
        messageId: output.messageId,
      );
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'outputs',
          context: ErrorDescription('while removing a saved response'),
        ),
      );
      if (mounted) {
        showOpenChatToast(
          context,
          context.openchatL10n.outputRemoveFailed,
          type: OpenChatToastType.error,
        );
      }
    }
  }
}

class _SavedOutputCard extends StatefulWidget {
  const _SavedOutputCard({
    required this.output,
    required this.onOpen,
    required this.onRemove,
  });

  final ChatSavedOutput output;
  final VoidCallback onOpen;
  final VoidCallback onRemove;

  @override
  State<_SavedOutputCard> createState() => _SavedOutputCardState();
}

class _SavedOutputCardState extends State<_SavedOutputCard> {
  bool _expanded = false;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final locale = l10n.localeName;
    final date = DateFormat.yMMMd(locale)
        .format(widget.output.savedAt.toLocal());
    final source = _savedOutputSource(widget.output, l10n);
    final canExpand =
        widget.output.content.length > 360 ||
        widget.output.content.split('\n').length > 8;

    return Container(
      padding: const EdgeInsets.all(OpenChatSpacing.md),
      decoration: BoxDecoration(
        color: palette.surface,
        borderRadius: BorderRadius.circular(OpenChatRadii.panel),
        border: Border.all(color: palette.subtleBorder),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Wrap(
            alignment: WrapAlignment.spaceBetween,
            crossAxisAlignment: WrapCrossAlignment.center,
            spacing: OpenChatSpacing.md,
            runSpacing: OpenChatSpacing.xs,
            children: [
              ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 680),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      widget.output.conversationTitle,
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: Theme.of(context).textTheme.titleMedium,
                    ),
                    const SizedBox(height: OpenChatSpacing.xxs),
                    Text(
                      l10n.outputSavedAt(date),
                      style: Theme.of(context).textTheme.bodySmall,
                    ),
                    if (source case final source?) ...[
                      const SizedBox(height: OpenChatSpacing.xxs),
                      Text(
                        source,
                        style: Theme.of(context).textTheme.bodySmall
                            ?.copyWith(color: palette.secondaryText),
                      ),
                    ],
                  ],
                ),
              ),
              Wrap(
                spacing: OpenChatSpacing.xxs,
                children: [
                  CopyMessageButton(content: widget.output.content),
                  TextButton.icon(
                    onPressed: widget.onOpen,
                    icon: const Icon(LucideIcons.messageCircle),
                    label: Text(l10n.outputOpenConversation),
                  ),
                  IconButton(
                    tooltip: l10n.removeSavedResponse,
                    onPressed: widget.onRemove,
                    icon: Icon(
                      LucideIcons.bookmarkMinus,
                      color: palette.secondaryIcon,
                    ),
                  ),
                ],
              ),
            ],
          ),
          const SizedBox(height: OpenChatSpacing.md),
          Text(
            widget.output.content.trim(),
            maxLines: _expanded ? null : 8,
            overflow: _expanded ? TextOverflow.visible : TextOverflow.ellipsis,
            style: Theme.of(context).textTheme.bodyLarge
                ?.copyWith(color: palette.text, height: 1.55),
          ),
          if (canExpand) ...[
            const SizedBox(height: OpenChatSpacing.xs),
            Align(
              alignment: Alignment.centerLeft,
              child: TextButton(
                onPressed: () => setState(() => _expanded = !_expanded),
                child: Text(_expanded ? l10n.showLess : l10n.showMore),
              ),
            ),
          ],
        ],
      ),
    );
  }
}

String? _savedOutputSource(ChatSavedOutput output, AppLocalizations l10n) {
  final providerId = output.providerId?.trim();
  final providerLabel = switch (providerId) {
    null || '' => null,
    'chatgpt' || 'chatgpt_api' => l10n.chatGptProvider,
    'opencode' => l10n.openCodeProvider,
    'gemini' => l10n.geminiProvider,
    'groq' => l10n.groqProvider,
    'cerebras' => l10n.cerebrasProvider,
    'openrouter' => l10n.openRouterProvider,
    'mistral' => l10n.mistralProvider,
    'llama_cpp' || 'vllm' || 'exllama' => l10n.localModelsPageTitle,
    _ => l10n.otherProvider,
  };
  final modelId = output.modelId?.trim();
  final labels = <String>[
    if (providerLabel != null) l10n.outputSourceProvider(providerLabel),
    if (modelId != null && modelId.isNotEmpty) l10n.outputSourceModel(modelId),
  ];
  return labels.isEmpty ? null : labels.join(' · ');
}

class _EmptyOutputs extends StatelessWidget {
  const _EmptyOutputs();

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    return Container(
      alignment: Alignment.center,
      padding: const EdgeInsets.all(OpenChatSpacing.section),
      decoration: BoxDecoration(
        color: palette.surface,
        borderRadius: BorderRadius.circular(OpenChatRadii.panel),
        border: Border.all(color: palette.subtleBorder),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(LucideIcons.bookmark, size: 28, color: palette.secondaryIcon),
          const SizedBox(height: OpenChatSpacing.md),
          Text(
            l10n.outputsEmptyTitle,
            textAlign: TextAlign.center,
            style: Theme.of(context).textTheme.titleLarge,
          ),
          const SizedBox(height: OpenChatSpacing.xs),
          ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 440),
            child: Text(
              l10n.outputsEmptyDescription,
              textAlign: TextAlign.center,
              style: Theme.of(context).textTheme.bodyMedium,
            ),
          ),
        ],
      ),
    );
  }
}
