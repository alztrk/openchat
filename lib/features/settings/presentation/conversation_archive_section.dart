import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:intl/intl.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/openchat_toast.dart';
import 'package:openchat/features/chat/data/chat_repository.dart';
import 'package:openchat/features/chat/domain/chat_conversation.dart';
import 'package:openchat/features/settings/data/conversation_archive_repository.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

class ConversationArchiveSection extends StatefulWidget {
  const ConversationArchiveSection({
    required this.serviceClient,
    required this.chatRepository,
    super.key,
  });

  final OpenChatServiceClient? serviceClient;
  final ChatRepository? chatRepository;

  @override
  State<ConversationArchiveSection> createState() =>
      _ConversationArchiveSectionState();
}

class _ConversationArchiveSectionState
    extends State<ConversationArchiveSection> {
  bool _isWorking = false;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final enabled =
        !_isWorking &&
        widget.serviceClient != null &&
        widget.chatRepository != null;

    return Container(
      padding: const EdgeInsets.all(18),
      decoration: BoxDecoration(
        color: palette.composer,
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: palette.border),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Icon(Icons.enhanced_encryption_outlined, color: palette.accent),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      l10n.conversationArchiveTitle,
                      style: Theme.of(context).textTheme.titleSmall,
                    ),
                    const SizedBox(height: 4),
                    Text(
                      l10n.conversationArchiveDescription,
                      style: Theme.of(context).textTheme.bodySmall
                          ?.copyWith(color: palette.secondaryText),
                    ),
                    const SizedBox(height: 8),
                    Text(
                      l10n.conversationArchiveIncludesNotice,
                      style: Theme.of(context).textTheme.bodySmall
                          ?.copyWith(color: palette.secondaryText),
                    ),
                  ],
                ),
              ),
            ],
          ),
          const SizedBox(height: 16),
          Wrap(
            spacing: 10,
            runSpacing: 10,
            children: [
              OutlinedButton.icon(
                onPressed: enabled
                    ? () => unawaited(_exportConversations())
                    : null,
                icon: const Icon(Icons.archive_outlined, size: 18),
                label: Text(l10n.exportConversations),
              ),
              OutlinedButton.icon(
                onPressed: enabled
                    ? () => unawaited(_importConversations())
                    : null,
                icon: const Icon(Icons.unarchive_outlined, size: 18),
                label: Text(l10n.importConversations),
              ),
            ],
          ),
          if (widget.serviceClient == null ||
              widget.chatRepository == null) ...[
            const SizedBox(height: 12),
            Text(
              l10n.conversationArchiveUnavailable,
              style: Theme.of(context).textTheme.bodySmall
                  ?.copyWith(color: palette.secondaryText),
            ),
          ],
          if (_isWorking) ...[
            const SizedBox(height: 14),
            const LinearProgressIndicator(),
            const SizedBox(height: 8),
            Text(
              l10n.conversationArchiveProcessing,
              style: Theme.of(context).textTheme.bodySmall
                  ?.copyWith(color: palette.secondaryText),
            ),
          ],
        ],
      ),
    );
  }

  Future<void> _exportConversations() async {
    final service = widget.serviceClient;
    final chatRepository = widget.chatRepository;
    if (_isWorking || service == null || chatRepository == null) return;
    setState(() => _isWorking = true);
    try {
      final conversations = await chatRepository.watchConversations().first;
      if (!mounted) return;
      if (conversations.isEmpty) {
        showOpenChatToast(
          context,
          context.openchatL10n.conversationArchiveNoConversations,
          type: OpenChatToastType.info,
        );
        return;
      }
      final selection = await showDialog<_ArchiveExportRequest>(
        context: context,
        builder: (dialogContext) =>
            _ConversationArchiveExportDialog(conversations: conversations),
      );
      if (!mounted || selection == null) return;

      final directory = await FilePicker.getDirectoryPath(
        dialogTitle: context.openchatL10n.conversationArchiveChooseFolder,
        windowsOptions: const WindowsOptions(lockParentWindow: true),
      );
      if (!mounted || directory == null || directory.trim().isEmpty) return;

      final fileName =
          'OpenChat-${DateTime.now().toUtc().microsecondsSinceEpoch}.openchatbackup';
      final path = Directory(directory).uri.resolve(fileName).toFilePath();
      final result = await ConversationArchiveRepository(service).export(
        conversationIds: selection.conversationIds,
        path: path,
        passphrase: selection.passphrase,
      );
      if (!mounted) return;
      showOpenChatToast(
        context,
        context.openchatL10n.conversationArchiveExportSuccess(
          result.conversationCount,
        ),
        type: OpenChatToastType.success,
      );
    } on OpenChatServiceException catch (error) {
      _showFailure(_archiveErrorMessage(context, error.code));
    } on PlatformException {
      _showFailure(context.openchatL10n.conversationArchivePickerFailed);
    } on FormatException {
      _showFailure(context.openchatL10n.conversationArchiveInvalidResponse);
    } on Object catch (error, stackTrace) {
      _reportUnexpectedError(
        error,
        stackTrace,
        'while exporting conversations',
      );
      _showFailure(context.openchatL10n.conversationArchiveExportFailed);
    } finally {
      if (mounted) setState(() => _isWorking = false);
    }
  }

  Future<void> _importConversations() async {
    final service = widget.serviceClient;
    if (_isWorking || service == null) return;
    setState(() => _isWorking = true);
    try {
      final selectedFile = await FilePicker.pickFile(
        dialogTitle: context.openchatL10n.conversationArchiveChooseFile,
        type: FileType.custom,
        allowedExtensions: const ['openchatbackup'],
        windowsOptions: const WindowsOptions(lockParentWindow: true),
      );
      if (!mounted || selectedFile == null) return;
      final path = selectedFile.path;
      if (path == null || path.trim().isEmpty) {
        _showFailure(context.openchatL10n.conversationArchiveInvalidFile);
        return;
      }

      final passphrase = await showDialog<String>(
        context: context,
        builder: (dialogContext) =>
            const _ConversationArchivePassphraseDialog(),
      );
      if (!mounted || passphrase == null) return;

      final repository = ConversationArchiveRepository(service);
      final summary = await repository.inspect(
        path: path,
        passphrase: passphrase,
      );
      if (!mounted) return;
      final choice = await showDialog<ConversationArchiveConflictPolicy>(
        context: context,
        builder: (dialogContext) =>
            _ConversationArchiveRestoreDialog(summary: summary),
      );
      if (!mounted || choice == null) return;

      final result = await repository.restore(
        path: path,
        passphrase: passphrase,
        conflictPolicy: choice,
      );
      if (!mounted) return;
      showOpenChatToast(
        context,
        context.openchatL10n.conversationArchiveImportSuccess(
          result.conversationCount,
        ),
        type: OpenChatToastType.success,
      );
    } on OpenChatServiceException catch (error) {
      _showFailure(_archiveErrorMessage(context, error.code));
    } on PlatformException {
      _showFailure(context.openchatL10n.conversationArchivePickerFailed);
    } on FormatException {
      _showFailure(context.openchatL10n.conversationArchiveInvalidResponse);
    } on Object catch (error, stackTrace) {
      _reportUnexpectedError(
        error,
        stackTrace,
        'while importing conversations',
      );
      _showFailure(context.openchatL10n.conversationArchiveImportFailed);
    } finally {
      if (mounted) setState(() => _isWorking = false);
    }
  }

  void _showFailure(String message) {
    if (!mounted) return;
    showOpenChatToast(context, message, type: OpenChatToastType.error);
  }

  void _reportUnexpectedError(
    Object error,
    StackTrace stackTrace,
    String context,
  ) {
    FlutterError.reportError(
      FlutterErrorDetails(
        exception: error,
        stack: stackTrace,
        library: 'conversation archive',
        context: ErrorDescription(context),
      ),
    );
  }
}

