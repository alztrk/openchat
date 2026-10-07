import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/settings/data/local_engines_models.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

class LocalModelDirectoryControls extends StatelessWidget {
  const LocalModelDirectoryControls({
    required this.directory,
    required this.isCustom,
    required this.isBusy,
    required this.isScanning,
    required this.onChoose,
    required this.onUseDefault,
    required this.onScan,
    super.key,
  });

  final String directory;
  final bool isCustom;
  final bool isBusy;
  final bool isScanning;
  final VoidCallback onChoose;
  final VoidCallback onUseDefault;
  final VoidCallback onScan;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          l10n.localModelDirectoryTitle,
          style: Theme.of(context).textTheme.titleSmall,
        ),
        const SizedBox(height: 4),
        Text(
          l10n.localModelDirectoryDescription,
          style: Theme.of(context).textTheme.bodySmall
              ?.copyWith(color: palette.secondaryText),
        ),
        const SizedBox(height: 8),
        SelectableText(
          directory,
          style: Theme.of(context).textTheme.bodySmall
              ?.copyWith(color: palette.secondaryText),
        ),
        const SizedBox(height: 8),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            OutlinedButton.icon(
              onPressed: isBusy ? null : onChoose,
              icon: const Icon(LucideIcons.folderOpen, size: 17),
              label: Text(l10n.localModelChooseDirectory),
            ),
            OutlinedButton.icon(
              onPressed: isBusy ? null : onScan,
              icon: isScanning
                  ? const SizedBox.square(
                      dimension: 15,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(LucideIcons.search, size: 17),
              label: Text(
                isScanning
                    ? l10n.localModelScanningDirectory
                    : l10n.localModelScanDirectory,
              ),
            ),
            if (isCustom)
              TextButton.icon(
                onPressed: isBusy ? null : onUseDefault,
                icon: const Icon(LucideIcons.rotateCcw, size: 17),
                label: Text(l10n.localModelUseDefaultDirectory),
              ),
          ],
        ),
      ],
    );
  }
}

class LocalModelDiscoveryDialog {
  const LocalModelDiscoveryDialog._();

  static Future<bool?> show(
    BuildContext context,
    LocalModelDiscovery discovery,
  ) {
    return showDialog<bool>(
      context: context,
      builder: (dialogContext) {
        final l10n = dialogContext.openchatL10n;
        final palette = OpenChatPalette.of(dialogContext);
        return AlertDialog(
          title: Text(l10n.localModelDiscoveryTitle),
          content: SizedBox(
            width: 460,
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text(l10n.localModelDiscoveryPrompt(discovery.models.length)),
                if (discovery.truncated) ...[
                  const SizedBox(height: 8),
                  Text(
                    l10n.localModelDiscoveryTruncated,
                    style: TextStyle(color: palette.secondaryText),
                  ),
                ],
                const SizedBox(height: 12),
                ConstrainedBox(
                  constraints: const BoxConstraints(maxHeight: 320),
                  child: ListView.separated(
                    shrinkWrap: true,
                    itemCount: discovery.models.length,
                    separatorBuilder: (_, _) => const Divider(height: 1),
                    itemBuilder: (context, index) {
                      final model = discovery.models[index];
                      final itemPalette = OpenChatPalette.of(context);
                      return Padding(
                        padding: const EdgeInsets.symmetric(vertical: 8),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              model.displayName,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: Theme.of(context).textTheme.titleSmall,
                            ),
                            const SizedBox(height: 2),
                            Text(
                              model.path,
                              maxLines: 2,
                              overflow: TextOverflow.ellipsis,
                              style: Theme.of(context).textTheme.bodySmall
                                  ?.copyWith(color: itemPalette.secondaryText),
                            ),
                          ],
                        ),
                      );
                    },
                  ),
                ),
              ],
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(dialogContext).pop(false),
              child: Text(l10n.cancel),
            ),
            FilledButton(
              onPressed: () => Navigator.of(dialogContext).pop(true),
              child: Text(l10n.localModelDiscoveryRegisterAll),
            ),
          ],
        );
      },
    );
  }
}
