import 'package:flutter/material.dart';

import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

class ProjectIndexDialog extends StatefulWidget {
  const ProjectIndexDialog({
    required this.serviceClient,
    required this.projectId,
    super.key,
  });
  final OpenChatServiceClient serviceClient;
  final String projectId;

  @override
  State<ProjectIndexDialog> createState() => _ProjectIndexDialogState();
}

class _ProjectIndexDialogState extends State<ProjectIndexDialog> {
  Map<String, Object?>? _index;
  bool _busy = true;
  String? _error;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      final response = await widget.serviceClient.call(
        'project.index.get',
        params: {'projectId': widget.projectId},
      );
      final index = response['index'];
      if (index is! Map<String, Object?>) {
        throw const FormatException('Invalid index state.');
      }
      if (mounted) {
        setState(() {
          _index = index;
          _busy = false;
        });
      }
    } on Exception {
      if (mounted) {
        setState(() {
          _busy = false;
          _error = context.openchatL10n.projectIndexLoadFailed;
        });
      }
    }
  }

  Future<void> _run(
    String method, [
    Map<String, Object?> extra = const {},
  ]) async {
    if (_busy) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      final response = await widget.serviceClient.call(
        method,
        params: {'projectId': widget.projectId, ...extra},
      );
      final index = response['index'];
      if (index is! Map<String, Object?>) {
        throw const FormatException('Invalid index state.');
      }
      if (mounted) {
        setState(() {
          _index = index;
          _busy = false;
        });
      }
    } on Exception {
      if (mounted) {
        setState(() {
          _busy = false;
          _error = context.openchatL10n.projectIndexActionFailed;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final index = _index;
    return AlertDialog(
      title: Text(l10n.projectIndexTitle),
      content: SizedBox(
        width: 480,
        child: _busy && index == null
            ? const SizedBox(
                height: 160,
                child: Center(child: CircularProgressIndicator()),
              )
            : Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Text(l10n.projectIndexDescription),
                  if (index != null)
                    SwitchListTile(
                      contentPadding: EdgeInsets.zero,
                      title: Text(l10n.projectIndexEnable),
                      value: index['enabled'] == true,
                      onChanged: _busy
                          ? null
                          : (enabled) => _run('project.index.set_enabled', {
                              'enabled': enabled,
                            }),
                    ),
                  if (index != null)
                    Text(
                      l10n.projectIndexStatus(
                        '${index['status']}',
                        '${index['indexedFiles']}',
                        '${index['indexedBytes']}',
                      ),
                    ),
                  if (_error != null)
                    Text(
                      _error!,
                      style: TextStyle(
                        color: Theme.of(context).colorScheme.error,
                      ),
                    ),
                ],
              ),
      ),
      actions: [
        TextButton(
          onPressed: _busy ? null : () => _run('project.index.sync'),
          child: Text(l10n.projectIndexSync),
        ),
        TextButton(
          onPressed: _busy ? null : () => _run('project.index.clear'),
          child: Text(l10n.projectIndexClear),
        ),
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
      ],
    );
  }
}
