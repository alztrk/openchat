import 'dart:async';
import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:intl/intl.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/openchat_toast.dart';
import 'package:openchat/features/chat/data/openchat_database.dart';
import 'package:openchat/features/settings/data/profile_archive_repository.dart';
import 'package:openchat/features/settings/presentation/archive_passphrase_dialog.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';
import 'package:openchat/platform/windows/window_controls.dart';

class ProfileArchiveSection extends StatefulWidget {
  const ProfileArchiveSection({required this.serviceClient, super.key});

  final OpenChatServiceClient? serviceClient;

  @override
  State<ProfileArchiveSection> createState() => _ProfileArchiveSectionState();
}

class _ProfileArchiveSectionState extends State<ProfileArchiveSection> {
  bool _isWorking = false;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final enabled = !_isWorking && widget.serviceClient != null;

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
              Icon(Icons.shield_outlined, color: palette.accent),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      l10n.profileArchiveTitle,
                      style: Theme.of(context).textTheme.titleSmall,
                    ),
                    const SizedBox(height: 4),
                    Text(
                      l10n.profileArchiveDescription,
                      style: Theme.of(context).textTheme.bodySmall
                          ?.copyWith(color: palette.secondaryText),
                    ),
                    const SizedBox(height: 8),
                    Text(
                      l10n.profileArchiveIncludesNotice,
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
                onPressed: enabled ? () => unawaited(_exportProfile()) : null,
                icon: const Icon(Icons.lock_outline_rounded, size: 18),
                label: Text(l10n.profileArchiveExport),
              ),
              OutlinedButton.icon(
                onPressed: enabled ? () => unawaited(_restoreProfile()) : null,
                icon: const Icon(Icons.restore_rounded, size: 18),
                label: Text(l10n.profileArchiveRestore),
              ),
            ],
          ),
          if (widget.serviceClient == null) ...[
            const SizedBox(height: 12),
            Text(
              l10n.profileArchiveUnavailable,
              style: Theme.of(context).textTheme.bodySmall
                  ?.copyWith(color: palette.secondaryText),
            ),
          ],
          if (_isWorking) ...[
            const SizedBox(height: 14),
            const LinearProgressIndicator(),
            const SizedBox(height: 8),
            Text(
              l10n.profileArchiveProcessing,
              style: Theme.of(context).textTheme.bodySmall
                  ?.copyWith(color: palette.secondaryText),
            ),
          ],
        ],
      ),
    );
  }

  Future<void> _exportProfile() async {
    final service = widget.serviceClient;
    if (_isWorking || service == null) return;
    setState(() => _isWorking = true);
    try {
      final directory = await FilePicker.getDirectoryPath(
        dialogTitle: context.openchatL10n.profileArchiveChooseFolder,
        windowsOptions: const WindowsOptions(lockParentWindow: true),
      );
      if (!mounted || directory == null || directory.trim().isEmpty) return;
      final passphrase = await showArchivePassphraseDialog(
        context,
        confirmPassphrase: true,
      );
      if (!mounted || passphrase == null) return;

      final fileName =
          'OpenChat-${DateTime.now().toUtc().microsecondsSinceEpoch}.'
          'openchatprofilebackup';
      final path = Directory(directory).uri.resolve(fileName).toFilePath();
      final result = await ProfileArchiveRepository(service)
          .export(path: path, passphrase: passphrase);
      if (!mounted) return;
      showOpenChatToast(
        context,
        context.openchatL10n.profileArchiveExportSuccess(
          _formatCount(result.conversationCount),
          _formatCount(result.messageCount),
          _formatCount(result.attachmentCount),
        ),
        type: OpenChatToastType.success,
      );
    } on OpenChatServiceException catch (error) {
      _showFailure(_profileArchiveErrorMessage(context, error.code));
    } on PlatformException {
      _showFailure(context.openchatL10n.profileArchivePickerFailed);
    } on FormatException {
      _showFailure(context.openchatL10n.profileArchiveInvalidResponse);
    } on Object catch (error, stackTrace) {
      _reportUnexpectedError(error, stackTrace, 'while exporting a profile');
      _showFailure(context.openchatL10n.profileArchiveExportFailed);
    } finally {
      if (mounted) setState(() => _isWorking = false);
    }
  }

  Future<void> _restoreProfile() async {
    final service = widget.serviceClient;
    if (_isWorking || service == null) return;
    setState(() => _isWorking = true);
    try {
      final selectedFile = await FilePicker.pickFile(
        dialogTitle: context.openchatL10n.profileArchiveChooseFile,
        type: FileType.custom,
        allowedExtensions: const ['openchatprofilebackup'],
        windowsOptions: const WindowsOptions(lockParentWindow: true),
      );
      if (!mounted || selectedFile == null) return;
      final path = selectedFile.path;
      if (path == null || path.trim().isEmpty) {
        _showFailure(context.openchatL10n.profileArchiveInvalidFile);
        return;
      }
      final passphrase = await showArchivePassphraseDialog(context);
      if (!mounted || passphrase == null) return;
      final confirmed = await showDialog<bool>(
        context: context,
        builder: (dialogContext) => const _ProfileRestoreConfirmationDialog(),
      );
      if (!mounted || confirmed != true) return;

      final result = await ProfileArchiveRepository(service).prepareRestore(
        path: path,
        passphrase: passphrase,
        chatSchemaVersion: OpenChatDatabase.currentSchemaVersion,
      );
      if (!mounted) return;
      await showDialog<void>(
        context: context,
        barrierDismissible: false,
        builder: (dialogContext) =>
            _ProfileRestoreReadyDialog(result: result, onClose: _closeApp),
      );
    } on OpenChatServiceException catch (error) {
      if (!mounted) return;
      _showFailure(_profileArchiveErrorMessage(context, error.code));
    } on PlatformException {
      if (!mounted) return;
      _showFailure(context.openchatL10n.profileArchivePickerFailed);
    } on FormatException {
      if (!mounted) return;
      _showFailure(context.openchatL10n.profileArchiveInvalidResponse);
    } on Object catch (error, stackTrace) {
      if (!mounted) return;
      _reportUnexpectedError(
        error,
        stackTrace,
        'while preparing a profile restore',
      );
      _showFailure(context.openchatL10n.profileArchiveRestoreFailed);
    } finally {
      if (mounted) setState(() => _isWorking = false);
    }
  }

  Future<void> _closeApp() async {
    try {
      await OpenChatWindowControls.close();
    } on PlatformException catch (error, stackTrace) {
      if (!mounted) return;
      _reportUnexpectedError(error, stackTrace, 'while closing after restore');
      _showFailure(context.openchatL10n.profileArchiveCloseFailed);
    }
  }

  String _formatCount(int count) {
    final locale = Localizations.localeOf(context).toString();
    return NumberFormat.decimalPattern(locale).format(count);
  }

  void _showFailure(String message) {
    if (!mounted) return;
    showOpenChatToast(context, message, type: OpenChatToastType.error);
  }

  void _reportUnexpectedError(
    Object error,
    StackTrace stackTrace,
    String contextDescription,
  ) {
    FlutterError.reportError(
      FlutterErrorDetails(
        exception: error,
        stack: stackTrace,
        library: 'profile archive',
        context: ErrorDescription(contextDescription),
      ),
    );
  }
}

