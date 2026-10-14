import 'dart:convert';
import 'dart:typed_data';
import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_code_editor/flutter_code_editor.dart';
import 'package:flutter_widget_from_html_core/flutter_widget_from_html_core.dart';
import 'package:file_picker/file_picker.dart';
import 'package:intl/intl.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_page_header.dart';
import 'package:openchat/app/safe_markdown.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/openchat_toast.dart';
import 'package:openchat/features/chat/data/chat_repository.dart';
import 'package:openchat/features/chat/domain/chat_saved_output.dart';
import 'package:openchat/features/chat/presentation/widgets/assistant_message.dart';
import 'package:openchat/features/chat/presentation/widgets/saved_response_source_editor.dart';
import 'package:openchat/features/chat/presentation/conversation_markdown_export.dart';
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
                            onEdit: () => unawaited(_openWorkspace(output)),
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

  Future<void> _openWorkspace(ChatSavedOutput output) async {
    final repository = widget.repository;
    if (repository == null) return;
    await showDialog<void>(
      context: context,
      builder: (context) =>
          _SavedOutputWorkspaceDialog(repository: repository, output: output),
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
    required this.onEdit,
    required this.onRemove,
  });

  final ChatSavedOutput output;
  final VoidCallback onOpen;
  final VoidCallback onEdit;
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
        border: Border.all(color: palette.border),
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
                    onPressed: widget.onEdit,
                    icon: const Icon(LucideIcons.filePenLine),
                    label: Text(l10n.outputWorkspaceOpen),
                  ),
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

class _SavedOutputWorkspaceDialog extends StatefulWidget {
  const _SavedOutputWorkspaceDialog({
    required this.repository,
    required this.output,
  });

  final ChatRepository repository;
  final ChatSavedOutput output;

  @override
  State<_SavedOutputWorkspaceDialog> createState() =>
      _SavedOutputWorkspaceDialogState();
}

