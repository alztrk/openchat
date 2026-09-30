import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/openchat_toast.dart';
import 'package:openchat/features/chat/domain/chatgpt_connection.dart';
import 'package:openchat/features/settings/domain/chat_gpt_usage_snapshot.dart';
import 'package:openchat/features/settings/presentation/chat_gpt_usage_bucket_label.dart';
import 'package:openchat/features/settings/presentation/settings_widgets.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

enum _QuotasLoadState { loading, loaded, failed }

class UsageQuotasSettingsSection extends StatefulWidget {
  const UsageQuotasSettingsSection({
    required this.serviceClient,
    this.onNavigateToConnections,
    super.key,
  });

  final OpenChatServiceClient? serviceClient;
  final VoidCallback? onNavigateToConnections;

  @override
  State<UsageQuotasSettingsSection> createState() =>
      _UsageQuotasSettingsSectionState();
}

class _UsageQuotasSettingsSectionState
    extends State<UsageQuotasSettingsSection> {
  _QuotasLoadState _loadState = _QuotasLoadState.loading;
  List<ChatGptConnection> _connections = const [];
  final Map<String, ChatGptUsageSnapshot> _snapshots = {};
  final Set<String> _loadingKeys = {};
  final Map<String, String> _errors = {};
  final Set<String> _refreshRequiredKeys = {};
  String? _confirmingResetCreditId;
  String? _redeemingResetCreditId;
  bool _isRefreshingAll = false;
  int _loadGeneration = 0;

  @override
  void initState() {
    super.initState();
    unawaited(_loadAll());
  }

  String _workspaceKey(String connectionId, String workspaceId) =>
      '$connectionId:$workspaceId';

  Future<void> _loadAll({bool forceRefresh = false}) async {
    final service = widget.serviceClient;
    if (service == null) {
      if (mounted) {
        setState(() => _loadState = _QuotasLoadState.loaded);
      }
      return;
    }

    final generation = ++_loadGeneration;
    setState(() {
      _loadState = _QuotasLoadState.loading;
      _isRefreshingAll = forceRefresh;
      if (forceRefresh) {
        _errors.clear();
      }
    });

    try {
      final response = await service.call('chatgpt.connections.list');
      final rawConnections = response['connections'];
      if (rawConnections is! List<Object?>) {
        throw const FormatException('The ChatGPT connection list was invalid.');
      }
      final connections = rawConnections
          .map((value) => ChatGptConnection.fromJson(_serviceObjectMap(value)))
          .toList(growable: false);

      if (!mounted || generation != _loadGeneration) return;

      setState(() {
        _connections = connections;
        _loadState = _QuotasLoadState.loaded;
      });

      final futures = <Future<void>>[];
      for (final connection in connections) {
        for (final workspace in connection.workspaces) {
          futures.add(_loadUsageForWorkspace(connection, workspace));
        }
      }
      await Future.wait(futures);
    } on OpenChatServiceException {
      if (!mounted || generation != _loadGeneration) return;
      setState(() => _loadState = _QuotasLoadState.failed);
    } on FormatException {
      if (!mounted || generation != _loadGeneration) return;
      setState(() => _loadState = _QuotasLoadState.failed);
    } finally {
      if (mounted && generation == _loadGeneration) {
        setState(() => _isRefreshingAll = false);
      }
    }
  }

  Future<void> _loadUsageForWorkspace(
    ChatGptConnection connection,
    ChatGptWorkspace workspace, {
    bool userInitiated = false,
  }) async {
    final service = widget.serviceClient;
    if (service == null) return;

    final key = _workspaceKey(connection.id, workspace.id);
    setState(() {
      _loadingKeys.add(key);
      _errors.remove(key);
    });

    try {
      final response = await service.call(
        'chatgpt.usage.get',
        params: <String, Object?>{
          'connectionId': connection.id,
          'workspaceId': workspace.id,
        },
      );
      final snapshot = ChatGptUsageSnapshot.fromJson(response);
      if (!mounted) return;
      setState(() {
        _snapshots[key] = snapshot;
        _loadingKeys.remove(key);
        if (userInitiated) {
          _refreshRequiredKeys.remove(key);
        }
      });
    } on OpenChatServiceException catch (error) {
      if (!mounted) return;
      setState(() {
        _loadingKeys.remove(key);
        _errors[key] = error.message;
      });
    } on FormatException {
      if (!mounted) return;
      setState(() {
        _loadingKeys.remove(key);
        _errors[key] = context.openchatL10n.providerDataUnavailable;
      });
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while loading ChatGPT usage quota'),
        ),
      );
      if (!mounted) return;
      setState(() {
        _loadingKeys.remove(key);
        _errors[key] = context.openchatL10n.usageLoadFailed;
      });
    }
  }

  Future<void> _redeemResetCredit(
    ChatGptConnection connection,
    ChatGptWorkspace workspace,
    ChatGptResetCredit credit,
  ) async {
    final service = widget.serviceClient;
    final key = _workspaceKey(connection.id, workspace.id);
    if (service == null ||
        _confirmingResetCreditId != null ||
        _redeemingResetCreditId != null ||
        credit.status != 'available' ||
        _refreshRequiredKeys.contains(key)) {
      return;
    }

    final l10n = context.openchatL10n;
    setState(() => _confirmingResetCreditId = credit.id);
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: Text(l10n.confirmResetCreditTitle),
        content: Text(l10n.confirmResetCreditMessage),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: Text(l10n.cancel),
          ),
          FilledButton.icon(
            onPressed: () => Navigator.of(dialogContext).pop(true),
            icon: const Icon(Icons.restart_alt_rounded, size: 17),
            label: Text(l10n.confirmResetCreditAction),
          ),
        ],
      ),
    );
    if (!mounted) return;
    setState(() => _confirmingResetCreditId = null);
    if (confirmed != true) return;

    setState(() => _redeemingResetCreditId = credit.id);
    var refreshRequired = false;
    var resultMessage = l10n.resetCreditOutcomeUnknown;
    var resultType = OpenChatToastType.success;
    try {
      final response = await service.call(
        'chatgpt.reset_credits.consume',
        params: <String, Object?>{
          'connectionId': connection.id,
          'workspaceId': workspace.id,
          'creditId': credit.id,
        },
      );
      switch (response['outcome']) {
        case 'reset':
          resultMessage = l10n.resetCreditApplied;
          break;
        case 'alreadyRedeemed':
          refreshRequired = true;
          resultMessage = l10n.resetCreditAlreadyUsed;
          resultType = OpenChatToastType.warning;
          break;
        case 'nothingToReset':
          resultMessage = l10n.resetCreditNothingToReset;
          resultType = OpenChatToastType.warning;
          break;
        case 'noCredit':
          refreshRequired = true;
          resultMessage = l10n.resetCreditNoLongerAvailable;
          resultType = OpenChatToastType.warning;
          break;
        default:
          refreshRequired = true;
          resultMessage = l10n.resetCreditOutcomeUnknown;
          resultType = OpenChatToastType.warning;
          break;
      }
    } on OpenChatServiceException catch (error) {
      switch (error.code) {
        case 'authentication_required':
          resultMessage = l10n.resetCreditSignInRequired;
          break;
        case 'permission_denied':
          resultMessage = l10n.resetCreditRejected;
          break;
        case 'reset_credit_unavailable':
        case 'workspace_not_found':
          resultMessage = l10n.resetCreditNoLongerAvailable;
          resultType = OpenChatToastType.warning;
          break;
        default:
          refreshRequired = true;
          resultMessage = l10n.resetCreditOutcomeUnknown;
          resultType = OpenChatToastType.warning;
          break;
      }
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while redeeming ChatGPT reset credit'),
        ),
      );
      refreshRequired = true;
      resultMessage = l10n.resetCreditOutcomeUnknown;
      resultType = OpenChatToastType.warning;
    } finally {
      if (mounted) {
        setState(() {
          _redeemingResetCreditId = null;
          if (refreshRequired) {
            _refreshRequiredKeys.add(key);
          }
        });
        showOpenChatToast(context, resultMessage, type: resultType);
        unawaited(
          _loadUsageForWorkspace(connection, workspace, userInitiated: true),
        );
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final l10n = context.openchatL10n;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    l10n.usageQuotas,
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                  const SizedBox(height: 4),
                  Text(
                    l10n.usageQuotasDescription,
                    style: TextStyle(
                      color: palette.secondaryText,
                      fontSize: 13,
                      height: 1.4,
                    ),
                  ),
                ],
              ),
            ),
            const SizedBox(width: 16),
            OutlinedButton.icon(
              onPressed:
                  _isRefreshingAll || _loadState == _QuotasLoadState.loading
                  ? null
                  : () => unawaited(_loadAll(forceRefresh: true)),
              icon: _isRefreshingAll
                  ? const SizedBox.square(
                      dimension: 14,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.refresh_rounded, size: 16),
              label: Text(l10n.refreshAll),
            ),
          ],
        ),
        const SizedBox(height: 20),
        if (_loadState == _QuotasLoadState.loading && _connections.isEmpty)
          const Padding(
            padding: EdgeInsets.symmetric(vertical: 32),
            child: Center(child: CircularProgressIndicator()),
          )
        else if (_loadState == _QuotasLoadState.failed)
          _buildFailedState(palette, l10n)
        else if (_connections.isEmpty)
          _buildEmptyState(palette, l10n)
        else
          Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              for (var i = 0; i < _connections.length; i++) ...[
                if (i > 0) const SizedBox(height: 16),
                _buildConnectionQuotaCard(_connections[i], palette, l10n),
              ],
            ],
          ),
      ],
    );
  }

  Widget _buildEmptyState(OpenChatPalette palette, AppLocalizations l10n) {
    return Card(
      elevation: 0,
      color: palette.surface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(12),
        side: BorderSide(color: palette.border),
      ),
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          children: [
            Icon(
              Icons.data_usage_rounded,
              size: 40,
              color: palette.secondaryIcon,
            ),
            const SizedBox(height: 14),
            Text(
              l10n.noChatGptAccountsForQuota,
              style: TextStyle(
                color: palette.text,
                fontSize: 15,
                fontWeight: FontWeight.w600,
              ),
            ),
            const SizedBox(height: 6),
            ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 440),
              child: Text(
                l10n.noChatGptAccountsForQuotaDescription,
                textAlign: TextAlign.center,
                style: TextStyle(
                  color: palette.secondaryText,
                  fontSize: 13,
                  height: 1.4,
                ),
              ),
            ),
            if (widget.onNavigateToConnections != null) ...[
              const SizedBox(height: 18),
              FilledButton.icon(
                onPressed: widget.onNavigateToConnections,
                icon: const Icon(Icons.link_rounded, size: 16),
                label: Text(l10n.goToConnections),
              ),
            ],
          ],
        ),
      ),
    );
  }

  Widget _buildFailedState(OpenChatPalette palette, AppLocalizations l10n) {
    return Card(
      elevation: 0,
      color: palette.surface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(12),
        side: BorderSide(color: palette.border),
      ),
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Row(
          children: [
            Icon(
              Icons.error_outline_rounded,
              size: 24,
              color: Theme.of(context).colorScheme.error,
            ),
            const SizedBox(width: 14),
            Expanded(
              child: Text(
                l10n.oauthConnectionsLoadFailed,
                style: TextStyle(color: palette.secondaryText, fontSize: 13),
              ),
            ),
            TextButton.icon(
              onPressed: () => unawaited(_loadAll(forceRefresh: true)),
              icon: const Icon(Icons.refresh_rounded, size: 16),
              label: Text(l10n.retry),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildConnectionQuotaCard(
    ChatGptConnection connection,
    OpenChatPalette palette,
    AppLocalizations l10n,
  ) {
    final theme = Theme.of(context);
    final isDark = theme.brightness == Brightness.dark;
    final logoColor = isDark ? Colors.white : Colors.black;

    final accountName =
        connection.email ?? connection.displayName ?? connection.id;
    final plan = connection.planType ?? l10n.planUnavailable;

    return Card(
      elevation: 0,
      margin: EdgeInsets.zero,
      color: palette.surface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(12),
        side: BorderSide(
          color: connection.isSelected
              ? theme.colorScheme.primary.withValues(alpha: 0.45)
              : palette.border,
          width: connection.isSelected ? 1.5 : 1.0,
        ),
      ),
      child: Padding(
        padding: const EdgeInsets.all(18),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                SvgPicture.asset(
                  'assets/icons/chatgpt.svg',
                  width: 22,
                  height: 22,
                  colorFilter: ColorFilter.mode(logoColor, BlendMode.srcIn),
                  excludeFromSemantics: true,
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        children: [
                          Flexible(
                            child: Text(
                              accountName,
                              style: TextStyle(
                                color: palette.text,
                                fontSize: 14,
                                fontWeight: FontWeight.w600,
                              ),
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                          if (connection.isSelected) ...[
                            const SizedBox(width: 8),
                            Container(
                              padding: const EdgeInsets.symmetric(
                                horizontal: 6,
                                vertical: 2,
                              ),
                              decoration: BoxDecoration(
                                color: theme.colorScheme.primary.withValues(
                                  alpha: 0.15,
                                ),
                                borderRadius: BorderRadius.circular(4),
                              ),
                              child: Text(
                                l10n.activeAccountBadge,
                                style: TextStyle(
                                  color: theme.colorScheme.primary,
                                  fontSize: 11,
                                  fontWeight: FontWeight.w600,
                                ),
                              ),
                            ),
                          ],
                        ],
                      ),
                      const SizedBox(height: 2),
                      Text(
                        l10n.accountUsage(plan),
                        style: TextStyle(
                          color: palette.secondaryText,
                          fontSize: 12,
                        ),
                      ),
                    ],
                  ),
                ),
                IconButton(
                  tooltip: l10n.refreshUsage,
                  onPressed: () {
                    for (final workspace in connection.workspaces) {
                      unawaited(
                        _loadUsageForWorkspace(
                          connection,
                          workspace,
                          userInitiated: true,
                        ),
                      );
                    }
                  },
                  icon: const Icon(Icons.refresh_rounded, size: 18),
                ),
              ],
            ),
            const SizedBox(height: 12),
            SettingsDivider(color: palette.border),
            const SizedBox(height: 10),
            for (final (index, workspace) in connection.workspaces.indexed) ...[
              if (connection.workspaces.length > 1)
                Padding(
                  padding: const EdgeInsets.only(bottom: 8),
                  child: Text(
                    _workspaceQuotaHeading(workspace, index, l10n),
                    style: TextStyle(
                      color: palette.secondaryText,
                      fontSize: 12,
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                ),
              _buildWorkspaceUsage(connection, workspace, palette, l10n),
            ],
          ],
        ),
      ),
    );
  }

  String _workspaceQuotaHeading(
    ChatGptWorkspace workspace,
    int index,
    AppLocalizations l10n,
  ) {
    final name = workspace.displayName?.trim();
    return name == null || name.isEmpty
        ? l10n.workspaceNumbered(index + 1)
        : l10n.workspaceQuotaLabel(name);
  }

  Widget _buildWorkspaceUsage(
    ChatGptConnection connection,
    ChatGptWorkspace workspace,
    OpenChatPalette palette,
    AppLocalizations l10n,
  ) {
    final key = _workspaceKey(connection.id, workspace.id);
    final isLoading = _loadingKeys.contains(key);
    final error = _errors[key];
    final snapshot = _snapshots[key];

    if (isLoading && snapshot == null) {
      return const Padding(
        padding: EdgeInsets.symmetric(vertical: 16),
        child: Center(
          child: SizedBox.square(
            dimension: 20,
            child: CircularProgressIndicator(strokeWidth: 2),
          ),
        ),
      );
    }

    if (error != null && snapshot == null) {
      return Padding(
        padding: const EdgeInsets.symmetric(vertical: 8),
        child: Row(
          children: [
            Expanded(
              child: Text(
                error,
                style: TextStyle(
                  color: Theme.of(context).colorScheme.error,
                  fontSize: 12,
                ),
              ),
            ),
            TextButton.icon(
              onPressed: () => unawaited(
                _loadUsageForWorkspace(
                  connection,
                  workspace,
                  userInitiated: true,
                ),
              ),
              icon: const Icon(Icons.refresh_rounded, size: 16),
              label: Text(l10n.retry),
            ),
          ],
        ),
      );
    }

    if (snapshot == null) {
      return Padding(
        padding: const EdgeInsets.symmetric(vertical: 8),
        child: Text(
          l10n.ordinaryUsageUnknown,
          style: TextStyle(color: palette.secondaryText, fontSize: 12),
        ),
      );
    }

    final allowed = snapshot.ordinaryUsageAllowed;
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

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (isLoading) const LinearProgressIndicator(),
        Padding(
          padding: const EdgeInsets.symmetric(vertical: 6),
          child: Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Text(
                permissionLabel,
                style: TextStyle(
                  color: permissionColor,
                  fontSize: 12,
                  fontWeight: FontWeight.w500,
                ),
              ),
              Text(
                l10n.usageUpdatedAt(
                  _localizedTimestamp(context, snapshot.fetchedAtUnixMs),
                ),
                style: TextStyle(color: palette.secondaryText, fontSize: 11),
              ),
            ],
          ),
        ),
        for (final bucket in snapshot.buckets) ...[
          Padding(
            padding: const EdgeInsets.only(top: 8, bottom: 4),
            child: Row(
              children: [
                Expanded(
                  child: Text(
                    chatGptUsageBucketLabel(bucket, l10n),
                    style: TextStyle(
                      color: palette.text,
                      fontSize: 12,
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                ),
                Text(
                  bucket.usedPercent == null
                      ? l10n.unavailableValue
                      : l10n.usageUsedPercent(
                          bucket.usedPercent!.toStringAsFixed(0),
                        ),
                  style: TextStyle(color: palette.secondaryText, fontSize: 12),
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
              padding: const EdgeInsets.only(top: 4),
              child: Text(
                l10n.quotaResetsAt(_localizedTimestamp(context, resetAt)),
                style: TextStyle(color: palette.secondaryText, fontSize: 11),
              ),
            ),
        ],
        if (snapshot.resetCredits.isNotEmpty ||
            snapshot.resetCreditCount != null) ...[
          const SizedBox(height: 12),
          _buildResetCreditsCard(connection, workspace, snapshot),
        ],
      ],
    );
  }

  Widget _buildResetCreditsCard(
    ChatGptConnection connection,
    ChatGptWorkspace workspace,
    ChatGptUsageSnapshot snapshot,
  ) {
    final palette = OpenChatPalette.of(context);
    final l10n = context.openchatL10n;
    final theme = Theme.of(context);
    final resetCreditCount = snapshot.resetCreditCount;
    final key = _workspaceKey(connection.id, workspace.id);
    final needsRefresh = _refreshRequiredKeys.contains(key);

    return Container(
      decoration: BoxDecoration(
        color: theme.colorScheme.surfaceContainerHighest.withValues(
          alpha: 0.35,
        ),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: palette.border),
      ),
      padding: const EdgeInsets.all(12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Icon(
                Icons.restart_alt_rounded,
                color: palette.accentIcon,
                size: 18,
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  resetCreditCount == null
                      ? l10n.resetCreditCountUnavailable
                      : l10n.resetCreditsAvailable(resetCreditCount),
                  style: TextStyle(
                    color: palette.text,
                    fontSize: 12,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ),
            ],
          ),
          if (snapshot.resetCredits.isNotEmpty)
            for (
              var index = 0;
              index < snapshot.resetCredits.length;
              index++
            ) ...[
              if (index > 0)
                Padding(
                  padding: const EdgeInsets.symmetric(vertical: 8),
                  child: Divider(height: 1, color: palette.border),
                )
              else
                const SizedBox(height: 8),
              _buildResetCreditRow(
                connection,
                workspace,
                snapshot.resetCredits[index],
                needsRefresh: needsRefresh,
              ),
            ],
          if (needsRefresh)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Text(
                l10n.resetCreditRefreshRequired,
                style: TextStyle(color: theme.colorScheme.error, fontSize: 11),
              ),
            ),
        ],
      ),
    );
  }

  Widget _buildResetCreditRow(
    ChatGptConnection connection,
    ChatGptWorkspace workspace,
    ChatGptResetCredit credit, {
    required bool needsRefresh,
  }) {
    final palette = OpenChatPalette.of(context);
    final l10n = context.openchatL10n;
    final isAvailable = credit.status == 'available';
    final isConfirming = _confirmingResetCreditId == credit.id;
    final isRedeeming = _redeemingResetCreditId == credit.id;

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

    return Row(
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
                  fontSize: 12,
                  fontWeight: FontWeight.w500,
                ),
              ),
              if (metadata.isNotEmpty) ...[
                const SizedBox(height: 2),
                Text(
                  metadata.join(' · '),
                  style: TextStyle(color: palette.secondaryText, fontSize: 11),
                ),
              ],
            ],
          ),
        ),
        if (isAvailable) ...[
          const SizedBox(width: 8),
          FilledButton.icon(
            onPressed: isConfirming || isRedeeming || needsRefresh
                ? null
                : () => unawaited(
                    _redeemResetCredit(connection, workspace, credit),
                  ),
            icon: isRedeeming
                ? const SizedBox.square(
                    dimension: 14,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.restart_alt_rounded, size: 15),
            label: Text(
              isRedeeming ? l10n.resetCreditRedeeming : l10n.useResetCredit,
            ),
          ),
        ],
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

Map<String, Object?> _serviceObjectMap(Object? value) {
  if (value is! Map) {
    throw const FormatException('A ChatGPT response item was invalid.');
  }
  final result = <String, Object?>{};
  for (final entry in value.entries) {
    final key = entry.key;
    if (key is! String) {
      throw const FormatException('A ChatGPT response key was invalid.');
    }
    result[key] = entry.value;
  }
  return result;
}
