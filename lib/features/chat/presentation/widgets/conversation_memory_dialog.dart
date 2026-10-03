import 'dart:async';

import 'package:flutter/material.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/features/chat/data/conversation_memory_repository.dart';
import 'package:openchat/features/chat/domain/conversation_memory.dart';

class ConversationMemorySection extends StatefulWidget {
  const ConversationMemorySection({
    required this.repository,
    required this.conversationId,
    required this.isSending,
    this.conversationTitle,
    this.embedded = false,
    super.key,
  });

  final ConversationMemoryRepository? repository;
  final String? conversationId;
  final String? conversationTitle;
  final bool isSending;
  final bool embedded;

  @override
  State<ConversationMemorySection> createState() =>
      _ConversationMemorySectionState();
}

class ConversationMemoryDialog extends ConversationMemorySection {
  const ConversationMemoryDialog({
    required ConversationMemoryRepository repository,
    required String conversationId,
    required super.isSending,
    super.key,
  }) : super(repository: repository, conversationId: conversationId);
}

class _ConversationMemorySectionState extends State<ConversationMemorySection> {
  final _queryController = TextEditingController();
  ConversationMemoryState? _memoryState;
  List<ArchivedMemoryResult> _searchResults = const <ArchivedMemoryResult>[];
  int _searchGeneration = 0;
  int _memoryLoadGeneration = 0;
  int _semanticStatusGeneration = 0;
  bool _isLoadingMemory = true;
  bool _isSearching = false;
  bool _isResetting = false;
  bool _hasSearched = false;
  bool _searchQueryTooShort = false;
  bool _loadFailed = false;
  bool _searchFailed = false;
  bool _resetFailed = false;
  bool? _semanticSearchReady;
  bool _isPreparingSemanticSearch = false;
  bool _isCancellingSemanticPreparation = false;
  bool _semanticPreparationFailed = false;
  bool _semanticPreparationCancelled = false;
  SemanticPreparationProgress? _semanticPreparationProgress;
  SemanticSearchPreparation? _semanticPreparation;
  StreamSubscription<SemanticPreparationProgress>?
  _semanticPreparationProgressSubscription;

  @override
  void initState() {
    super.initState();
    _loadMemory();
    _loadSemanticSearchStatus();
  }

