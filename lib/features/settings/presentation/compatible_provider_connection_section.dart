import 'dart:async';

import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/open_url_or_copy.dart';
import 'package:openchat/features/chat/presentation/widgets/provider_icon.dart';
import 'package:openchat/features/settings/data/api_compatible_provider_key_store.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/features/settings/domain/provider_verification_record.dart';
import 'package:openchat/features/settings/domain/provider_request_outcome_record.dart';
import 'package:openchat/features/settings/presentation/provider_connection_status.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

class CompatibleProviderConnectionSection extends StatefulWidget {
  const CompatibleProviderConnectionSection({
    required this.providerId,
    required this.apiKeyStore,
    required this.settingsPreferences,
    this.serviceClient,
    required this.onChanged,
    super.key,
  });

  final String providerId;
  final ApiCompatibleProviderKeyStore? apiKeyStore;
  final SettingsPreferences settingsPreferences;
  final OpenChatServiceClient? serviceClient;
  final Future<void> Function()? onChanged;

  @override
  State<CompatibleProviderConnectionSection> createState() =>
      _CompatibleProviderConnectionSectionState();
}

class _CompatibleProviderConnectionSectionState
    extends State<CompatibleProviderConnectionSection> {
  final _controller = TextEditingController();
  ApiCompatibleProviderKeyStatus? _status;
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
    ApiCompatibleProviderKeyStatus? status;
    var requestOutcomeReadFailed = false;
    try {
      status = await store.readStatus(widget.providerId);
      final verification = await widget.settingsPreferences
          .readProviderVerificationRecord(widget.providerId);
      ProviderRequestOutcomeRecord? requestOutcome;
      try {
        requestOutcome = await widget.settingsPreferences
            .readProviderRequestOutcomeForRoute(
              providerId: widget.providerId,
              connectionId: widget.providerId,
            );
      } on Object catch (error, stackTrace) {
        FlutterError.reportError(
          FlutterErrorDetails(
            exception: error,
            stack: stackTrace,
            library: 'provider_setup',
            context: ErrorDescription(
              'while reading the last provider request',
            ),
          ),
        );
        requestOutcomeReadFailed = true;
      }
      if (mounted) {
        setState(() {
          _status = status;
          _verificationState = verification == null
              ? ProviderVerificationState.notChecked
              : _stateFor(verification.outcome);
          _verificationCheckedAt = verification?.checkedAt;
          _lastRequestOutcome = requestOutcome;
          _lastRequestReadFailed = requestOutcomeReadFailed;
          _loading = false;
          _error = null;
        });
      }
    } on ApiCompatibleProviderKeyStorageException {
      if (mounted) {
        setState(() {
          _status = status;
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
            'while loading provider verification state',
          ),
        ),
      );
      if (mounted) {
        setState(() {
          _status = status;
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
          context: ErrorDescription('while loading provider connection state'),
        ),
      );
      if (mounted) {
        setState(() {
          _status = status;
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
        widget.providerId,
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
          context: ErrorDescription('while saving provider verification state'),
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
      await store.saveApiKey(widget.providerId, _controller.text);
      await widget.settingsPreferences.clearProviderVerificationRecord(
        widget.providerId,
      );
      await widget.settingsPreferences.clearProviderRequestOutcomeForRoute(
        providerId: widget.providerId,
        connectionId: widget.providerId,
      );
      _controller.clear();
      await _load();
      await widget.onChanged?.call();
      if (mounted) setState(() => _showForm = false);
    } on InvalidApiCompatibleProviderKeyException {
      if (mounted) setState(() => _error = 'invalid');
    } on ApiCompatibleProviderKeyStorageException {
      if (mounted) setState(() => _error = 'storage');
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'provider_setup',
          context: ErrorDescription('while resetting provider verification'),
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
      await store.deleteApiKey(widget.providerId);
      await widget.settingsPreferences.clearProviderVerificationRecord(
        widget.providerId,
      );
      await widget.settingsPreferences.clearProviderRequestOutcomeForRoute(
        providerId: widget.providerId,
        connectionId: widget.providerId,
      );
      await _load();
      await widget.onChanged?.call();
    } on ApiCompatibleProviderKeyStorageException {
      if (mounted) setState(() => _error = 'storage');
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'provider_setup',
          context: ErrorDescription('while resetting provider verification'),
        ),
      );
      if (mounted) setState(() => _error = 'verification-save');
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  Future<void> _verifyConnection() async {
    final store = widget.apiKeyStore;
    final service = widget.serviceClient;
    if (store == null || service == null || _saving || _isVerifying) return;
    setState(() {
      _verificationState = ProviderVerificationState.checking;
    });
    try {
      final apiKey = await store.readApiKey(widget.providerId);
      if (apiKey == null) {
        if (mounted) {
          setState(() {
            _verificationState = ProviderVerificationState.notChecked;
          });
        }
        return;
      }
      CustomCompatibleProvider? customProvider;
      if (ApiCompatibleProviderKeyStore.isCustomProviderId(widget.providerId)) {
        final customProviders = await store.readCustomProviders();
        for (final provider in customProviders) {
          if (provider.id == widget.providerId) {
            customProvider = provider;
            break;
          }
        }
      }
      final response = await service.call(
        'compatible.models.list',
        params: <String, Object?>{
          'providerId': widget.providerId,
          'apiKey': apiKey,
          if (customProvider != null) ...<String, Object?>{
            'baseUrl': customProvider.baseUrl,
            'protocol': customProvider.protocol.name,
          },
        },
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
      if (!mounted) return;
      await _recordVerification(
        models.isEmpty
            ? ProviderVerificationState.emptyCatalog
            : ProviderVerificationState.verified,
      );
    } on OpenChatServiceException catch (error) {
      await _recordVerification(providerVerificationFailureForCode(error.code));
    } on ApiCompatibleProviderKeyStorageException {
      await _recordVerification(ProviderVerificationState.failed);
    } on FormatException {
      await _recordVerification(ProviderVerificationState.failed);
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'provider_setup',
          context: ErrorDescription('while verifying a provider connection'),
        ),
      );
      await _recordVerification(ProviderVerificationState.failed);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final configured = _status?.isConfigured == true;
    final name = _name(l10n);
    final description = _description(l10n);
    final url = _url;
    final isVerifying = _isVerifying;
    final actions = Wrap(
      spacing: 8,
      runSpacing: 8,
      children: [
        OutlinedButton.icon(
          onPressed: _saving || isVerifying
              ? null
              : () => setState(() => _showForm = !_showForm),
          icon: Icon(
            configured ? LucideIcons.pencil : LucideIcons.plus,
            size: 16,
          ),
          label: Text(configured ? l10n.edit : l10n.add),
        ),
        if (configured)
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
                  providerId: widget.providerId,
                  color: palette.text,
                  size: 24,
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        name,
                        style: Theme.of(context).textTheme.titleMedium,
                      ),
                      const SizedBox(height: 4),
                      Text(
                        description,
                        style: TextStyle(
                          color: palette.secondaryText,
                          fontSize: 13,
                        ),
                      ),
                      if (url.isNotEmpty) ...[
                        const SizedBox(height: 4),
                        TextButton(
                          onPressed: () =>
                              unawaited(openUrlOrCopy(context, url)),
                          style: TextButton.styleFrom(
                            alignment: Alignment.centerLeft,
                            foregroundColor: palette.secondaryText,
                            minimumSize: const Size(0, 44),
                            padding: EdgeInsets.zero,
                          ),
                          child: Text(
                            url,
                            style: const TextStyle(
                              fontSize: 12,
                              decoration: TextDecoration.underline,
                            ),
                          ),
                        ),
                      ],
                    ],
                  ),
                ),
              ],
            ),
            if (widget.providerId == 'gemini') ...[
              const SizedBox(height: 12),
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Icon(
                    LucideIcons.info,
                    size: 16,
                    color: palette.secondaryIcon,
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      l10n.geminiUnpaidDataNotice,
                      style: TextStyle(
                        color: palette.secondaryText,
                        fontSize: 12,
                        height: 1.4,
                      ),
                    ),
                  ),
                ],
              ),
            ],
            if (_error == 'storage' && !_showForm) ...[
              const SizedBox(height: 10),
              Text(
                l10n.providerKeyStorageFailed,
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
            if (_loading)
              const LinearProgressIndicator()
            else
              LayoutBuilder(
                builder: (context, constraints) {
                  final suffix = _status?.keySuffix;
                  final status = Text(
                    configured
                        ? suffix == null
                              ? l10n.providerKeySaved
                              : l10n.providerKeySavedSuffix(suffix)
                        : l10n.providerNoKey,
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
            if (configured && !_loading) ...[
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
            ],
            if (_showForm) ...[
              const SizedBox(height: 12),
              Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  TextField(
                    controller: _controller,
                    obscureText: true,
                    autocorrect: false,
                    enableSuggestions: false,
                    decoration: InputDecoration(
                      labelText: l10n.providerApiKey,
                      errorText: _error == 'invalid'
                          ? l10n.providerKeyInvalid
                          : _error == 'storage'
                          ? l10n.providerKeyStorageFailed
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
          ],
        ),
      ),
    );
  }

  String _name(AppLocalizations l10n) => switch (widget.providerId) {
    'gemini' => l10n.geminiProvider,
    'groq' => l10n.groqProvider,
    'cerebras' => l10n.cerebrasProvider,
    'openrouter' => l10n.openRouterProvider,
    'mistral' => l10n.mistralProvider,
    'deepseek' => l10n.deepSeekProvider,
    'xai' => l10n.xAiProvider,
    'perplexity' => l10n.perplexityProvider,
    'together' => l10n.togetherProvider,
    'sambanova' => l10n.sambaNovaProvider,
    'anthropic' => l10n.anthropicProvider,
    'fireworks' => l10n.fireworksProvider,
    'deepinfra' => l10n.deepInfraProvider,
    'dashscope' => l10n.dashScopeProvider,
    'cohere' => l10n.cohereProvider,
    _ => widget.providerId,
  };

  String _description(AppLocalizations l10n) => switch (widget.providerId) {
    'gemini' => l10n.geminiApiDescription,
    'groq' => l10n.groqApiDescription,
    'cerebras' => l10n.cerebrasApiDescription,
    'openrouter' => l10n.openRouterApiDescription,
    'mistral' => l10n.mistralApiDescription,
    'deepseek' => l10n.deepSeekApiDescription,
    'xai' => l10n.xAiApiDescription,
    'perplexity' => l10n.perplexityApiDescription,
    'together' => l10n.togetherApiDescription,
    'sambanova' => l10n.sambaNovaApiDescription,
    'anthropic' => l10n.anthropicApiDescription,
    'fireworks' => l10n.fireworksApiDescription,
    'deepinfra' => l10n.deepInfraApiDescription,
    'dashscope' => l10n.dashScopeApiDescription,
    'cohere' => l10n.cohereApiDescription,
    _ => '',
  };

  String get _url => switch (widget.providerId) {
    'gemini' => 'https://ai.google.dev/gemini-api/docs',
    'groq' => 'https://console.groq.com/docs',
    'cerebras' => 'https://docs.cerebras.ai/',
    'openrouter' => 'https://openrouter.ai/docs',
    'mistral' => 'https://docs.mistral.ai/',
    'deepseek' => 'https://api-docs.deepseek.com/',
    'xai' => 'https://docs.x.ai/',
    'perplexity' => 'https://docs.perplexity.ai/',
    'together' => 'https://docs.together.ai/',
    'sambanova' => 'https://docs.sambanova.ai/',
    'anthropic' => 'https://platform.claude.com/docs/en/api/overview',
    'fireworks' => 'https://docs.fireworks.ai/',
    'deepinfra' => 'https://docs.deepinfra.com/',
    'dashscope' => 'https://www.alibabacloud.com/help/en/model-studio/',
    'cohere' => 'https://docs.cohere.com/',
    _ => '',
  };

  bool get _isVerifying =>
      _verificationState == ProviderVerificationState.checking;
}
