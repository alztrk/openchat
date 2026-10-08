import 'dart:convert';

import 'package:flutter/material.dart';

import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

class ProjectInstructionsDialog extends StatefulWidget {
  const ProjectInstructionsDialog({
    required this.serviceClient,
    required this.projectId,
    super.key,
  });

  final OpenChatServiceClient serviceClient;
  final String projectId;

  @override
  State<ProjectInstructionsDialog> createState() =>
      _ProjectInstructionsDialogState();
}

class _ProjectInstructionsDialogState extends State<ProjectInstructionsDialog> {
  static const _maxBytes = 16 * 1024;
  final _controller = TextEditingController();
  bool _isLoading = true;
  bool _isSaving = false;
  bool _loadFailed = false;
  String? _error;

  int get _byteCount => utf8.encode(_controller.text).length;

  @override
  void initState() {
    super.initState();
    _load();
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    setState(() {
      _isLoading = true;
      _loadFailed = false;
      _error = null;
    });
    try {
      final response = await widget.serviceClient.call(
        'project.instructions.get',
        params: <String, Object?>{'projectId': widget.projectId},
      );
      final instructions = response['instructions'];
      if (instructions is! String) {
        throw const FormatException('The project instructions were invalid.');
      }
      if (mounted) {
        setState(() {
          _controller.text = instructions;
          _isLoading = false;
          _loadFailed = false;
        });
      }
    } on Exception {
      if (mounted) {
        setState(() {
          _isLoading = false;
          _loadFailed = true;
          _error = context.openchatL10n.projectInstructionsLoadFailed;
        });
      }
    }
  }

  Future<void> _save() async {
    if (_byteCount > _maxBytes || _isSaving) return;
    setState(() {
      _isSaving = true;
      _error = null;
    });
    try {
      await widget.serviceClient.call(
        'project.instructions.save',
        params: <String, Object?>{
          'projectId': widget.projectId,
          'instructions': _controller.text,
        },
      );
      if (mounted) Navigator.of(context).pop();
    } on Exception {
      if (mounted) {
        setState(() {
          _isSaving = false;
          _error = context.openchatL10n.projectInstructionsSaveFailed;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final error = _error;
    return AlertDialog(
      title: Text(l10n.projectInstructionsTitle),
      content: SizedBox(
        width: 560,
        child: _isLoading
            ? const SizedBox(
                height: 220,
                child: Center(child: CircularProgressIndicator()),
              )
            : Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Text(l10n.projectInstructionsDescription),
                  const SizedBox(height: 12),
                  TextField(
                    controller: _controller,
                    enabled: !_isSaving && !_loadFailed,
                    minLines: 5,
                    maxLines: 12,
                    onChanged: (_) => setState(() {}),
                    decoration: InputDecoration(
                      labelText: l10n.projectInstructionsTitle,
                      hintText: l10n.projectInstructionsHint,
                      alignLabelWithHint: true,
                      border: const OutlineInputBorder(),
                    ),
                  ),
                  Align(
                    alignment: AlignmentDirectional.centerEnd,
                    child: Text(
                      l10n.projectInstructionsSize(_byteCount, _maxBytes),
                    ),
                  ),
                  if (error != null) ...[
                    const SizedBox(height: 8),
                    Text(
                      error,
                      style: TextStyle(
                        color: Theme.of(context).colorScheme.error,
                      ),
                    ),
                  ],
                  if (_loadFailed) ...[
                    const SizedBox(height: 8),
                    Align(
                      alignment: AlignmentDirectional.centerStart,
                      child: TextButton(
                        onPressed: _load,
                        child: Text(l10n.agentRunRefresh),
                      ),
                    ),
                  ],
                ],
              ),
      ),
      actions: [
        TextButton(
          onPressed: _isSaving ? null : () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          onPressed:
              _isLoading || _isSaving || _loadFailed || _byteCount > _maxBytes
              ? null
              : _save,
          child: _isSaving
              ? const SizedBox.square(
                  dimension: 18,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : Text(l10n.projectInstructionsSave),
        ),
      ],
    );
  }
}