class _ArchiveExportRequest {
  const _ArchiveExportRequest(this.conversationIds, this.passphrase);

  final List<String> conversationIds;
  final String passphrase;
}

class _ConversationArchiveExportDialog extends StatefulWidget {
  const _ConversationArchiveExportDialog({required this.conversations});

  final List<ChatConversation> conversations;

  @override
  State<_ConversationArchiveExportDialog> createState() =>
      _ConversationArchiveExportDialogState();
}

class _ConversationArchiveExportDialogState
    extends State<_ConversationArchiveExportDialog> {
  final _formKey = GlobalKey<FormState>();
  final _searchController = TextEditingController();
  final _passphraseController = TextEditingController();
  final _confirmPassphraseController = TextEditingController();
  final Set<String> _selectedIds = <String>{};
  String _search = '';

  @override
  void dispose() {
    _searchController.dispose();
    _passphraseController.dispose();
    _confirmPassphraseController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final locale = Localizations.localeOf(context).toString();
    final dateFormat = DateFormat.yMMMd(locale).add_jm();
    final filtered = widget.conversations
        .where(
          (conversation) =>
              conversation.title.toLowerCase().contains(_search.toLowerCase()),
        )
        .toList(growable: false);
    final allFilteredSelected =
        filtered.isNotEmpty &&
        filtered.every((item) => _selectedIds.contains(item.id));

    return AlertDialog(
      title: Text(l10n.conversationArchiveSelectTitle),
      content: SizedBox(
        width: 560,
        child: Form(
          key: _formKey,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(
                l10n.conversationArchiveSelectedCount(_selectedIds.length),
                style: Theme.of(context).textTheme.bodySmall
                    ?.copyWith(color: palette.secondaryText),
              ),
              const SizedBox(height: 10),
              TextField(
                controller: _searchController,
                onChanged: (value) => setState(() => _search = value.trim()),
                decoration: InputDecoration(
                  prefixIcon: const Icon(Icons.search_rounded),
                  hintText: l10n.conversationArchiveSearch,
                  isDense: true,
                ),
              ),
              Align(
                alignment: Alignment.centerRight,
                child: TextButton(
                  onPressed: filtered.isEmpty
                      ? null
                      : () => setState(() {
                          if (allFilteredSelected) {
                            _selectedIds.removeAll(
                              filtered.map((item) => item.id),
                            );
                          } else {
                            _selectedIds.addAll(
                              filtered.map((item) => item.id),
                            );
                          }
                        }),
                  child: Text(
                    allFilteredSelected
                        ? l10n.conversationArchiveDeselectAll
                        : l10n.conversationArchiveSelectAll,
                  ),
                ),
              ),
              Container(
                height: 240,
                decoration: BoxDecoration(
                  border: Border.all(color: palette.border),
                  borderRadius: BorderRadius.circular(8),
                ),
                child: filtered.isEmpty
                    ? Center(child: Text(l10n.conversationArchiveNoMatches))
                    : ListView.builder(
                        itemCount: filtered.length,
                        itemBuilder: (context, index) {
                          final conversation = filtered[index];
                          final selected = _selectedIds.contains(
                            conversation.id,
                          );
                          return CheckboxListTile(
                            key: ValueKey(conversation.id),
                            value: selected,
                            dense: true,
                            title: Text(
                              conversation.title,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                            ),
                            subtitle: Text(
                              dateFormat.format(conversation.updatedAt),
                            ),
                            onChanged: (value) => setState(() {
                              if (value == true) {
                                _selectedIds.add(conversation.id);
                              } else {
                                _selectedIds.remove(conversation.id);
                              }
                            }),
                            controlAffinity: ListTileControlAffinity.leading,
                          );
                        },
                      ),
              ),
              const SizedBox(height: 14),
              TextFormField(
                controller: _passphraseController,
                obscureText: true,
                enableSuggestions: false,
                autocorrect: false,
                validator: (value) => _validatePassphrase(value, l10n),
                decoration: InputDecoration(
                  labelText: l10n.conversationArchivePassphrase,
                  hintText: l10n.conversationArchivePassphraseHint,
                ),
              ),
              const SizedBox(height: 10),
              TextFormField(
                controller: _confirmPassphraseController,
                obscureText: true,
                enableSuggestions: false,
                autocorrect: false,
                validator: (value) {
                  final baseError = _validatePassphrase(value, l10n);
                  if (baseError != null) return baseError;
                  if (value != _passphraseController.text) {
                    return l10n.conversationArchivePassphraseMismatch;
                  }
                  return null;
                },
                decoration: InputDecoration(
                  labelText: l10n.conversationArchiveConfirmPassphrase,
                ),
              ),
              const SizedBox(height: 8),
              Text(
                l10n.conversationArchivePassphraseRecovery,
                style: Theme.of(context).textTheme.bodySmall
                    ?.copyWith(color: palette.secondaryText),
              ),
            ],
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
        FilledButton.icon(
          onPressed: _selectedIds.isEmpty
              ? null
              : () {
                  if (!_formKey.currentState!.validate()) return;
                  Navigator.of(context).pop(
                    _ArchiveExportRequest(
                      _selectedIds.toList(growable: false),
                      _passphraseController.text,
                    ),
                  );
                },
          icon: const Icon(Icons.lock_outline_rounded, size: 18),
          label: Text(l10n.exportConversations),
        ),
      ],
    );
  }
}

