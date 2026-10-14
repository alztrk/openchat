import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/openchat_toast.dart';
import 'package:openchat/features/chat/domain/history_storage_status.dart';
import 'package:openchat/features/settings/domain/support_diagnostics_bundle.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

typedef SupportDiagnosticsExporter = Future<bool> Function(String contents);

class SupportDiagnosticsSection extends StatefulWidget {
  const SupportDiagnosticsSection({
    required this.serviceClient,
    required this.historyStorageStatus,
    this.exporter,
    super.key,
  });

  final OpenChatServiceClient? serviceClient;
  final HistoryStorageStatus historyStorageStatus;
  final SupportDiagnosticsExporter? exporter;

  @override
  State<SupportDiagnosticsSection> createState() =>
      _SupportDiagnosticsSectionState();
}

class _SupportDiagnosticsSectionState extends State<SupportDiagnosticsSection> {
  bool _isPreparing = false;
  bool _isExporting = false;
  String? _error;

  Future<void> _reviewAndExport() async {
    final serviceClient = widget.serviceClient;
    if (serviceClient == null || _isPreparing || _isExporting) {
      setState(
        () => _error = context.openchatL10n.supportDiagnosticsLoadFailed,
      );
      return;
    }
    setState(() {
      _isPreparing = true;
      _error = null;
    });

    late final SupportDiagnosticsBundle bundle;
    try {
      final response = await serviceClient.call('system.health');
      bundle = SupportDiagnosticsBundle.fromServiceHealth(
        value: response,
        generatedAt: DateTime.now().toUtc(),
        platform: Platform.operatingSystem,
        runtimeVersion: Platform.version.split(' ').first,
        historyStorageStatus: _historyStatusValue(widget.historyStorageStatus),
      );
    } on OpenChatServiceException {
      if (mounted) {
        setState(
          () => _error = context.openchatL10n.supportDiagnosticsLoadFailed,
        );
      }
      return;
    } on FormatException {
      if (mounted) {
        setState(
          () => _error = context.openchatL10n.supportDiagnosticsInvalidResponse,
        );
      }
      return;
    } on ArgumentError {
      if (mounted) {
        setState(
          () => _error = context.openchatL10n.supportDiagnosticsInvalidResponse,
        );
      }
      return;
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'support_diagnostics',
          context: ErrorDescription('while loading reviewed diagnostics'),
        ),
      );
      if (mounted) {
        setState(
          () => _error = context.openchatL10n.supportDiagnosticsLoadFailed,
        );
      }
      return;
    } finally {
      if (mounted) setState(() => _isPreparing = false);
    }

    if (!mounted) return;
    final contents = bundle.encode();
    final shouldSave = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(context.openchatL10n.supportDiagnosticsPreviewTitle),
        content: SizedBox(
          width: MediaQuery.sizeOf(context).width < 656
              ? MediaQuery.sizeOf(context).width - 96
              : 560,
          child: SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text(context.openchatL10n.supportDiagnosticsPreviewDescription),
                const SizedBox(height: 16),
                Container(
                  constraints: const BoxConstraints(maxHeight: 360),
                  padding: const EdgeInsets.all(12),
                  decoration: BoxDecoration(
                    color: OpenChatPalette.of(context).surface,
                    border: Border.all(
                      color: OpenChatPalette.of(context).border,
                    ),
                    borderRadius: BorderRadius.circular(OpenChatRadii.control),
                  ),
                  child: SingleChildScrollView(
                    child: SelectableText(
                      contents,
                      style: Theme.of(context).textTheme.bodySmall
                          ?.copyWith(fontFamily: 'Source Code Pro'),
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: Text(context.openchatL10n.cancel),
          ),
          FilledButton.icon(
            onPressed: () => Navigator.of(context).pop(true),
            icon: const Icon(LucideIcons.download, size: 16),
            label: Text(context.openchatL10n.supportDiagnosticsExportAction),
          ),
        ],
      ),
    );
    if (shouldSave != true || !mounted) return;

    setState(() {
      _isExporting = true;
      _error = null;
    });
    try {
      final exported = await (widget.exporter ?? _exportJson)(contents);
      if (exported && mounted) {
        showOpenChatToast(
          context,
          context.openchatL10n.supportDiagnosticsSaved,
          type: OpenChatToastType.success,
        );
      }
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'support_diagnostics',
          context: ErrorDescription('while exporting reviewed diagnostics'),
        ),
      );
      if (mounted) {
        setState(
          () => _error = context.openchatL10n.supportDiagnosticsExportFailed,
        );
      }
    } finally {
      if (mounted) setState(() => _isExporting = false);
    }
  }

  Future<bool> _exportJson(String contents) async {
    final savedPath = await FilePicker.saveFile(
      fileName: 'openchat-support-diagnostics.json',
      bytes: Uint8List.fromList(utf8.encode(contents)),
      mimeType: 'application/json',
      dialogTitle: context.openchatL10n.supportDiagnosticsPreviewTitle,
      type: FileType.custom,
      allowedExtensions: const <String>['json'],
      windowsOptions: const WindowsOptions(lockParentWindow: true),
      linuxOptions: const LinuxOptions(lockParentWindow: true),
    );
    return savedPath != null;
  }

  String _historyStatusValue(HistoryStorageStatus status) => switch (status) {
    HistoryStorageStatus.loading => 'loading',
    HistoryStorageStatus.available => 'available',
    HistoryStorageStatus.unavailable => 'unavailable',
    HistoryStorageStatus.corrupt => 'corrupt',
    HistoryStorageStatus.backupUnavailable => 'backupUnavailable',
  };

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final textTheme = Theme.of(context).textTheme;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        LayoutBuilder(
          builder: (context, constraints) {
            final inline =
                constraints.maxWidth /
                    MediaQuery.textScalerOf(context).scale(1) >=
                680;
            final button = SizedBox(
              width: inline ? 220 : constraints.maxWidth,
              height: 36,
              child: OutlinedButton.icon(
                onPressed: _isPreparing || _isExporting
                    ? null
                    : () => unawaited(_reviewAndExport()),
                icon: _isPreparing || _isExporting
                    ? const SizedBox.square(
                        dimension: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(LucideIcons.fileDown, size: 16),
                label: Text(
                  _isPreparing
                      ? l10n.supportDiagnosticsPreparing
                      : _isExporting
                      ? l10n.saving
                      : l10n.reviewSupportDiagnostics,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            );
            final description = Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(l10n.supportDiagnosticsTitle, style: textTheme.titleSmall),
                const SizedBox(height: 4),
                Text(
                  l10n.supportDiagnosticsDescription,
                  style: textTheme.bodySmall?.copyWith(
                    color: palette.secondaryText,
                  ),
                ),
              ],
            );
            if (!inline) {
              return Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  description,
                  const SizedBox(height: 12),
                  Align(alignment: Alignment.centerRight, child: button),
                ],
              );
            }
            return Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(child: description),
                const SizedBox(width: 16),
                button,
              ],
            );
          },
        ),
        if (_error case final String error) ...[
          const SizedBox(height: 8),
          Text(
            error,
            style: TextStyle(color: Theme.of(context).colorScheme.error),
          ),
          Align(
            alignment: Alignment.centerRight,
            child: TextButton.icon(
              onPressed: _isPreparing || _isExporting
                  ? null
                  : () => unawaited(_reviewAndExport()),
              icon: const Icon(LucideIcons.refreshCw, size: 16),
              label: Text(l10n.retry),
            ),
          ),
        ],
        const SizedBox(height: 4),
        Text(
          l10n.supportDiagnosticsPrivacyNote,
          style: TextStyle(color: palette.secondaryText, fontSize: 12),
        ),
      ],
    );
  }
}
