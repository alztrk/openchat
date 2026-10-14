import 'dart:async';

import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/open_url_or_copy.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/features/chat/presentation/widgets/provider_icon.dart';
import 'package:openchat/features/settings/data/open_code_api_key_store.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/features/settings/domain/provider_verification_record.dart';
import 'package:openchat/features/settings/domain/provider_request_outcome_record.dart';
import 'package:openchat/features/settings/presentation/provider_connection_status.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

class OpenCodeConnectionSection extends StatefulWidget {
  const OpenCodeConnectionSection({
    super.key,
    required this.apiKeyStore,
    required this.settingsPreferences,
    required this.onChanged,
    this.serviceClient,
  });

  final OpenCodeApiKeyStore? apiKeyStore;
  final SettingsPreferences settingsPreferences;
  final Future<void> Function()? onChanged;
  final OpenChatServiceClient? serviceClient;

  @override
  State<OpenCodeConnectionSection> createState() =>
      _OpenCodeConnectionSectionState();
}

class _OpenCodeConnectionSectionState extends State<OpenCodeConnectionSection> {
  final _controller = TextEditingController();
  String? _keySuffix;
  bool _loading = true;
  bool _saving = false;
  bool _showForm = false;
  String? _error;
  ProviderVerificationState _verificationState =
      ProviderVerificationState.notChecked;
  DateTime? _verificationCheckedAt;
  bool _verificationSaveFailed = false;
  ProviderRequestOutcomeRecord? _lastRequestOutcome;
  bool _lastRequestReadFailed = false;

