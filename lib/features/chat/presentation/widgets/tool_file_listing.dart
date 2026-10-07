import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

class ToolFileListing {
  const ToolFileListing({
    required this.entries,
    required this.hasMore,
    required this.isIncomplete,
  });

  final List<ToolFileEntry> entries;
  final bool hasMore;
  final bool isIncomplete;

  static ToolFileListing? fromOutput(Object? output) {
    final value = toolActivityObjectMap(output);
    final rawEntries = value?['entries'];
    if (value == null || rawEntries is! List) return null;

    final entries = <ToolFileEntry>[];
    for (final rawEntry in rawEntries) {
      final entry = toolActivityObjectMap(rawEntry);
      final path = entry?['path'];
      final type = entry?['type'];
      if (entry == null ||
          path is! String ||
          path.isEmpty ||
          (type != 'file' && type != 'directory')) {
        return null;
      }
      entries.add(ToolFileEntry(path: path, isDirectory: type == 'directory'));
    }

    return ToolFileListing(
      entries: entries,
      hasMore: value['nextOffset'] is int,
      isIncomplete: value['truncated'] == true,
    );
  }
}

class ToolFileEntry {
  const ToolFileEntry({required this.path, required this.isDirectory});

  final String path;
  final bool isDirectory;

  String get name {
    final segments = path.replaceAll('\\', '/').split('/');
    return segments.isEmpty ? path : segments.last;
  }
}

class ToolFileListingResult extends StatelessWidget {
  const ToolFileListingResult({
    super.key,
    required this.listing,
    required this.locationLabel,
    required this.palette,
  });

  final ToolFileListing listing;
  final String? locationLabel;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;

    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              if (locationLabel case final label?) ...[
                Icon(
                  LucideIcons.folderOpen,
                  size: 15,
                  color: palette.secondaryIcon,
                ),
                const SizedBox(width: 6),
                Expanded(
                  child: Text(
                    label,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      color: palette.secondaryText,
                      fontSize: 11,
                      height: 16 / 11,
                    ),
                  ),
                ),
              ] else
                const Spacer(),
              Text(
                l10n.toolFileCount(listing.entries.length),
                style: TextStyle(
                  color: palette.secondaryText,
                  fontSize: 11,
                  fontWeight: FontWeight.w500,
                  height: 16 / 11,
                ),
              ),
            ],
          ),
          const SizedBox(height: 7),
          if (listing.entries.isEmpty)
            ToolActivityNotice(message: l10n.toolEmptyListing, palette: palette)
          else
            Container(
              decoration: BoxDecoration(
                color: palette.surface,
                borderRadius: BorderRadius.circular(10),
                border: Border.all(color: palette.border),
              ),
              constraints: const BoxConstraints(maxHeight: 216),
              child: ListView.separated(
                shrinkWrap: true,
                padding: const EdgeInsets.symmetric(vertical: 4),
                itemCount: listing.entries.length,
                separatorBuilder: (context, index) => Divider(
                  height: 1,
                  indent: 32,
                  endIndent: 10,
                  color: palette.border.withValues(alpha: 0.65),
                ),
                itemBuilder: (context, index) {
                  final entry = listing.entries[index];
                  return SizedBox(
                    height: 32,
                    child: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 10),
                      child: Row(
                        children: [
                          Icon(
                            entry.isDirectory
                                ? LucideIcons.folder
                                : LucideIcons.file,
                            size: 16,
                            color: entry.isDirectory
                                ? palette.accent
                                : palette.secondaryIcon,
                          ),
                          const SizedBox(width: 8),
                          Expanded(
                            child: Tooltip(
                              message: entry.name,
                              child: Text(
                                entry.name,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: TextStyle(
                                  color: palette.text,
                                  fontSize: 12,
                                  height: 18 / 12,
                                ),
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                  );
                },
              ),
            ),
          if (listing.hasMore || listing.isIncomplete) ...[
            const SizedBox(height: 7),
            if (listing.hasMore)
              _ToolListingFootnote(
                message: l10n.toolMoreFilesAvailable,
                palette: palette,
              ),
            if (listing.isIncomplete)
              _ToolListingFootnote(
                message: l10n.toolListingIncomplete,
                palette: palette,
              ),
          ],
        ],
      ),
    );
  }
}

class _ToolListingFootnote extends StatelessWidget {
  const _ToolListingFootnote({required this.message, required this.palette});

  final String message;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(top: 3),
    child: Row(
      children: [
        Icon(LucideIcons.info, size: 14, color: palette.secondaryIcon),
        const SizedBox(width: 6),
        Expanded(
          child: Text(
            message,
            style: TextStyle(
              color: palette.secondaryText,
              fontSize: 11,
              height: 16 / 11,
            ),
          ),
        ),
      ],
    ),
  );
}

Map<String, Object?>? toolActivityObjectMap(Object? value) {
  if (value is! Map) return null;
  final result = <String, Object?>{};
  for (final key in value.keys) {
    if (key is! String) return null;
    result[key] = value[key];
  }
  return result;
}

class ToolActivityNotice extends StatelessWidget {
  const ToolActivityNotice({
    super.key,
    required this.message,
    required this.palette,
    this.isError = false,
    this.isLoading = false,
  });

  final String message;
  final OpenChatPalette palette;
  final bool isError;
  final bool isLoading;

  @override
  Widget build(BuildContext context) {
    final errorColor = Theme.of(context).colorScheme.error;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 11),
      decoration: BoxDecoration(
        color: isError ? errorColor.withValues(alpha: 0.08) : palette.surface,
        borderRadius: BorderRadius.circular(10),
        border: isError
            ? Border.all(color: errorColor.withValues(alpha: 0.28))
            : null,
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (isLoading)
            SizedBox.square(
              dimension: 15,
              child: CircularProgressIndicator(
                strokeWidth: 1.6,
                color: palette.accent,
              ),
            )
          else if (isError) ...[
            Icon(LucideIcons.circleAlert, size: 15, color: errorColor),
          ],
          if (isLoading || isError) const SizedBox(width: 7),
          Expanded(
            child: Text(
              message,
              style: TextStyle(
                color: isError ? errorColor : palette.secondaryText,
                fontSize: 12,
                height: 18 / 12,
              ),
            ),
          ),
        ],
      ),
    );
  }
}