class _ConversationArchivePassphraseDialog extends StatefulWidget {
  const _ConversationArchivePassphraseDialog();

  @override
  State<_ConversationArchivePassphraseDialog> createState() =>
      _ConversationArchivePassphraseDialogState();
}

class _ConversationArchivePassphraseDialogState
    extends State<_ConversationArchivePassphraseDialog> {
  final _formKey = GlobalKey<FormState>();
  final _passphraseController = TextEditingController();

  @override
  void dispose() {
    _passphraseController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    return AlertDialog(
      title: Text(l10n.conversationArchivePassphraseTitle),
      content: SizedBox(
        width: 420,
        child: Form(
          key: _formKey,
          child: TextFormField(
            controller: _passphraseController,
            obscureText: true,
            enableSuggestions: false,
            autocorrect: false,
            autofocus: true,
            validator: (value) => _validatePassphrase(value, l10n),
            onFieldSubmitted: (_) => _submit(context),
            decoration: InputDecoration(
              labelText: l10n.conversationArchivePassphrase,
              hintText: l10n.conversationArchivePassphraseHint,
            ),
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          onPressed: () => _submit(context),
          child: Text(l10n.continueLabel),
        ),
      ],
    );
  }

  void _submit(BuildContext context) {
    if (!_formKey.currentState!.validate()) return;
    Navigator.of(context).pop(_passphraseController.text);
  }
}

class _ConversationArchiveRestoreDialog extends StatefulWidget {
  const _ConversationArchiveRestoreDialog({required this.summary});

  final ConversationArchiveSummary summary;

  @override
  State<_ConversationArchiveRestoreDialog> createState() =>
      _ConversationArchiveRestoreDialogState();
}

class _ConversationArchiveRestoreDialogState
    extends State<_ConversationArchiveRestoreDialog> {
  ConversationArchiveConflictPolicy _policy =
      ConversationArchiveConflictPolicy.skipExisting;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final locale = Localizations.localeOf(context).toString();
    final createdAt = DateTime.fromMillisecondsSinceEpoch(
      widget.summary.createdAtUnixMs,
      isUtc: true,
    ).toLocal();
    final dateFormat = DateFormat.yMMMd(locale).add_jm();

    return AlertDialog(
      title: Text(l10n.conversationArchivePreviewTitle),
      content: SizedBox(
        width: 520,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            _ArchiveSummaryRow(
              label: l10n.conversationArchiveCreatedAt,
              value: dateFormat.format(createdAt),
            ),
            _ArchiveSummaryRow(
              label: l10n.conversationArchiveConversationCount,
              value: '${widget.summary.conversationCount}',
            ),
            _ArchiveSummaryRow(
              label: l10n.conversationArchiveMessageCount,
              value: '${widget.summary.messageCount}',
            ),
            _ArchiveSummaryRow(
              label: l10n.conversationArchiveAttachmentCount,
              value:
                  '${widget.summary.attachmentCount} · ${_formatByteCount(widget.summary.attachmentBytes, locale)}',
            ),
            if (widget.summary.duplicateConversationCount > 0)
              _ArchiveSummaryRow(
                label: l10n.conversationArchiveDuplicateCount,
                value: '${widget.summary.duplicateConversationCount}',
                valueColor: Theme.of(context).colorScheme.tertiary,
              ),
            const SizedBox(height: 12),
            RadioGroup<ConversationArchiveConflictPolicy>(
              groupValue: _policy,
              onChanged: (value) {
                if (value != null) setState(() => _policy = value);
              },
              child: Column(
                children: [
                  RadioListTile<ConversationArchiveConflictPolicy>(
                    value: ConversationArchiveConflictPolicy.skipExisting,
                    title: Text(l10n.conversationArchiveSkipDuplicates),
                    dense: true,
                    contentPadding: EdgeInsets.zero,
                  ),
                  RadioListTile<ConversationArchiveConflictPolicy>(
                    value: ConversationArchiveConflictPolicy.importAsCopy,
                    title: Text(l10n.conversationArchiveImportCopies),
                    dense: true,
                    contentPadding: EdgeInsets.zero,
                  ),
                ],
              ),
            ),
            const SizedBox(height: 4),
            Text(
              l10n.conversationArchiveRestoreNotice,
              style: Theme.of(context).textTheme.bodySmall
                  ?.copyWith(color: palette.secondaryText),
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
        FilledButton.icon(
          onPressed: () => Navigator.of(context).pop(_policy),
          icon: const Icon(Icons.unarchive_outlined, size: 18),
          label: Text(l10n.importConversations),
        ),
      ],
    );
  }
}

class _ArchiveSummaryRow extends StatelessWidget {
  const _ArchiveSummaryRow({
    required this.label,
    required this.value,
    this.valueColor,
  });

  final String label;
  final String value;
  final Color? valueColor;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 5),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Expanded(
            child: Text(
              label,
              style: Theme.of(context).textTheme.bodySmall
                  ?.copyWith(color: palette.secondaryText),
            ),
          ),
          const SizedBox(width: 12),
          Flexible(
            child: Text(
              value,
              textAlign: TextAlign.end,
              style: Theme.of(context).textTheme.bodySmall?.copyWith(
                color: valueColor ?? palette.text,
                fontWeight: FontWeight.w600,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

String? _validatePassphrase(String? value, AppLocalizations l10n) {
  if (value == null || value.runes.length < 12) {
    return l10n.conversationArchivePassphraseTooShort;
  }
  if (utf8.encode(value).length > 512 || value.contains('\u0000')) {
    return l10n.conversationArchivePassphraseTooLong;
  }
  return null;
}

String _archiveErrorMessage(BuildContext context, String code) {
  final l10n = context.openchatL10n;
  return switch (code) {
    'conversation_archive_invalid' =>
      l10n.conversationArchiveInvalidPassphraseOrFile,
    'conversation_archive_passphrase_invalid' =>
      l10n.conversationArchivePassphraseInvalid,
    'conversation_archive_not_found' => l10n.conversationArchiveNotFound,
    'conversation_archive_conflict' => l10n.conversationArchiveConflict,
    'conversation_archive_busy' => l10n.conversationArchiveBusy,
    'conversation_archive_storage_failed' =>
      l10n.conversationArchiveStorageFailed,
    'conversation_archive_limit_exceeded' =>
      l10n.conversationArchiveLimitExceeded,
    'service_timeout' => l10n.conversationArchiveTakingLong,
    _ => l10n.conversationArchiveOperationFailed,
  };
}

String _formatByteCount(int bytes, String locale) {
  final formatter = NumberFormat.decimalPattern(locale);
  if (bytes < 1024) return '${formatter.format(bytes)} B';
  if (bytes < 1024 * 1024) {
    return '${formatter.format(bytes / 1024)} KB';
  }
  if (bytes < 1024 * 1024 * 1024) {
    return '${formatter.format(bytes / (1024 * 1024))} MB';
  }
  return '${formatter.format(bytes / (1024 * 1024 * 1024))} GB';
}