class _ProfileRestoreConfirmationDialog extends StatelessWidget {
  const _ProfileRestoreConfirmationDialog();

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    return AlertDialog(
      icon: Icon(
        Icons.warning_amber_rounded,
        color: Theme.of(context).colorScheme.error,
      ),
      title: Text(l10n.profileArchiveRestoreConfirmTitle),
      content: SizedBox(
        width: 440,
        child: Text(l10n.profileArchiveRestoreConfirmBody),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(false),
          child: Text(l10n.cancel),
        ),
        FilledButton.icon(
          style: FilledButton.styleFrom(
            backgroundColor: Theme.of(context).colorScheme.error,
            foregroundColor: Theme.of(context).colorScheme.onError,
          ),
          onPressed: () => Navigator.of(context).pop(true),
          icon: const Icon(Icons.restore_rounded, size: 18),
          label: Text(l10n.profileArchiveRestoreConfirmButton),
        ),
      ],
    );
  }
}

class _ProfileRestoreReadyDialog extends StatelessWidget {
  const _ProfileRestoreReadyDialog({
    required this.result,
    required this.onClose,
  });

  final ProfileArchiveResult result;
  final Future<void> Function() onClose;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    return PopScope(
      canPop: false,
      child: AlertDialog(
        icon: const Icon(Icons.task_alt_rounded),
        title: Text(l10n.profileArchiveRestartTitle),
        content: SizedBox(
          width: 440,
          child: Text(
            l10n.profileArchiveRestoreReady(
              result.conversationCount,
              result.messageCount,
              result.attachmentCount,
            ),
          ),
        ),
        actions: [
          FilledButton.icon(
            onPressed: () => unawaited(onClose()),
            icon: const Icon(Icons.close_rounded, size: 18),
            label: Text(l10n.profileArchiveCloseApp),
          ),
        ],
      ),
    );
  }
}

String _profileArchiveErrorMessage(BuildContext context, String code) {
  final l10n = context.openchatL10n;
  return switch (code) {
    'profile_archive_invalid' => l10n.profileArchiveInvalidArchive,
    'profile_archive_passphrase_invalid' =>
      l10n.profileArchivePassphraseInvalid,
    'profile_archive_not_found' => l10n.profileArchiveNotFound,
    'profile_archive_conflict' => l10n.profileArchiveConflict,
    'profile_archive_busy' => l10n.profileArchiveBusy,
    'profile_archive_storage_failed' => l10n.profileArchiveStorageFailed,
    'profile_archive_limit_exceeded' => l10n.profileArchiveLimitExceeded,
    'profile_archive_schema_unsupported' =>
      l10n.profileArchiveSchemaUnsupported,
    'service_timeout' => l10n.profileArchiveTakingLong,
    _ => l10n.profileArchiveOperationFailed,
  };
}
