import 'package:flutter/material.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/features/settings/domain/provider_request_outcome_record.dart';

enum ProviderVerificationState {
  notChecked,
  checking,
  verified,
  emptyCatalog,
  staleCatalog,
  authenticationFailed,
  rateLimited,
  networkFailed,
  failed,
}

ProviderVerificationState providerVerificationFailureForCode(String code) =>
    switch (code) {
      'authentication_required' =>
        ProviderVerificationState.authenticationFailed,
      'rate_limited' ||
      'quota_exceeded' ||
      'insufficient_quota' => ProviderVerificationState.rateLimited,
      'network_unavailable' => ProviderVerificationState.networkFailed,
      _ => ProviderVerificationState.failed,
    };

class ProviderConnectionStatus extends StatelessWidget {
  const ProviderConnectionStatus({
    required this.state,
    required this.onVerify,
    this.checkedAt,
    this.saveFailed = false,
    this.recordReadFailed = false,
    this.showQuotaUnavailable = true,
    this.lastRequestOutcome,
    this.lastRequestReadFailed = false,
    this.lastRequestSaveFailed = false,
    super.key,
  });

  final ProviderVerificationState state;
  final VoidCallback? onVerify;
  final DateTime? checkedAt;
  final bool saveFailed;
  final bool recordReadFailed;
  final bool showQuotaUnavailable;
  final ProviderRequestOutcomeRecord? lastRequestOutcome;
  final bool lastRequestReadFailed;
  final bool lastRequestSaveFailed;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final message = switch (state) {
      ProviderVerificationState.notChecked =>
        l10n.providerConnectionNotVerified,
      ProviderVerificationState.checking => l10n.verifyingProviderConnection,
      ProviderVerificationState.verified => l10n.providerConnectionVerified,
      ProviderVerificationState.emptyCatalog => l10n.providerModelCatalogEmpty,
      ProviderVerificationState.staleCatalog =>
        l10n.providerVerificationStaleCatalog,
      ProviderVerificationState.authenticationFailed =>
        l10n.providerVerificationAuthenticationFailed,
      ProviderVerificationState.rateLimited =>
        l10n.providerVerificationRateLimited,
      ProviderVerificationState.networkFailed =>
        l10n.providerVerificationNetworkFailed,
      ProviderVerificationState.failed =>
        l10n.providerConnectionVerificationFailed,
    };
    final color = switch (state) {
      ProviderVerificationState.verified => palette.success,
      ProviderVerificationState.staleCatalog => palette.warning,
      ProviderVerificationState.authenticationFailed ||
      ProviderVerificationState.rateLimited ||
      ProviderVerificationState.networkFailed ||
      ProviderVerificationState.failed => Theme.of(context).colorScheme.error,
      _ => palette.secondaryText,
    };
    final requestRecord = lastRequestOutcome;
    final requestOutcomeLabel = switch (requestRecord?.outcome) {
      ProviderRequestOutcome.completed => l10n.providerRequestCompleted,
      ProviderRequestOutcome.waitingForInput =>
        l10n.providerRequestWaitingForInput,
      ProviderRequestOutcome.stopped => l10n.providerRequestStopped,
      ProviderRequestOutcome.authenticationFailed =>
        l10n.providerRequestAuthenticationFailed,
      ProviderRequestOutcome.rateLimited => l10n.providerRequestRateLimited,
      ProviderRequestOutcome.networkFailed => l10n.providerRequestNetworkFailed,
      ProviderRequestOutcome.failed => l10n.providerRequestFailed,
      null => null,
    };

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const SizedBox(height: 8),
        Semantics(
          liveRegion: true,
          child: Text(message, style: TextStyle(color: color, fontSize: 13)),
        ),
        if (checkedAt != null) ...[
          const SizedBox(height: 4),
          Text(
            l10n.providerConnectionLastChecked(
              '${MaterialLocalizations.of(context).formatMediumDate(checkedAt!.toLocal())} ${MaterialLocalizations.of(context).formatTimeOfDay(TimeOfDay.fromDateTime(checkedAt!.toLocal()))}',
            ),
            style: TextStyle(color: palette.secondaryText, fontSize: 12),
          ),
        ],
        if (requestRecord != null && requestOutcomeLabel != null) ...[
          const SizedBox(height: 8),
          Text(
            l10n.providerLastRequestOutcome(requestOutcomeLabel),
            style: TextStyle(color: palette.secondaryText, fontSize: 12),
          ),
          Text(
            l10n.providerLastRequestAt(
              '${MaterialLocalizations.of(context).formatMediumDate(requestRecord.occurredAt.toLocal())} ${MaterialLocalizations.of(context).formatTimeOfDay(TimeOfDay.fromDateTime(requestRecord.occurredAt.toLocal()))}',
            ),
            style: TextStyle(color: palette.secondaryText, fontSize: 12),
          ),
        ],
        if (lastRequestReadFailed) ...[
          const SizedBox(height: 4),
          Text(
            l10n.providerLastRequestReadFailed,
            style: TextStyle(
              color: Theme.of(context).colorScheme.error,
              fontSize: 12,
            ),
          ),
        ],
        if (lastRequestSaveFailed) ...[
          const SizedBox(height: 4),
          Text(
            l10n.providerLastRequestSaveFailed,
            style: TextStyle(
              color: Theme.of(context).colorScheme.error,
              fontSize: 12,
            ),
          ),
        ],
        if (saveFailed) ...[
          const SizedBox(height: 4),
          Text(
            l10n.providerVerificationResultNotSaved,
            style: TextStyle(
              color: Theme.of(context).colorScheme.error,
              fontSize: 12,
            ),
          ),
        ],
        if (recordReadFailed) ...[
          const SizedBox(height: 4),
          Text(
            l10n.providerVerificationRecordUnreadable,
            style: TextStyle(
              color: Theme.of(context).colorScheme.error,
              fontSize: 12,
            ),
          ),
        ],
        if (showQuotaUnavailable) ...[
          const SizedBox(height: 4),
          Text(
            l10n.providerQuotaUnavailable,
            style: TextStyle(color: palette.secondaryText, fontSize: 12),
          ),
        ],
      ],
    );
  }
}