  @override
  void didUpdateWidget(covariant ConversationMemorySection oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.conversationId != widget.conversationId ||
        oldWidget.repository != widget.repository) {
      _searchGeneration++;
      _memoryState = null;
      _searchResults = const <ArchivedMemoryResult>[];
      _hasSearched = false;
      _searchQueryTooShort = false;
      _searchFailed = false;
      _resetFailed = false;
      _loadMemory();
      if (oldWidget.repository != widget.repository) {
        _semanticSearchReady = null;
        _semanticPreparationFailed = false;
        _loadSemanticSearchStatus();
      }
    }
  }

  @override
  void dispose() {
    _searchGeneration++;
    final progressSubscription = _semanticPreparationProgressSubscription;
    if (progressSubscription != null) {
      unawaited(progressSubscription.cancel());
    }
    final preparation = _semanticPreparation;
    if (preparation != null) unawaited(preparation.cancel());
    _queryController.dispose();
    super.dispose();
  }

  Future<void> _loadMemory() async {
    final generation = ++_memoryLoadGeneration;
    final repository = widget.repository;
    final conversationId = widget.conversationId;
    if (repository == null || conversationId == null) {
      if (!mounted) return;
      setState(() {
        _memoryState = null;
        _isLoadingMemory = false;
        _loadFailed = repository == null;
      });
      return;
    }
    setState(() {
      _isLoadingMemory = true;
      _loadFailed = false;
    });
    try {
      final memoryState = await repository.inspect(conversationId);
      if (!mounted || generation != _memoryLoadGeneration) return;
      setState(() {
        _memoryState = memoryState;
        _isLoadingMemory = false;
      });
    } on Object catch (error, stackTrace) {
      _reportMemoryError(
        error,
        stackTrace,
        'while loading compacted conversation context',
      );
      if (!mounted || generation != _memoryLoadGeneration) return;
      setState(() {
        _isLoadingMemory = false;
        _loadFailed = true;
      });
    }
  }

  Future<void> _loadSemanticSearchStatus() async {
    final generation = ++_semanticStatusGeneration;
    final repository = widget.repository;
    if (repository == null) {
      if (mounted) {
        setState(() {
          _semanticSearchReady = false;
          _semanticPreparationFailed = true;
        });
      }
      return;
    }
    try {
      final isReady = await repository.semanticSearchIsReady();
      if (!mounted || generation != _semanticStatusGeneration) return;
      setState(() => _semanticSearchReady = isReady);
    } on Object catch (error, stackTrace) {
      _reportMemoryError(
        error,
        stackTrace,
        'while checking local semantic search readiness',
      );
      if (!mounted || generation != _semanticStatusGeneration) return;
      setState(() {
        _semanticSearchReady = false;
        _semanticPreparationFailed = true;
      });
    }
  }

  Future<void> _prepareSemanticSearch() async {
    if (_isPreparingSemanticSearch) return;
    final repository = widget.repository;
    if (repository == null) return;
    setState(() {
      _isPreparingSemanticSearch = true;
      _isCancellingSemanticPreparation = false;
      _semanticPreparationFailed = false;
      _semanticPreparationCancelled = false;
      _semanticPreparationProgress = null;
    });
    SemanticSearchPreparation? preparation;
    StreamSubscription<SemanticPreparationProgress>? progressSubscription;
    try {
      preparation = await repository.prepareSemanticSearch();
      if (!mounted) {
        unawaited(preparation.cancel());
        return;
      }
      _semanticPreparation = preparation;
      Object? progressStreamError;
      StackTrace? progressStreamStackTrace;
      progressSubscription = preparation.progress.listen(
        (progress) {
          if (!mounted) return;
          setState(() => _semanticPreparationProgress = progress);
        },
        onError: (Object error, StackTrace stackTrace) {
          progressStreamError = error;
          progressStreamStackTrace = stackTrace;
        },
      );
      _semanticPreparationProgressSubscription = progressSubscription;
      final isReady = await preparation.completed;
      if (!mounted) return;
      if (isReady && progressStreamError != null) {
        final stackTrace = progressStreamStackTrace;
        if (stackTrace != null) {
          _reportMemoryError(
            progressStreamError!,
            stackTrace,
            'while receiving semantic model download progress',
          );
        }
      }
      setState(() {
        _semanticSearchReady = isReady;
        _semanticPreparationCancelled = !isReady;
      });
    } on Object catch (error, stackTrace) {
      _reportMemoryError(
        error,
        stackTrace,
        'while preparing local semantic search',
      );
      if (!mounted) return;
      setState(() => _semanticPreparationFailed = true);
    } finally {
      if (progressSubscription != null) {
        unawaited(
          progressSubscription.cancel().catchError((
            Object error,
            StackTrace stackTrace,
          ) {
            _reportMemoryError(
              error,
              stackTrace,
              'while closing semantic model progress updates',
            );
          }),
        );
      }
      if (identical(_semanticPreparation, preparation)) {
        _semanticPreparation = null;
      }
      if (identical(
        _semanticPreparationProgressSubscription,
        progressSubscription,
      )) {
        _semanticPreparationProgressSubscription = null;
      }
      if (mounted) {
        setState(() {
          _isPreparingSemanticSearch = false;
          _isCancellingSemanticPreparation = false;
          _semanticPreparationProgress = null;
        });
      }
    }
  }

  Future<void> _cancelSemanticPreparation() async {
    final preparation = _semanticPreparation;
    if (preparation == null || _isCancellingSemanticPreparation) return;
    setState(() => _isCancellingSemanticPreparation = true);
    try {
      await preparation.cancel();
    } on Object catch (error, stackTrace) {
      _reportMemoryError(
        error,
        stackTrace,
        'while cancelling semantic model preparation',
      );
    }
  }

  Future<void> _searchArchive() async {
    final repository = widget.repository;
    final conversationId = widget.conversationId;
    if (repository == null || conversationId == null) return;
    final generation = ++_searchGeneration;
    final query = _queryController.text.trim();
    if (query.runes.length < 2) {
      setState(() {
        _hasSearched = false;
        _isSearching = false;
        _searchQueryTooShort = true;
        _searchFailed = false;
        _searchResults = const <ArchivedMemoryResult>[];
      });
      return;
    }

    setState(() {
      _hasSearched = true;
      _isSearching = true;
      _searchQueryTooShort = false;
      _searchFailed = false;
      _searchResults = const <ArchivedMemoryResult>[];
    });
    try {
      final results = await repository.search(conversationId, query);
      if (!mounted || generation != _searchGeneration) return;
      setState(() {
        _searchResults = results;
        _isSearching = false;
      });
    } on Object catch (error, stackTrace) {
      _reportMemoryError(
        error,
        stackTrace,
        'while searching the conversation archive',
      );
      if (!mounted || generation != _searchGeneration) return;
      setState(() {
        _isSearching = false;
        _searchFailed = true;
      });
    }
  }

  Future<void> _confirmReset() async {
    final repository = widget.repository;
    final conversationId = widget.conversationId;
    if (repository == null ||
        conversationId == null ||
        widget.isSending ||
        _isResetting ||
        _memoryState?.hasContext != true) {
      return;
    }
    final l10n = context.openchatL10n;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: Text(l10n.conversationMemoryResetTitle),
        content: Text(l10n.conversationMemoryResetConfirmation),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: Text(l10n.cancel),
          ),
          FilledButton(
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: Text(l10n.conversationMemoryResetConfirm),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted) return;

    setState(() {
      _isResetting = true;
      _resetFailed = false;
    });
    try {
      await repository.resetCompactedContext(conversationId);
      await _loadMemory();
      if (!mounted) return;
      setState(() => _isResetting = false);
    } on Object catch (error, stackTrace) {
      _reportMemoryError(
        error,
        stackTrace,
        'while resetting compacted conversation context',
      );
      if (!mounted) return;
      setState(() {
        _isResetting = false;
        _resetFailed = true;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    if (widget.embedded) return _buildEmbedded(context);

    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final size = MediaQuery.sizeOf(context);

    return AlertDialog(
      title: Row(
        children: [
          const Icon(Icons.memory_outlined, size: 20),
          const SizedBox(width: 10),
          Expanded(child: Text(l10n.conversationMemory)),
        ],
      ),
      content: SizedBox(
        width: size.width < 640 ? size.width * 0.78 : 520,
        height: size.height * 0.62,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(
              l10n.conversationMemoryDescription,
              style: Theme.of(context).textTheme.bodySmall
                  ?.copyWith(color: palette.secondaryText),
            ),
            const SizedBox(height: 12),
            _buildSemanticSearchSection(context),
            const SizedBox(height: 14),
            _buildSummarySection(context),
            const SizedBox(height: 18),
            Text(
              l10n.conversationMemorySearchTitle,
              style: Theme.of(context).textTheme.titleSmall,
            ),
            const SizedBox(height: 8),
            Row(
              children: [
                Expanded(
                  child: TextField(
                    controller: _queryController,
                    maxLength: 512,
                    maxLines: 1,
                    textInputAction: TextInputAction.search,
                    onSubmitted: (_) => _searchArchive(),
                    decoration: InputDecoration(
                      hintText: l10n.conversationMemorySearchHint,
                      counterText: '',
                      prefixIcon: const Icon(Icons.search_rounded),
                    ),
                  ),
                ),
                const SizedBox(width: 8),
                FilledButton.tonal(
                  onPressed: _isSearching ? null : _searchArchive,
                  child: _isSearching
                      ? const SizedBox.square(
                          dimension: 16,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      : Text(l10n.conversationMemorySearchAction),
                ),
              ],
            ),
            if (_searchQueryTooShort) ...[
              const SizedBox(height: 6),
              Text(
                l10n.conversationMemorySearchQueryTooShort,
                style: TextStyle(color: palette.secondaryText),
              ),
            ],
            const SizedBox(height: 10),
            Expanded(child: _buildSearchResults(context)),
          ],
        ),
      ),
      actions: [
        if (_resetFailed)
          Align(
            alignment: Alignment.centerLeft,
            child: Text(
              l10n.conversationMemoryResetFailed,
              style: TextStyle(color: Theme.of(context).colorScheme.error),
            ),
          ),
        if (_memoryState?.hasContext == true)
          TextButton.icon(
            onPressed: widget.isSending || _isResetting ? null : _confirmReset,
            icon: _isResetting
                ? const SizedBox.square(
                    dimension: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.restart_alt_rounded),
            label: Text(l10n.conversationMemoryResetAction),
          ),
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.close),
        ),
      ],
    );
  }

  Widget _buildEmbedded(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final conversationId = widget.conversationId;
    final hasConversation = conversationId != null;
    final detailStyle = Theme.of(context).textTheme.bodySmall
        ?.copyWith(color: palette.secondaryText);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(l10n.conversationMemoryDescription, style: detailStyle),
        if (widget.conversationTitle case final title?) ...[
          const SizedBox(height: 6),
          Text(
            l10n.conversationMemoryCurrentConversation(title),
            style: detailStyle,
          ),
        ],
        const SizedBox(height: 16),
        _buildSemanticSearchSection(context),
        const SizedBox(height: 18),
        if (!hasConversation)
          Container(
            padding: const EdgeInsets.all(16),
            decoration: BoxDecoration(
              color: palette.composer,
              border: Border.all(color: palette.border),
              borderRadius: BorderRadius.circular(12),
            ),
            child: Text(
              l10n.conversationMemoryNoConversation,
              style: TextStyle(color: palette.secondaryText),
            ),
          )
        else ...[
          _buildSummarySection(context),
          const SizedBox(height: 18),
          Text(
            l10n.conversationMemorySearchTitle,
            style: Theme.of(context).textTheme.titleSmall,
          ),
          const SizedBox(height: 8),
          Row(
            children: [
              Expanded(
                child: TextField(
                  controller: _queryController,
                  maxLength: 512,
                  maxLines: 1,
                  textInputAction: TextInputAction.search,
                  onSubmitted: (_) => _searchArchive(),
                  decoration: InputDecoration(
                    hintText: l10n.conversationMemorySearchHint,
                    counterText: '',
                    prefixIcon: const Icon(Icons.search_rounded),
                  ),
                ),
              ),
              const SizedBox(width: 8),
              FilledButton.tonal(
                onPressed: _isSearching ? null : _searchArchive,
                child: _isSearching
                    ? const SizedBox.square(
                        dimension: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : Text(l10n.conversationMemorySearchAction),
              ),
            ],
          ),
          if (_searchQueryTooShort) ...[
            const SizedBox(height: 6),
            Text(
              l10n.conversationMemorySearchQueryTooShort,
              style: detailStyle,
            ),
          ],
          const SizedBox(height: 10),
          SizedBox(height: 320, child: _buildSearchResults(context)),
          if (_resetFailed) ...[
            const SizedBox(height: 12),
            Text(
              l10n.conversationMemoryResetFailed,
              style: TextStyle(color: Theme.of(context).colorScheme.error),
            ),
          ],
          if (_memoryState?.hasContext == true) ...[
            const SizedBox(height: 10),
            Align(
              alignment: Alignment.centerRight,
              child: TextButton.icon(
                onPressed: widget.isSending || _isResetting
                    ? null
                    : _confirmReset,
                icon: _isResetting
                    ? const SizedBox.square(
                        dimension: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.restart_alt_rounded),
                label: Text(l10n.conversationMemoryResetAction),
              ),
            ),
          ],
        ],
      ],
    );
  }

  Widget _buildSemanticSearchSection(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final titleStyle = Theme.of(context).textTheme.titleSmall;
    final detailStyle = Theme.of(context).textTheme.bodySmall
        ?.copyWith(color: palette.secondaryText);

    if (_semanticSearchReady == true) {
      return Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(Icons.auto_awesome_rounded, size: 18, color: palette.accent),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(l10n.conversationMemorySemanticReady, style: titleStyle),
                const SizedBox(height: 3),
                Text(
                  l10n.conversationMemorySemanticIndexNotice,
                  style: detailStyle,
                ),
              ],
            ),
          ),
        ],
      );
    }

    if (_semanticSearchReady == null && !_semanticPreparationFailed) {
      return Row(
        children: [
          const SizedBox.square(
            dimension: 16,
            child: CircularProgressIndicator(strokeWidth: 2),
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Text(
              l10n.conversationMemorySemanticChecking,
              style: detailStyle,
            ),
          ),
        ],
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Icon(
              Icons.auto_awesome_outlined,
              size: 18,
              color: palette.secondaryText,
            ),
            const SizedBox(width: 10),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(l10n.conversationMemorySemanticTitle, style: titleStyle),
                  const SizedBox(height: 3),
                  Text(
                    l10n.conversationMemorySemanticDescription,
                    style: detailStyle,
                  ),
                ],
              ),
            ),
            const SizedBox(width: 8),
            if (_isPreparingSemanticSearch &&
                (_semanticPreparationProgress == null ||
                    _semanticPreparationProgress?.phase ==
                        SemanticPreparationPhase.downloading))
              OutlinedButton(
                onPressed: _isCancellingSemanticPreparation
                    ? null
                    : _cancelSemanticPreparation,
                child: _isCancellingSemanticPreparation
                    ? const SizedBox.square(
                        dimension: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : Text(l10n.cancel),
              )
            else if (_isPreparingSemanticSearch)
              const OutlinedButton(
                onPressed: null,
                child: SizedBox.square(
                  dimension: 16,
                  child: CircularProgressIndicator(strokeWidth: 2),
                ),
              )
            else
              OutlinedButton(
                onPressed: widget.repository == null
                    ? null
                    : _prepareSemanticSearch,
                child: Text(
                  _semanticPreparationFailed
                      ? l10n.retry
                      : l10n.conversationMemorySemanticPrepare,
                ),
              ),
          ],
        ),
        if (_isPreparingSemanticSearch) ...[
          const SizedBox(height: 8),
          LinearProgressIndicator(
            value:
                _semanticPreparationProgress?.phase ==
                    SemanticPreparationPhase.downloading
                ? _semanticPreparationProgress!.fraction
                : null,
          ),
          const SizedBox(height: 4),
          Text(_semanticPreparationStatus(l10n), style: detailStyle),
        ] else ...[
          const SizedBox(height: 5),
          Text(
            _semanticPreparationFailed
                ? l10n.conversationMemorySemanticPrepareFailed
                : _semanticPreparationCancelled
                ? l10n.conversationMemorySemanticDownloadCancelled
                : l10n.conversationMemorySemanticKeywordSearchFallback,
            style: detailStyle,
          ),
        ],
      ],
    );
  }

  String _semanticPreparationStatus(AppLocalizations l10n) {
    if (_isCancellingSemanticPreparation) {
      return l10n.conversationMemorySemanticCancelling;
    }
    final progress = _semanticPreparationProgress;
    return switch (progress?.phase) {
      SemanticPreparationPhase.downloading =>
        l10n.conversationMemorySemanticDownloadProgress(
          progress!.percentage.toString(),
          (progress.downloadedBytes / 1000000).toStringAsFixed(1),
          (progress.totalBytes / 1000000).toStringAsFixed(1),
        ),
      SemanticPreparationPhase.indexing =>
        l10n.conversationMemorySemanticIndexing,
      SemanticPreparationPhase.ready => l10n.conversationMemorySemanticReady,
      null => l10n.conversationMemorySemanticPreparing,
    };
  }

  Widget _buildSummarySection(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    if (_isLoadingMemory) {
      return const LinearProgressIndicator();
    }
    if (_loadFailed) {
      return Row(
        children: [
          Expanded(child: Text(l10n.conversationMemoryLoadFailed)),
          if (widget.repository != null)
            TextButton(onPressed: _loadMemory, child: Text(l10n.retry)),
        ],
      );
    }

    final state = _memoryState;
    final summary = state?.summary;
    final summaryBody = switch (state?.compactionKind) {
      'summary' when summary != null => SelectableText(summary),
      'responses_checkpoint' => Text(
        l10n.conversationMemoryCheckpointDescription,
      ),
      _ => Text(
        l10n.conversationMemoryNoSummary,
        style: TextStyle(color: palette.secondaryText),
      ),
    };
    final lastPrompt = state?.lastPrompt;

    return Container(
      decoration: BoxDecoration(
        color: palette.composer,
        border: Border.all(color: palette.border),
        borderRadius: BorderRadius.circular(12),
      ),
      padding: const EdgeInsets.all(14),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(
            l10n.conversationMemorySummaryTitle,
            style: Theme.of(context).textTheme.titleSmall,
          ),
          const SizedBox(height: 8),
          ConstrainedBox(
            constraints: const BoxConstraints(maxHeight: 150),
            child: SingleChildScrollView(child: summaryBody),
          ),
          if (lastPrompt != null) ...[
            const SizedBox(height: 10),
            Text(
              l10n.conversationMemoryLastPromptTokens(
                _providerLabel(l10n, lastPrompt.providerId),
                lastPrompt.modelId,
                lastPrompt.inputTokens.toString(),
              ),
              style: Theme.of(context).textTheme.bodySmall
                  ?.copyWith(color: palette.secondaryText),
            ),
          ],
        ],
      ),
    );
  }

  Widget _buildSearchResults(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    if (_searchFailed) {
      return Text(
        l10n.conversationMemorySearchFailed,
        style: TextStyle(color: Theme.of(context).colorScheme.error),
      );
    }
    if (!_hasSearched) {
      return Text(
        l10n.conversationMemorySearchInstruction,
        style: TextStyle(color: palette.secondaryText),
      );
    }
    if (_isSearching) {
      return const Center(child: CircularProgressIndicator());
    }
    if (_searchResults.isEmpty) {
      return Text(
        l10n.conversationMemorySearchNoResults,
        style: TextStyle(color: palette.secondaryText),
      );
    }

    return ListView.separated(
      itemCount: _searchResults.length,
      separatorBuilder: (_, _) => const Divider(height: 18),
      itemBuilder: (context, index) {
        final result = _searchResults[index];
        final createdAt = DateTime.fromMillisecondsSinceEpoch(
          result.createdAtUnixMs,
        ).toLocal();
        final dateLabel = MaterialLocalizations.of(context)
            .formatShortDate(createdAt);
        final timeLabel = MaterialLocalizations.of(context)
            .formatTimeOfDay(TimeOfDay.fromDateTime(createdAt));
        final roleLabel = result.role == 'user'
            ? l10n.conversationMemoryUserMessage
            : l10n.conversationMemoryAssistantMessage;
        return Column(
          key: ValueKey<String>(result.messageId),
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(
              '$roleLabel · $dateLabel · $timeLabel',
              style: Theme.of(context).textTheme.labelSmall
                  ?.copyWith(color: palette.secondaryText),
            ),
            const SizedBox(height: 5),
            SelectableText.rich(_highlightMatches(result.content, context)),
          ],
        );
      },
    );
  }
}