class _SavedOutputWorkspaceDialogState
    extends State<_SavedOutputWorkspaceDialog> {
  late final CodeController _contentController;
  final Map<int, String> _drafts = <int, String>{};
  late int _selectedVersion;
  String _editorLanguageId = 'markdown';
  bool _isSaving = false;
  bool _isExporting = false;
  bool _showPreview = false;

  @override
  void initState() {
    super.initState();
    _selectedVersion = widget.output.selectedVersion;
    _contentController = createSavedResponseSourceController(
      widget.output.content,
    );
  }

  @override
  void dispose() {
    _contentController.dispose();
    super.dispose();
  }

  bool _isDirty(List<ChatSavedOutputVersion> versions) {
    final current = _versionFor(versions, _selectedVersion);
    return current != null && _contentController.text != current.content;
  }

  ChatSavedOutputVersion? _versionFor(
    List<ChatSavedOutputVersion> versions,
    int versionNumber,
  ) {
    for (final version in versions) {
      if (version.versionNumber == versionNumber) return version;
    }
    return null;
  }

  Future<void> _selectVersion(ChatSavedOutputVersion target) async {
    if (target.versionNumber == _selectedVersion) return;
    _drafts[_selectedVersion] = _contentController.text;
    try {
      await widget.repository.selectSavedOutputVersion(
        conversationId: widget.output.conversationId,
        messageId: widget.output.messageId,
        versionNumber: target.versionNumber,
      );
      if (!mounted) return;
      setState(() {
        _selectedVersion = target.versionNumber;
        _contentController.text =
            _drafts[target.versionNumber] ?? target.content;
      });
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'outputs',
          context: ErrorDescription('while selecting a response version'),
        ),
      );
      if (mounted) {
        showOpenChatToast(
          context,
          context.openchatL10n.outputWorkspaceVersionsFailed,
          type: OpenChatToastType.error,
        );
      }
    }
  }

  Future<void> _saveVersion() async {
    if (_isSaving) return;
    setState(() => _isSaving = true);
    try {
      final versionNumber = await widget.repository.createSavedOutputVersion(
        conversationId: widget.output.conversationId,
        messageId: widget.output.messageId,
        content: _contentController.text,
      );
      if (!mounted) return;
      _drafts.clear();
      setState(() => _selectedVersion = versionNumber);
      showOpenChatToast(
        context,
        context.openchatL10n.outputWorkspaceVersion(versionNumber),
        type: OpenChatToastType.success,
      );
    } on InvalidSavedOutputVersionException {
      if (mounted) {
        showOpenChatToast(
          context,
          context.openchatL10n.outputWorkspaceEmpty,
          type: OpenChatToastType.error,
        );
      }
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'outputs',
          context: ErrorDescription('while saving a response version'),
        ),
      );
      if (mounted) {
        showOpenChatToast(
          context,
          context.openchatL10n.outputWorkspaceSaveFailed,
          type: OpenChatToastType.error,
        );
      }
    } finally {
      if (mounted) setState(() => _isSaving = false);
    }
  }

  Future<void> _exportVersion(ChatSavedOutputVersion version) async {
    if (_isExporting) return;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) {
        final l10n = context.openchatL10n;
        final palette = OpenChatPalette.of(context);
        final dialogWidth =
            (MediaQuery.sizeOf(context).width - OpenChatSpacing.xl * 2)
                .clamp(0.0, 520.0)
                .toDouble();
        return AlertDialog(
          title: Text(l10n.outputExportReviewTitle),
          content: SizedBox(
            width: dialogWidth,
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(l10n.outputExportReviewScope(version.versionNumber)),
                const SizedBox(height: OpenChatSpacing.sm),
                Container(
                  constraints: const BoxConstraints(maxHeight: 280),
                  width: double.infinity,
                  padding: const EdgeInsets.all(OpenChatSpacing.sm),
                  decoration: BoxDecoration(
                    color: palette.surface,
                    borderRadius: BorderRadius.circular(OpenChatRadii.card),
                    border: Border.all(color: palette.border),
                  ),
                  child: SingleChildScrollView(
                    child: SelectableText(version.content),
                  ),
                ),
                const SizedBox(height: OpenChatSpacing.sm),
                Text(
                  l10n.outputExportReviewSensitiveNotice,
                  style: Theme.of(context).textTheme.bodySmall
                      ?.copyWith(color: palette.secondaryText),
                ),
              ],
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(context).pop(false),
              child: Text(l10n.cancel),
            ),
            FilledButton(
              onPressed: () => Navigator.of(context).pop(true),
              child: Text(l10n.continueLabel),
            ),
          ],
        );
      },
    );
    if (confirmed != true || !mounted) return;
    setState(() => _isExporting = true);
    try {
      final l10n = context.openchatL10n;
      final savedPath = await FilePicker.saveFile(
        fileName: conversationExportFileName(
          '${widget.output.conversationTitle}-response-v${version.versionNumber}',
        ),
        bytes: Uint8List.fromList(utf8.encode(version.content)),
        mimeType: 'text/markdown',
        dialogTitle: l10n.outputWorkspaceExport,
        type: FileType.custom,
        allowedExtensions: const <String>['md'],
        windowsOptions: const WindowsOptions(lockParentWindow: true),
        linuxOptions: const LinuxOptions(lockParentWindow: true),
      );
      if (savedPath == null || !mounted) return;
      showOpenChatToast(
        context,
        context.openchatL10n.outputWorkspaceExported,
        type: OpenChatToastType.success,
      );
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'outputs',
          context: ErrorDescription('while exporting a response version'),
        ),
      );
      if (mounted) {
        showOpenChatToast(
          context,
          context.openchatL10n.outputWorkspaceExportFailed,
          type: OpenChatToastType.error,
        );
      }
    } finally {
      if (mounted) setState(() => _isExporting = false);
    }
  }

  Future<void> _close(List<ChatSavedOutputVersion> versions) async {
    if (_isDirty(versions)) {
      final l10n = context.openchatL10n;
      final shouldDiscard = await showDialog<bool>(
        context: context,
        builder: (context) => AlertDialog(
          title: Text(l10n.outputWorkspaceUnsavedTitle),
          content: Text(l10n.outputWorkspaceUnsavedBody),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(context).pop(false),
              child: Text(l10n.outputWorkspaceKeepEditing),
            ),
            FilledButton(
              onPressed: () => Navigator.of(context).pop(true),
              child: Text(l10n.outputWorkspaceDiscard),
            ),
          ],
        ),
      );
      if (shouldDiscard != true || !mounted) return;
    }
    if (mounted) Navigator.of(context).pop();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final size = MediaQuery.sizeOf(context);
    final compact = size.width < 640;
    return Dialog(
      alignment: compact ? Alignment.center : Alignment.centerRight,
      insetPadding: compact ? EdgeInsets.zero : const EdgeInsets.all(24),
      child: SizedBox(
        width: compact ? size.width : 900,
        height: compact ? size.height : size.height * 0.92,
        child: StreamBuilder<List<ChatSavedOutputVersion>>(
          stream: widget.repository.watchSavedOutputVersions(
            conversationId: widget.output.conversationId,
            messageId: widget.output.messageId,
          ),
          builder: (context, snapshot) {
            final versions = snapshot.data ?? const <ChatSavedOutputVersion>[];
            final selected = _versionFor(versions, _selectedVersion);
            final dirty = _isDirty(versions);
            return Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Padding(
                  padding: const EdgeInsets.fromLTRB(
                    OpenChatSpacing.lg,
                    OpenChatSpacing.md,
                    OpenChatSpacing.md,
                    OpenChatSpacing.md,
                  ),
                  child: Row(
                    children: [
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              l10n.outputWorkspaceTitle,
                              style: Theme.of(context).textTheme.titleLarge,
                            ),
                            const SizedBox(height: OpenChatSpacing.xxs),
                            Text(
                              l10n.outputWorkspaceDescription,
                              style: Theme.of(context).textTheme.bodySmall
                                  ?.copyWith(color: palette.secondaryText),
                            ),
                          ],
                        ),
                      ),
                      IconButton(
                        tooltip: MaterialLocalizations.of(context)
                            .closeButtonTooltip,
                        onPressed: snapshot.hasData
                            ? () => unawaited(_close(versions))
                            : null,
                        icon: const Icon(LucideIcons.x),
                      ),
                    ],
                  ),
                ),
                const Divider(height: 1),
                if (snapshot.hasError)
                  Expanded(
                    child: Center(
                      child: Text(l10n.outputWorkspaceVersionsFailed),
                    ),
                  )
                else if (!snapshot.hasData)
                  const Expanded(
                    child: Center(child: CircularProgressIndicator()),
                  )
                else if (versions.isEmpty || selected == null)
                  Expanded(
                    child: Center(
                      child: Padding(
                        padding: const EdgeInsets.all(OpenChatSpacing.lg),
                        child: Text(l10n.outputWorkspaceVersionsFailed),
                      ),
                    ),
                  )
                else
                  Expanded(
                    child: Padding(
                      padding: const EdgeInsets.all(OpenChatSpacing.lg),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          Row(
                            children: [
                              Expanded(
                                child: DropdownButtonFormField<int>(
                                  key: ValueKey<int>(_selectedVersion),
                                  initialValue: _selectedVersion,
                                  decoration: InputDecoration(
                                    labelText: l10n.outputWorkspaceVersion(
                                      _selectedVersion,
                                    ),
                                  ),
                                  items: [
                                    for (final version in versions)
                                      DropdownMenuItem<int>(
                                        value: version.versionNumber,
                                        child: Text(
                                          l10n.outputWorkspaceVersion(
                                            version.versionNumber,
                                          ),
                                        ),
                                      ),
                                  ],
                                  onChanged: _isSaving || _isExporting
                                      ? null
                                      : (number) {
                                          if (number == null) return;
                                          final target = _versionFor(
                                            versions,
                                            number,
                                          );
                                          if (target != null) {
                                            unawaited(_selectVersion(target));
                                          }
                                        },
                                ),
                              ),
                              const SizedBox(width: OpenChatSpacing.sm),
                              IconButton.outlined(
                                tooltip: l10n.outputWorkspaceExport,
                                onPressed: _isExporting
                                    ? null
                                    : () => unawaited(_exportVersion(selected)),
                                icon: _isExporting
                                    ? const SizedBox.square(
                                        dimension: 18,
                                        child: CircularProgressIndicator(
                                          strokeWidth: 2,
                                        ),
                                      )
                                    : const Icon(LucideIcons.download),
                              ),
                            ],
                          ),
                          const SizedBox(height: OpenChatSpacing.md),
                          SegmentedButton<bool>(
                            segments: [
                              ButtonSegment<bool>(
                                value: false,
                                icon: const Icon(LucideIcons.pencilLine),
                                label: Text(l10n.outputWorkspaceEdit),
                              ),
                              ButtonSegment<bool>(
                                value: true,
                                icon: const Icon(LucideIcons.eye),
                                label: Text(l10n.outputWorkspacePreview),
                              ),
                            ],
                            selected: <bool>{_showPreview},
                            onSelectionChanged: (values) => setState(
                              () => _showPreview = values.contains(true),
                            ),
                          ),
                          const SizedBox(height: OpenChatSpacing.sm),
                          Expanded(
                            child: _showPreview
                                ? SingleChildScrollView(
                                    padding: const EdgeInsets.all(
                                      OpenChatSpacing.md,
                                    ),
                                    child: HtmlWidget(
                                      markdownToSafeHtml(
                                        _contentController.text,
                                      ),
                                      textStyle: Theme.of(context)
                                          .textTheme
                                          .bodyMedium,
                                    ),
                                  )
                                : SavedResponseSourceEditor(
                                    controller: _contentController,
                                    languageId: _editorLanguageId,
                                    sourceLabel: l10n.outputWorkspaceSource,
                                    languageLabel: l10n.outputWorkspaceLanguage,
                                    onLanguageChanged: (languageId) => setState(
                                      () => _editorLanguageId = languageId,
                                    ),
                                    onChanged: (_) => setState(() {}),
                                  ),
                          ),
                          const SizedBox(height: OpenChatSpacing.md),
                          Wrap(
                            alignment: WrapAlignment.end,
                            spacing: OpenChatSpacing.sm,
                            runSpacing: OpenChatSpacing.sm,
                            children: [
                              TextButton(
                                onPressed: snapshot.hasData
                                    ? () => unawaited(_close(versions))
                                    : null,
                                child: Text(l10n.cancel),
                              ),
                              FilledButton.icon(
                                onPressed: !dirty || _isSaving
                                    ? null
                                    : () => unawaited(_saveVersion()),
                                icon: _isSaving
                                    ? const SizedBox.square(
                                        dimension: 18,
                                        child: CircularProgressIndicator(
                                          strokeWidth: 2,
                                        ),
                                      )
                                    : const Icon(LucideIcons.save),
                                label: Text(l10n.outputWorkspaceSaveVersion),
                              ),
                            ],
                          ),
                        ],
                      ),
                    ),
                  ),
              ],
            );
          },
        ),
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
    'deepseek' => l10n.deepSeekProvider,
    'xai' => l10n.xAiProvider,
    'perplexity' => l10n.perplexityProvider,
    'together' => l10n.togetherProvider,
    'sambanova' => l10n.sambaNovaProvider,
    'anthropic' => l10n.anthropicProvider,
    'fireworks' => l10n.fireworksProvider,
    'deepinfra' => l10n.deepInfraProvider,
    'dashscope' => l10n.dashScopeProvider,
    'cohere' => l10n.cohereProvider,
    'llama_cpp' => l10n.localModelsPageTitle,
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
        border: Border.all(color: palette.border),
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
