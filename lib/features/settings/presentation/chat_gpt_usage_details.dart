import 'package:flutter/material.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

import 'package:openchat/features/chat/domain/chatgpt_connection.dart';
import 'package:openchat/features/settings/domain/chat_gpt_usage_snapshot.dart';

class ChatGptUsageDetails extends StatelessWidget {
  const ChatGptUsageDetails({
    required this.connection,
    required this.workspace,
    required this.palette,
    required this.isLoading,
    required this.isLoaded,
    required this.hasError,
    required this.snapshot,
    required this.onRefresh,
    super.key,
  });

  final ChatGptConnection connection;
  final ChatGptWorkspace workspace;
  final OpenChatPalette palette;
  final bool isLoading;
  final bool isLoaded;
  final bool hasError;
  final ChatGptUsageSnapshot? snapshot;
  final VoidCallback onRefresh;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final allowed = snapshot?.ordinaryUsageAllowed;
    final permissionLabel = switch (allowed) {
      true => l10n.ordinaryUsageAvailable,
      false => l10n.ordinaryUsageUnavailable,
      null => l10n.ordinaryUsageUnknown,
    };
    final permissionColor = switch (allowed) {
      true => Theme.of(context).colorScheme.primary,
      false => Theme.of(context).colorScheme.error,
      null => palette.secondaryText,
    };
    return Padding(
      padding: const EdgeInsets.fromLTRB(30, 10, 0, 4),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Expanded(
                child: Text(
                  l10n.accountUsage(
                    workspace.planType ??
                        connection.planType ??
                        l10n.planUnavailable,
                  ),
                  style: TextStyle(
                    color: palette.text,
                    fontSize: 13,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ),
              IconButton(
                tooltip: l10n.refreshUsage,
                onPressed: isLoading ? null : onRefresh,
                icon: const Icon(Icons.refresh_rounded, size: 18),
              ),
            ],
          ),
          if (isLoading)
            const LinearProgressIndicator()
          else if (hasError)
            Row(
              children: [
                Expanded(
                  child: Text(
                    l10n.usageLoadFailed,
                    style: TextStyle(
                      color: palette.secondaryText,
                      fontSize: 12,
                    ),
                  ),
                ),
                TextButton(onPressed: onRefresh, child: Text(l10n.retry)),
              ],
            )
          else if (isLoaded && snapshot != null)
            Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Padding(
                  padding: const EdgeInsets.symmetric(vertical: 8),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        permissionLabel,
                        style: TextStyle(color: permissionColor, fontSize: 12),
                      ),
                      const SizedBox(height: 4),
                      Text(
                        l10n.usageUpdatedAt(
                          _localizedTimestamp(
                            context,
                            snapshot!.fetchedAtUnixMs,
                          ),
                        ),
                        style: TextStyle(
                          color: palette.secondaryText,
                          fontSize: 11,
                        ),
                      ),
                    ],
                  ),
                ),
                for (final bucket in snapshot!.buckets) ...[
                  Padding(
                    padding: const EdgeInsets.only(top: 8, bottom: 5),
                    child: Row(
                      children: [
                        Expanded(
                          child: Text(
                            switch (bucket.limitId) {
                              'codex:primary' => l10n.usageFiveHour,
                              'codex:secondary' => l10n.usageWeekly,
                              _ => bucket.limitId,
                            },
                            style: TextStyle(
                              color: palette.secondaryText,
                              fontSize: 12,
                            ),
                          ),
                        ),
                        Text(
                          bucket.usedPercent == null
                              ? l10n.unavailableValue
                              : l10n.usageUsedPercent(
                                  bucket.usedPercent!.toStringAsFixed(0),
                                ),
                          style: TextStyle(
                            color: palette.secondaryText,
                            fontSize: 12,
                          ),
                        ),
                      ],
                    ),
                  ),
                  LinearProgressIndicator(
                    value: bucket.usedPercent == null
                        ? null
                        : (bucket.usedPercent! / 100).clamp(0, 1),
                  ),
                  if (bucket.resetAtUnixMs case final int resetAt)
                    Padding(
                      padding: const EdgeInsets.only(top: 5),
                      child: Text(
                        l10n.quotaResetsAt(
                          _localizedTimestamp(context, resetAt),
                        ),
                        style: TextStyle(
                          color: palette.secondaryText,
                          fontSize: 11,
                        ),
                      ),
                    ),
                ],
                const SizedBox(height: 12),
                Text(
                  snapshot!.resetCreditCount == null
                      ? l10n.resetCreditCountUnavailable
                      : l10n.resetCreditsAvailable(snapshot!.resetCreditCount!),
                  style: TextStyle(color: palette.secondaryText, fontSize: 12),
                ),
                if (snapshot!.resetCredits.isNotEmpty)
                  for (final credit in snapshot!.resetCredits)
                    Padding(
                      padding: const EdgeInsets.only(top: 6),
                      child: Text(
                        [
                          credit.title ?? credit.resetType ?? l10n.resetCredit,
                          credit.status ?? l10n.statusUnavailable,
                          if (credit.grantedAtUnixMs case final int grantedAt)
                            l10n.creditGrantedAt(
                              _localizedTimestamp(context, grantedAt),
                            ),
                          if (credit.expiresAtUnixMs case final int expiresAt)
                            l10n.creditExpiresAt(
                              _localizedTimestamp(context, expiresAt),
                            ),
                        ].join(' · '),
                        style: TextStyle(
                          color: palette.secondaryText,
                          fontSize: 12,
                        ),
                      ),
                    )
                else if (snapshot!.resetCreditDetailsState == 'available')
                  Padding(
                    padding: const EdgeInsets.only(top: 5),
                    child: Text(
                      l10n.noResetCredits,
                      style: TextStyle(
                        color: palette.secondaryText,
                        fontSize: 12,
                      ),
                    ),
                  )
                else if (snapshot!.resetCreditDetailsState != 'available')
                  Padding(
                    padding: const EdgeInsets.only(top: 5),
                    child: Text(
                      l10n.resetCreditDetailsUnavailable,
                      style: TextStyle(
                        color: palette.secondaryText,
                        fontSize: 12,
                      ),
                    ),
                  ),
                for (final credit in snapshot!.resetCredits)
                  if (credit.description case final String description)
                    Padding(
                      padding: const EdgeInsets.only(top: 3),
                      child: Text(
                        description,
                        style: TextStyle(
                          color: palette.secondaryText,
                          fontSize: 11,
                        ),
                      ),
                    ),
              ],
            ),
        ],
      ),
    );
  }
}

String _localizedTimestamp(BuildContext context, int timestamp) {
  final dateTime = DateTime.fromMillisecondsSinceEpoch(timestamp).toLocal();
  final localizations = MaterialLocalizations.of(context);
  final date = localizations.formatMediumDate(dateTime);
  final time = localizations.formatTimeOfDay(TimeOfDay.fromDateTime(dateTime));
  return '$date, $time';
}