  @override
  void initState() {
    super.initState();
    unawaited(_load());
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    final store = widget.apiKeyStore;
    if (store == null) {
      if (mounted) setState(() => _loading = false);
      return;
    }
    String? suffix;
    var requestOutcomeReadFailed = false;
    try {
      suffix = await store.readKeySuffix();
      final verification = await widget.settingsPreferences
          .readProviderVerificationRecord('opencode');
      ProviderRequestOutcomeRecord? requestOutcome;
      try {
        requestOutcome = await widget.settingsPreferences
            .readProviderRequestOutcomeForRoute(
              providerId: 'opencode',
              connectionId: 'opencode',
            );
      } on Object catch (error, stackTrace) {
        FlutterError.reportError(
          FlutterErrorDetails(
            exception: error,
            stack: stackTrace,
            library: 'provider_setup',
            context: ErrorDescription(
              'while reading the last OpenCode request',
            ),
          ),
        );
        requestOutcomeReadFailed = true;
      }
      if (mounted) {
        setState(() {
          _keySuffix = suffix;
          _verificationState = verification == null
              ? ProviderVerificationState.notChecked
              : _stateFor(verification.outcome);
          _verificationCheckedAt = verification?.checkedAt;
          _lastRequestOutcome = requestOutcome;
          _lastRequestReadFailed = requestOutcomeReadFailed;
          _loading = false;
        });
      }
    } on OpenCodeApiKeyStorageException {
      if (mounted) {
        setState(() {
          _keySuffix = suffix;
          _error = 'storage';
          _loading = false;
        });
      }
    } on FormatException catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'provider_setup',
          context: ErrorDescription(
            'while loading OpenCode verification state',
          ),
        ),
      );
      if (mounted) {
        setState(() {
          _keySuffix = suffix;
          _error = 'verification';
          _loading = false;
        });
      }
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'provider_setup',
          context: ErrorDescription('while loading OpenCode connection state'),
        ),
      );
      if (mounted) {
        setState(() {
          _keySuffix = suffix;
          _error = 'verification';
          _loading = false;
        });
      }
    }
  }

  ProviderVerificationState _stateFor(ProviderVerificationOutcome outcome) =>
      switch (outcome) {
        ProviderVerificationOutcome.verified =>
          ProviderVerificationState.verified,
        ProviderVerificationOutcome.emptyCatalog =>
          ProviderVerificationState.emptyCatalog,
        ProviderVerificationOutcome.staleCatalog =>
          ProviderVerificationState.staleCatalog,
        ProviderVerificationOutcome.authenticationFailed =>
          ProviderVerificationState.authenticationFailed,
        ProviderVerificationOutcome.rateLimited =>
          ProviderVerificationState.rateLimited,
        ProviderVerificationOutcome.networkFailed =>
          ProviderVerificationState.networkFailed,
        ProviderVerificationOutcome.failed => ProviderVerificationState.failed,
      };

  ProviderVerificationOutcome _outcomeFor(ProviderVerificationState state) =>
      switch (state) {
        ProviderVerificationState.verified =>
          ProviderVerificationOutcome.verified,
        ProviderVerificationState.emptyCatalog =>
          ProviderVerificationOutcome.emptyCatalog,
        ProviderVerificationState.staleCatalog =>
          ProviderVerificationOutcome.staleCatalog,
        ProviderVerificationState.authenticationFailed =>
          ProviderVerificationOutcome.authenticationFailed,
        ProviderVerificationState.rateLimited =>
          ProviderVerificationOutcome.rateLimited,
        ProviderVerificationState.networkFailed =>
          ProviderVerificationOutcome.networkFailed,
        ProviderVerificationState.failed => ProviderVerificationOutcome.failed,
        ProviderVerificationState.notChecked ||
        ProviderVerificationState.checking => throw StateError(
          'Only completed provider checks can be saved.',
        ),
      };

  Future<void> _recordVerification(ProviderVerificationState state) async {
    final checkedAt = DateTime.now().toUtc();
    var saveFailed = false;
    try {
      await widget.settingsPreferences.writeProviderVerificationRecord(
        'opencode',
        ProviderVerificationRecord(
          outcome: _outcomeFor(state),
          checkedAt: checkedAt,
        ),
      );
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'provider_setup',
          context: ErrorDescription('while saving OpenCode verification state'),
        ),
      );
      saveFailed = true;
    }
    if (!mounted) return;
    setState(() {
      _verificationState = state;
      _verificationCheckedAt = saveFailed ? null : checkedAt;
      _verificationSaveFailed = saveFailed;
    });
  }

  Future<void> _save() async {
    final store = widget.apiKeyStore;
    if (store == null || _saving || _isVerifying) return;
    setState(() {
      _saving = true;
      _error = null;
      _verificationState = ProviderVerificationState.notChecked;
      _verificationCheckedAt = null;
      _verificationSaveFailed = false;
    });
    try {
      await store.saveApiKey(_controller.text);
      await widget.settingsPreferences.clearProviderVerificationRecord(
        'opencode',
      );
      await widget.settingsPreferences.clearProviderRequestOutcomeForRoute(
        providerId: 'opencode',
        connectionId: 'opencode',
      );
      _controller.clear();
      await _load();
      await widget.onChanged?.call();
      if (mounted) setState(() => _showForm = false);
    } on InvalidOpenCodeApiKeyException {
      if (mounted) setState(() => _error = 'invalid');
    } on OpenCodeApiKeyStorageException {
      if (mounted) setState(() => _error = 'storage');
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'provider_setup',
          context: ErrorDescription('while resetting OpenCode verification'),
        ),
      );
      if (mounted) setState(() => _error = 'verification-save');
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  Future<void> _remove() async {
    final store = widget.apiKeyStore;
    if (store == null || _saving || _isVerifying) return;
    setState(() {
      _saving = true;
      _error = null;
      _verificationState = ProviderVerificationState.notChecked;
      _verificationCheckedAt = null;
      _verificationSaveFailed = false;
    });
    try {
      await store.deleteApiKey();
      await widget.settingsPreferences.clearProviderVerificationRecord(
        'opencode',
      );
      await widget.settingsPreferences.clearProviderRequestOutcomeForRoute(
        providerId: 'opencode',
        connectionId: 'opencode',
      );
      await _load();
      await widget.onChanged?.call();
    } on OpenCodeApiKeyStorageException {
      if (mounted) setState(() => _error = 'storage');
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'provider_setup',
          context: ErrorDescription('while resetting OpenCode verification'),
        ),
      );
      if (mounted) setState(() => _error = 'verification-save');
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  bool get _isVerifying =>
      _verificationState == ProviderVerificationState.checking;

  Future<void> _verifyConnection() async {
    final store = widget.apiKeyStore;
    final service = widget.serviceClient;
    if (store == null || service == null || _saving || _isVerifying) return;
    setState(() => _verificationState = ProviderVerificationState.checking);
    try {
      final apiKey = await store.readApiKey();
      if (apiKey == null) {
        if (mounted) {
          setState(
            () => _verificationState = ProviderVerificationState.notChecked,
          );
        }
        return;
      }
      final response = await service.call(
        'opencode.models.list',
        params: <String, Object?>{'apiKey': apiKey, 'forceRefresh': true},
      );
      final models = response['models'];
      if (models is! List<Object?>) {
        throw const FormatException('The provider model catalog was invalid.');
      }
      if (response['freshness'] == 'stale') {
        await _recordVerification(ProviderVerificationState.staleCatalog);
        return;
      }
      if (response['freshness'] != 'current') {
        throw const FormatException('The provider model catalog was invalid.');
      }
      await _recordVerification(
        models.isEmpty
            ? ProviderVerificationState.emptyCatalog
            : ProviderVerificationState.verified,
      );
    } on OpenChatServiceException catch (error) {
      await _recordVerification(providerVerificationFailureForCode(error.code));
    } on OpenCodeApiKeyStorageException {
      await _recordVerification(ProviderVerificationState.failed);
    } on FormatException {
      await _recordVerification(ProviderVerificationState.failed);
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'provider_setup',
          context: ErrorDescription('while verifying the OpenCode connection'),
        ),
      );
      await _recordVerification(ProviderVerificationState.failed);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final hasKey = _keySuffix != null;
    final isVerifying = _isVerifying;
    final actions = Wrap(
      spacing: 8,
      runSpacing: 8,
      children: [
        OutlinedButton.icon(
          onPressed: _saving || isVerifying
              ? null
              : () => setState(() => _showForm = !_showForm),
          icon: Icon(hasKey ? LucideIcons.pencil : LucideIcons.plus, size: 16),
          label: Text(hasKey ? l10n.edit : l10n.add),
        ),
        if (hasKey)
          TextButton.icon(
            onPressed: _saving || isVerifying ? null : _remove,
            icon: const Icon(LucideIcons.trash2, size: 16),
            label: Text(l10n.deleteAll),
          ),
      ],
    );
    return Card(
      margin: EdgeInsets.zero,
      color: palette.surface,
      elevation: 0,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(OpenChatRadii.card),
        side: BorderSide(color: palette.border),
      ),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                ProviderIcon(
                  providerId: 'opencode',
                  color: palette.text,
                  size: 24,
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        l10n.openCodeConsole,
                        style: Theme.of(context).textTheme.titleMedium,
                      ),
                      const SizedBox(height: 4),
                      Text(
                        l10n.openCodeConsoleDescription,
                        style: TextStyle(
                          color: palette.secondaryText,
                          fontSize: 13,
                        ),
                      ),
                      const SizedBox(height: 4),
                      TextButton(
                        onPressed: () => unawaited(
                          openUrlOrCopy(context, 'https://opencode.ai/zen'),
                        ),
                        style: TextButton.styleFrom(
                          alignment: Alignment.centerLeft,
                          foregroundColor: palette.secondaryText,
                          minimumSize: const Size(0, 44),
                          padding: EdgeInsets.zero,
                        ),
                        child: const Text(
                          'https://opencode.ai/zen',
                          style: TextStyle(
                            fontSize: 12,
                            decoration: TextDecoration.underline,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
              ],
            ),
            if (_error == 'storage' && !_showForm) ...[
              const SizedBox(height: 10),
              Text(
                l10n.openCodeKeyStorageFailed,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ],
            if (_error == 'verification' && !_showForm) ...[
              const SizedBox(height: 10),
              Text(
                l10n.providerVerificationRecordUnreadable,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ],
            if (_error == 'verification-save' && !_showForm) ...[
              const SizedBox(height: 10),
              Text(
                l10n.providerVerificationResultNotSaved,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ],
            const SizedBox(height: 12),
            if (_loading) const LinearProgressIndicator(),
            if (!_loading)
              LayoutBuilder(
                builder: (context, constraints) {
                  final status = Text(
                    hasKey
                        ? l10n.openCodeKeySaved(_keySuffix!)
                        : l10n.openCodeNoKey,
                  );
                  if (constraints.maxWidth /
                          MediaQuery.textScalerOf(context).scale(1) <
                      520) {
                    return Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        status,
                        const SizedBox(height: 8),
                        Align(alignment: Alignment.centerRight, child: actions),
                      ],
                    );
                  }
                  return Row(
                    children: [
                      Expanded(child: status),
                      const SizedBox(width: 12),
                      actions,
                    ],
                  );
                },
              ),
            if (hasKey && !_loading)
              ProviderConnectionStatus(
                state: _verificationState,
                checkedAt: _verificationCheckedAt,
                saveFailed: _verificationSaveFailed,
                lastRequestOutcome: _lastRequestOutcome,
                lastRequestReadFailed: _lastRequestReadFailed,
                onVerify: isVerifying || _saving || widget.serviceClient == null
                    ? null
                    : _verifyConnection,
              ),
            if (_showForm)
              Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  TextField(
                    controller: _controller,
                    obscureText: true,
                    autocorrect: false,
                    enableSuggestions: false,
                    decoration: InputDecoration(
                      labelText: l10n.openCodeApiKey,
                      errorText: _error == 'invalid'
                          ? l10n.openCodeKeyInvalid
                          : _error == 'storage'
                          ? l10n.openCodeKeyStorageFailed
                          : null,
                    ),
                    onSubmitted: (_) => unawaited(_save()),
                  ),
                  const SizedBox(height: 12),
                  Align(
                    alignment: Alignment.centerRight,
                    child: FilledButton(
                      onPressed: _saving ? null : _save,
                      child: Text(_saving ? l10n.saving : l10n.save),
                    ),
                  ),
                ],
              ),
          ],
        ),
      ),
    );
  }
}
