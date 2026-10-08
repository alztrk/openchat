import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

class CreateProjectDialog extends StatefulWidget {
  const CreateProjectDialog({super.key});

  @override
  State<CreateProjectDialog> createState() => _CreateProjectDialogState();
}

class _CreateProjectDialogState extends State<CreateProjectDialog> {
  final _nameController = TextEditingController();
  String? _folderPath;
  String? _errorMessage;
  bool _isSelectingFolder = false;

  @override
  void dispose() {
    _nameController.dispose();
    super.dispose();
  }

  Future<void> _selectFolder() async {
    if (_isSelectingFolder) return;
    setState(() {
      _isSelectingFolder = true;
      _errorMessage = null;
    });
    try {
      final folderPath = await FilePicker.getDirectoryPath(
        dialogTitle: context.openchatL10n.chooseProjectFolder,
        windowsOptions: const WindowsOptions(lockParentWindow: true),
        linuxOptions: const LinuxOptions(lockParentWindow: true),
      );
      if (!mounted) return;
      if (folderPath != null && folderPath.trim().isNotEmpty) {
        setState(() => _folderPath = folderPath.trim());
      }
    } on Exception {
      if (mounted) {
        setState(
          () =>
              _errorMessage = context.openchatL10n.projectFolderSelectionFailed,
        );
      }
    } finally {
      if (mounted) setState(() => _isSelectingFolder = false);
    }
  }

  void _createProject() {
    final name = _nameController.text.trim();
    final folderPath = _folderPath;
    if (name.isEmpty) {
      setState(() => _errorMessage = context.openchatL10n.projectNameRequired);
      return;
    }
    if (folderPath == null) {
      setState(
        () => _errorMessage = context.openchatL10n.projectFolderNotSelected,
      );
      return;
    }
    Navigator.of(context).pop((name: name, folderPath: folderPath));
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);

    return AlertDialog(
      title: Text(l10n.createProject),
      content: SizedBox(
        width: 420,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            TextField(
              controller: _nameController,
              autofocus: true,
              maxLines: 1,
              textInputAction: TextInputAction.next,
              onChanged: (_) {
                if (_errorMessage != null) {
                  setState(() => _errorMessage = null);
                }
              },
              decoration: InputDecoration(labelText: l10n.projectName),
            ),
            const SizedBox(height: 18),
            Text(
              l10n.projectFolder,
              style: Theme.of(context).textTheme.labelLarge,
            ),
            const SizedBox(height: 4),
            Text(
              l10n.projectFolderDescription,
              style: Theme.of(context).textTheme.bodySmall,
            ),
            const SizedBox(height: 8),
            Container(
              constraints: const BoxConstraints(minHeight: 52),
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
              decoration: BoxDecoration(
                color: palette.composer,
                border: Border.all(color: palette.border),
                borderRadius: BorderRadius.circular(8),
              ),
              child: Row(
                children: [
                  Icon(
                    LucideIcons.folder,
                    size: 18,
                    color: palette.secondaryIcon,
                  ),
                  const SizedBox(width: 9),
                  Expanded(
                    child: Text(
                      _folderPath ?? l10n.projectFolderNotSelected,
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: _folderPath == null
                            ? palette.secondaryText
                            : palette.text,
                        fontSize: 13,
                      ),
                    ),
                  ),
                ],
              ),
            ),
            const SizedBox(height: 10),
            OutlinedButton.icon(
              onPressed: _isSelectingFolder ? null : _selectFolder,
              icon: _isSelectingFolder
                  ? const SizedBox.square(
                      dimension: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(LucideIcons.folderOpen, size: 18),
              label: Text(l10n.chooseProjectFolder),
            ),
            if (_errorMessage case final String error) ...[
              const SizedBox(height: 8),
              Text(
                error,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ],
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: _isSelectingFolder
              ? null
              : () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          onPressed: _isSelectingFolder ? null : _createProject,
          child: Text(l10n.createProject),
        ),
      ],
    );
  }
}