String _providerLabel(AppLocalizations l10n, String providerId) =>
    switch (providerId) {
      'chatgpt' || 'chatgpt_api' => l10n.chatGptProvider,
      'opencode' => l10n.openCodeProvider,
      'gemini' => l10n.geminiProvider,
      'groq' => l10n.groqProvider,
      'cerebras' => l10n.cerebrasProvider,
      'openrouter' => l10n.openRouterProvider,
      'mistral' => l10n.mistralProvider,
      _ => providerId,
    };

TextSpan _highlightMatches(String content, BuildContext context) {
  const openingMarker = '[match]';
  const closingMarker = '[/match]';
  final textStyle = Theme.of(context).textTheme.bodyMedium;
  final spans = <InlineSpan>[];
  var cursor = 0;
  while (cursor < content.length) {
    final opening = content.indexOf(openingMarker, cursor);
    if (opening < 0) {
      spans.add(TextSpan(text: content.substring(cursor)));
      break;
    }
    if (opening > cursor) {
      spans.add(TextSpan(text: content.substring(cursor, opening)));
    }
    final matchStart = opening + openingMarker.length;
    final closing = content.indexOf(closingMarker, matchStart);
    if (closing < 0) {
      spans.add(TextSpan(text: content.substring(matchStart)));
      break;
    }
    spans.add(
      TextSpan(
        text: content.substring(matchStart, closing),
        style: const TextStyle(fontWeight: FontWeight.w700),
      ),
    );
    cursor = closing + closingMarker.length;
  }
  return TextSpan(style: textStyle, children: spans);
}

void _reportMemoryError(
  Object error,
  StackTrace stackTrace,
  String description,
) {
  FlutterError.reportError(
    FlutterErrorDetails(
      exception: error,
      stack: stackTrace,
      library: 'conversation_memory',
      context: ErrorDescription(description),
    ),
  );
}
