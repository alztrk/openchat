import 'package:flutter/material.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

import 'package:openchat/features/chat/domain/chatgpt_connection.dart';
import 'package:openchat/features/settings/domain/chat_gpt_usage_snapshot.dart';
import 'package:openchat/features/settings/presentation/chat_gpt_usage_bucket_label.dart';

class ChatGptUsageDetails extends StatelessWidget {
  const ChatGptUsageDetails({
    required this.connection,
    required this.workspace,
    required this.palette,
    required this.isLoading,
    required this.isLoaded,
    required this.hasError,
    required this.snapshot,
    required this.confirmingResetCreditId,
    required this.redeemingResetCreditId,
    required this.needsResetCreditRefresh,
    required this.onRefresh,
    required this.onRedeemResetCredit,
    super.key,
  });

  final ChatGptConnection connection;
  final ChatGptWorkspace workspace;
  final OpenChatPalette palette;
  final bool isLoading;
  final bool isLoaded;
  final bool hasError;
  final ChatGptUsageSnapshot? snapshot;
  final String? confirmingResetCreditId;
  final String? redeemingResetCreditId;
  final bool needsResetCreditRefresh;
  final VoidCallback onRefresh;
  final ValueChanged<ChatGptResetCredit> onRedeemResetCredit;

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
                onPressed:
                    isLoading ||
                        confirmingResetCreditId != null ||
                        redeemingResetCreditId != null
                    ? null
                    : onRefresh,
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
                            chatGptUsageBucketLabel(bucket, l10n),
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
                const SizedBox(height: 14),
                _buildResetCreditsCard(context, snapshot!),
              ],
            ),
        ],
      ),
    );
  }

  Widget _buildResetCreditsCard(
    BuildContext context,
    ChatGptUsageSnapshot snapshot,
  ) {
    final l10n = context.openchatL10n;
    final theme = Theme.of(context);
    final resetCreditCount = snapshot.resetCreditCount;

    return Card(
      margin: EdgeInsets.zero,
      color: palette.surface,
      elevation: 0,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(10),
        side: BorderSide(color: palette.border),
      ),
      child: Padding(
        padding: const EdgeInsets.all(14),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                Icon(
                  Icons.restart_alt_rounded,
                  color: palette.accentIcon,
                  size: 19,
                ),
                const SizedBox(width: 9),
                Expanded(
                  child: Text(
                    resetCreditCount == null
                        ? l10n.resetCreditCountUnavailable
                        : l10n.resetCreditsAvailable(resetCreditCount),
                    style: theme.textTheme.titleSmall?.copyWith(
                      color: palette.text,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                ),
              ],
            ),
            if (snapshot.resetCredits.isNotEmpty)
              for (var index = 0; index < snapshot.resetCredits.length; index++)
                _buildResetCreditRow(
                  context,
                  snapshot.resetCredits[index],
                  showDivider: index > 0,
                )
            else
              Padding(
                padding: const EdgeInsets.only(top: 8),
                child: Text(
                  snapshot.resetCreditDetailsState == 'available'
                      ? l10n.noResetCredits
                      : l10n.resetCreditDetailsUnavailable,
                  style: TextStyle(color: palette.secondaryText, fontSize: 12),
                ),
              ),
            if (needsResetCreditRefresh)
              Padding(
                padding: const EdgeInsets.only(top: 10),
                child: Text(
                  l10n.resetCreditRefreshRequired,
                  style: TextStyle(
                    color: theme.colorScheme.error,
                    fontSize: 12,
                  ),
                ),
              ),
          ],
        ),
      ),
    );
  }

  Widget _buildResetCreditRow(
    BuildContext context,
    ChatGptResetCredit credit, {
    required bool showDivider,
  }) {
    final l10n = context.openchatL10n;
    final isAvailable = credit.status == 'available';
    final isConfirming = confirmingResetCreditId == credit.id;
    final isRedeeming = redeemingResetCreditId == credit.id;
    final metadata = <String>[
      if (credit.status != null)
        credit.status == 'available'
            ? l10n.resetCreditAvailableStatus
            : credit.status!,
      if (credit.grantedAtUnixMs case final int grantedAt)
        l10n.creditGrantedAt(_localizedTimestamp(context, grantedAt)),
      if (credit.expiresAtUnixMs case final int expiresAt)
        l10n.creditExpiresAt(_localizedTimestamp(context, expiresAt)),
    ];

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (showDivider)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 10),
            child: Divider(height: 1, color: palette.border),
          )
        else
          const SizedBox(height: 10),
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    credit.title ?? credit.resetType ?? l10n.resetCredit,
                    style: TextStyle(
                      color: palette.text,
                      fontSize: 13,
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                  if (metadata.isNotEmpty) ...[
                    const SizedBox(height: 4),
                    Text(
                      metadata.join(' · '),
                      style: TextStyle(
                        color: palette.secondaryText,
                        fontSize: 11,
                      ),
                    ),
                  ],
                  if (credit.description case final String description) ...[
                    const SizedBox(height: 4),
                    Text(
                      description,
                      style: TextStyle(
                        color: palette.secondaryText,
                        fontSize: 11,
                      ),
                    ),
                  ],
                ],
              ),
            ),
            if (isAvailable) ...[
              const SizedBox(width: 10),
              FilledButton.icon(
                onPressed:
                    isConfirming || isRedeeming || needsResetCreditRefresh
                    ? null
                    : () => onRedeemResetCredit(credit),
                icon: isRedeeming
                    ? const SizedBox.square(
                        dimension: 15,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.restart_alt_rounded, size: 16),
                label: Text(
                  isRedeeming ? l10n.resetCreditRedeeming : l10n.useResetCredit,
                ),
              ),
            ],
          ],
        ),
      ],
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
