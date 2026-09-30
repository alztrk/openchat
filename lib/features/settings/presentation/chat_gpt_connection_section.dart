import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/openchat_toast.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';
import 'package:openchat/features/chat/domain/chatgpt_connection.dart';
import 'package:openchat/features/settings/data/chat_gpt_api_key_store.dart';
import 'package:openchat/features/settings/domain/chat_gpt_api_key_connection.dart';
import 'package:openchat/features/settings/domain/chat_gpt_usage_snapshot.dart';
import 'package:openchat/features/settings/presentation/chat_gpt_usage_details.dart';
import 'package:openchat/features/settings/presentation/chat_gpt_oauth_connection_row.dart';
import 'package:openchat/features/settings/presentation/chat_gpt_title_preference_section.dart';
import 'package:openchat/features/settings/presentation/chat_gpt_api_key_form.dart';
import 'package:openchat/features/settings/presentation/settings_widgets.dart';

enum _ConnectionLoadState { loading, loaded, failed }

enum _UsageLoadState { idle, loading, loaded, failed }

class ChatGptConnectionSection extends StatefulWidget {
  const ChatGptConnectionSection({
    super.key,
    required this.apiKeyStore,
    required this.serviceClient,
    required this.onProviderStateChanged,
    required this.onConnectionRemoved,
  });

  final ChatGptApiKeyStore? apiKeyStore;
  final OpenChatServiceClient? serviceClient;
  final Future<void> Function()? onProviderStateChanged;
  final Future<void> Function(String connectionId)? onConnectionRemoved;

  @override
  State<ChatGptConnectionSection> createState() =>
      _ChatGptConnectionSectionState();
}

class _ChatGptConnectionSectionState extends State<ChatGptConnectionSection> {
  final _apiKeyController = TextEditingController();
  List<ChatGptApiKeyConnection> _connections = const [];
  List<ChatGptConnection> _oauthConnections = const [];
  _ConnectionLoadState _loadState = _ConnectionLoadState.loading;
  _ConnectionLoadState _oauthLoadState = _ConnectionLoadState.loading;
  _ConnectionLoadState _titlePreferenceLoadState = _ConnectionLoadState.loading;
  _UsageLoadState _usageLoadState = _UsageLoadState.idle;
  ChatGptUsageSnapshot? _usageSnapshot;
  String? _confirmingResetCreditId;
  String? _redeemingResetCreditId;
  String? _resetCreditRefreshRequiredConnectionId;
  String? _resetCreditRefreshRequiredWorkspaceId;
  ChatGptTitlePreference _titlePreference = const ChatGptTitlePreference();
  OpenChatServiceOperation? _oauthOperation;
  String? _oauthError;
  String? _selectedWorkspaceId;
  String? _formError;
  bool _showApiKeyForm = false;
  bool _isSaving = false;
  bool _isSigningIn = false;
  bool _isSavingTitlePreference = false;
  final Set<String> _removingConnectionIds = <String>{};
  int _usageRequestGeneration = 0;
  int _loadRequestGeneration = 0;

  @override
  void initState() {
    super.initState();
    unawaited(_loadConnections());
    unawaited(_loadOAuthConnections());
  }

  @override
  void dispose() {
    final operation = _oauthOperation;
    if (operation != null) unawaited(operation.cancel());
    _apiKeyController.dispose();
    super.dispose();
  }

  Future<void> _loadConnections() async {
    final store = widget.apiKeyStore;
    final requestGeneration = ++_loadRequestGeneration;
    if (store == null) {
      setState(() => _loadState = _ConnectionLoadState.loaded);
      return;
    }

    setState(() => _loadState = _ConnectionLoadState.loading);
    try {
      final connections = await store.readConnections();
      if (!mounted || requestGeneration != _loadRequestGeneration) return;
      setState(() {
        _connections = connections;
        _loadState = _ConnectionLoadState.loaded;
      });
    } on ChatGptApiKeyStorageException {
      if (!mounted || requestGeneration != _loadRequestGeneration) return;
      setState(() => _loadState = _ConnectionLoadState.failed);
    }
  }

  Future<void> _loadOAuthConnections() async {
    final service = widget.serviceClient;
    if (service == null) {
      if (mounted) {
        setState(() => _oauthLoadState = _ConnectionLoadState.loaded);
      }
      return;
    }
    setState(() => _oauthLoadState = _ConnectionLoadState.loading);
    try {
      final response = await service.call('chatgpt.connections.list');
      final rawConnections = response['connections'];
      if (rawConnections is! List<Object?>) {
        throw const FormatException('The ChatGPT connection list was invalid.');
      }
      final connections = rawConnections
          .map((value) => ChatGptConnection.fromJson(_serviceObjectMap(value)))
          .toList(growable: false);
      ChatGptConnection? selectedConnection;
      for (final connection in connections) {
        if (connection.isSelected) {
          selectedConnection = connection;
          break;
        }
      }
      ChatGptWorkspace? selectedWorkspace;
      for (final workspace in selectedConnection?.workspaces ?? const []) {
        if (workspace.isSelected) {
          selectedWorkspace = workspace;
          break;
        }
      }
      if (!mounted) return;
      setState(() {
        _oauthConnections = connections;
        _selectedWorkspaceId = selectedWorkspace?.id;
        _oauthLoadState = _ConnectionLoadState.loaded;
      });
      unawaited(_loadTitlePreference());
      if (selectedConnection != null && selectedWorkspace != null) {
        unawaited(_loadUsage(selectedConnection.id, selectedWorkspace.id));
      } else {
        setState(() {
          _usageSnapshot = null;
          _usageLoadState = _UsageLoadState.idle;
        });
      }
    } on OpenChatServiceException {
      if (!mounted) return;
      setState(() => _oauthLoadState = _ConnectionLoadState.failed);
    } on FormatException {
      if (!mounted) return;
      setState(() => _oauthLoadState = _ConnectionLoadState.failed);
    }
  }

  Future<void> _loadTitlePreference() async {
    final service = widget.serviceClient;
    if (service == null) {
      if (mounted) {
        setState(() => _titlePreferenceLoadState = _ConnectionLoadState.loaded);
      }
      return;
    }
    setState(() => _titlePreferenceLoadState = _ConnectionLoadState.loading);
    try {
      final response = await service.call('chatgpt.titles.get');
      final preference = ChatGptTitlePreference.fromJson(response);
      if (!mounted) return;
      setState(() {
        _titlePreference = preference;
        _titlePreferenceLoadState = _ConnectionLoadState.loaded;
      });
    } on OpenChatServiceException {
      if (!mounted) return;
      setState(() => _titlePreferenceLoadState = _ConnectionLoadState.failed);
    } on FormatException {
      if (!mounted) return;
      setState(() => _titlePreferenceLoadState = _ConnectionLoadState.failed);
    }
  }

  Future<void> _selectTitleConnection(String? connectionId) async {
    if (connectionId == null || _isSavingTitlePreference) return;
    final selectedId = connectionId.isEmpty ? null : connectionId;
    String? workspaceId;
    if (selectedId != null) {
      for (final connection in _oauthConnections) {
        if (connection.id == selectedId && connection.workspaces.length == 1) {
          workspaceId = connection.workspaces.single.id;
          break;
        }
      }
    }
    await _saveTitlePreference(selectedId, workspaceId);
  }

  Future<void> _selectTitleWorkspace(String? workspaceId) async {
    if (workspaceId == null || _isSavingTitlePreference) return;
    final connectionId = _titlePreference.connectionId;
    if (connectionId == null) return;
    await _saveTitlePreference(connectionId, workspaceId);
  }

  Future<void> _saveTitlePreference(
    String? connectionId,
    String? workspaceId,
  ) async {
    final service = widget.serviceClient;
    if (service == null) return;
    setState(() => _isSavingTitlePreference = true);
    try {
      final response = await service.call(
        'chatgpt.titles.select',
        params: <String, Object?>{
          'connectionId': connectionId,
          'workspaceId': workspaceId,
        },
      );
      final preference = ChatGptTitlePreference.fromJson(response);
      if (!mounted) return;
      setState(() {
        _titlePreference = preference;
        _titlePreferenceLoadState = _ConnectionLoadState.loaded;
      });
    } on OpenChatServiceException {
      if (mounted) _showMessage(context.openchatL10n.titlePreferenceSaveFailed);
    } on FormatException {
      if (mounted) _showMessage(context.openchatL10n.providerDataUnavailable);
    } finally {
      if (mounted) setState(() => _isSavingTitlePreference = false);
    }
  }

  Future<void> _startOAuth() async {
    final service = widget.serviceClient;
    if (service == null || _isSigningIn || _isResetCreditActionPending) {
      if (service == null) {
        _showMessage(context.openchatL10n.oauthConnectionUnavailable);
      }
      return;
    }
    setState(() {
      _isSigningIn = true;
      _oauthError = null;
    });
    try {
      final operation = await service.startOperation('chatgpt.oauth.start');
      _oauthOperation = operation;
      final result = await operation.result;
      final cleanupWarning = result['oldCredentialCleanupFailed'];
      if (cleanupWarning != null && cleanupWarning is! bool) {
        throw const FormatException(
          'The ChatGPT connection result was invalid.',
        );
      }
      if (!mounted) return;
      await _loadOAuthConnections();
      await widget.onProviderStateChanged?.call();
      if (mounted) {
        showOpenChatToast(
          context,
          cleanupWarning == true
              ? context.openchatL10n.oldCredentialCleanupFailed
              : context.openchatL10n.oauthConnectionAdded,
          type: cleanupWarning == true
              ? OpenChatToastType.warning
              : OpenChatToastType.success,
        );
      }
    } on OpenChatServiceException catch (error) {
      if (!mounted || error.code == 'oauth_cancelled') return;
      setState(
        () => _oauthError =
            '${context.openchatL10n.oauthSignInFailed} (${error.code})',
      );
    } on FormatException {
      if (mounted) {
        setState(() => _oauthError = context.openchatL10n.oauthResponseInvalid);
      }
    } finally {
      _oauthOperation = null;
      if (mounted) setState(() => _isSigningIn = false);
    }
  }

  void _cancelOAuth() {
    final operation = _oauthOperation;
    if (operation != null) unawaited(operation.cancel());
  }

  Future<void> _selectOAuthConnection(ChatGptConnection connection) async {
    final service = widget.serviceClient;
    if (service == null ||
        connection.isSelected ||
        _isResetCreditActionPending) {
      return;
    }
    try {
      await service.call(
        'chatgpt.connections.select',
        params: <String, Object?>{'connectionId': connection.id},
      );
      await _loadOAuthConnections();
      await widget.onProviderStateChanged?.call();
    } on OpenChatServiceException {
      if (mounted) _showMessage(context.openchatL10n.connectionSelectionFailed);
    }
  }

  Future<void> _selectOAuthWorkspace(
    ChatGptConnection connection,
    String workspaceId,
  ) async {
    final service = widget.serviceClient;
    if (service == null ||
        workspaceId == _selectedWorkspaceId ||
        _isResetCreditActionPending) {
      return;
    }
    try {
      await service.call(
        'chatgpt.workspaces.select',
        params: <String, Object?>{
          'connectionId': connection.id,
          'workspaceId': workspaceId,
        },
      );
      await _loadOAuthConnections();
      await widget.onProviderStateChanged?.call();
    } on OpenChatServiceException {
      if (mounted) _showMessage(context.openchatL10n.workspaceSelectionFailed);
    }
  }

  Future<void> _loadUsage(
    String connectionId,
    String workspaceId, {
    bool userInitiated = false,
  }) async {
    final service = widget.serviceClient;
    if (service == null) return;
    final generation = ++_usageRequestGeneration;
    setState(() {
      _usageLoadState = _UsageLoadState.loading;
      _usageSnapshot = null;
    });
    try {
      final response = await service.call(
        'chatgpt.usage.get',
        params: <String, Object?>{
          'connectionId': connectionId,
          'workspaceId': workspaceId,
        },
      );
      final snapshot = ChatGptUsageSnapshot.fromJson(response);
      if (!mounted || generation != _usageRequestGeneration) return;
      setState(() {
        _usageSnapshot = snapshot;
        _usageLoadState = _UsageLoadState.loaded;
        if (userInitiated &&
            _resetCreditRefreshRequiredConnectionId == connectionId &&
            _resetCreditRefreshRequiredWorkspaceId == workspaceId) {
          _resetCreditRefreshRequiredConnectionId = null;
          _resetCreditRefreshRequiredWorkspaceId = null;
        }
      });
    } on OpenChatServiceException {
      if (!mounted || generation != _usageRequestGeneration) return;
      setState(() => _usageLoadState = _UsageLoadState.failed);
    } on FormatException {
      if (!mounted || generation != _usageRequestGeneration) return;
      setState(() => _usageLoadState = _UsageLoadState.failed);
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while loading ChatGPT usage'),
        ),
      );
      if (!mounted || generation != _usageRequestGeneration) return;
      setState(() => _usageLoadState = _UsageLoadState.failed);
    }
  }

  bool _needsResetCreditRefresh(String connectionId, String workspaceId) =>
      _resetCreditRefreshRequiredConnectionId == connectionId &&
      _resetCreditRefreshRequiredWorkspaceId == workspaceId;

  bool get _isResetCreditActionPending =>
      _confirmingResetCreditId != null || _redeemingResetCreditId != null;

  Future<void> _redeemResetCredit(
    ChatGptConnection connection,
    ChatGptWorkspace workspace,
    ChatGptResetCredit credit,
  ) async {
    final service = widget.serviceClient;
    if (service == null ||
        _isResetCreditActionPending ||
        !connection.isSelected ||
        workspace.id != _selectedWorkspaceId ||
        credit.status != 'available' ||
        _needsResetCreditRefresh(connection.id, workspace.id)) {
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
          context: ErrorDescription('while using a ChatGPT reset credit'),
        ),
      );
      refreshRequired = true;
      resultMessage = l10n.resetCreditOutcomeUnknown;
      resultType = OpenChatToastType.warning;
    }

    if (!mounted) return;
    if (refreshRequired) {
      setState(() {
        _resetCreditRefreshRequiredConnectionId = connection.id;
        _resetCreditRefreshRequiredWorkspaceId = workspace.id;
      });
    }
    _showMessage(resultMessage, type: resultType);
    await _loadUsage(connection.id, workspace.id);
    if (mounted) setState(() => _redeemingResetCreditId = null);
  }

  void _toggleApiKeyForm() {
    if (widget.apiKeyStore == null) {
      _showMessage(context.openchatL10n.apiKeyConnectionUnavailable);
      return;
    }

    setState(() {
      _showApiKeyForm = !_showApiKeyForm;
      _formError = null;
      if (!_showApiKeyForm) _apiKeyController.clear();
    });
  }

  Future<void> _saveApiKey() async {
    final store = widget.apiKeyStore;
    if (store == null || _isSaving || _apiKeyController.text.trim().isEmpty) {
      if (_apiKeyController.text.trim().isEmpty && mounted) {
        setState(() => _formError = context.openchatL10n.apiKeyRequired);
      }
      return;
    }

    setState(() {
      _isSaving = true;
      _formError = null;
    });

    try {
      final connections = await store.saveApiKey(_apiKeyController.text);
      if (!mounted) return;
      _loadRequestGeneration++;
      _apiKeyController.clear();
      setState(() {
        _connections = connections;
        _loadState = _ConnectionLoadState.loaded;
        _showApiKeyForm = false;
      });
      _showMessage(
        context.openchatL10n.apiKeySaved,
        type: OpenChatToastType.success,
      );
    } on EmptyChatGptApiKeyException {
      if (mounted) {
        setState(() => _formError = context.openchatL10n.apiKeyRequired);
      }
    } on InvalidChatGptApiKeyFormatException {
      if (mounted) {
        setState(() => _formError = context.openchatL10n.apiKeyInvalidFormat);
      }
    } on DuplicateChatGptApiKeyException {
      if (mounted) {
        setState(() => _formError = context.openchatL10n.apiKeyAlreadySaved);
      }
    } on ChatGptApiKeyStorageException {
      if (!mounted) return;
      _showMessage(context.openchatL10n.apiKeySaveFailed);
    } finally {
      if (mounted) setState(() => _isSaving = false);
    }
  }

  Future<bool> _confirmConnectionRemoval(String connectionName) async {
    final l10n = context.openchatL10n;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: Text(l10n.removeChatGptConnection),
        content: Text(l10n.confirmRemoveConnection(connectionName)),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: Text(l10n.cancel),
          ),
          FilledButton(
            style: FilledButton.styleFrom(
              backgroundColor: Theme.of(dialogContext).colorScheme.error,
            ),
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: Text(l10n.removeConnectionAction),
          ),
        ],
      ),
    );
    return confirmed == true;
  }

  Future<void> _removeApiKeyConnection(
    ChatGptApiKeyConnection connection,
  ) async {
    final store = widget.apiKeyStore;
    if (store == null || _removingConnectionIds.contains(connection.id)) return;
    final l10n = context.openchatL10n;
    final connectionName = switch (connection.keySuffix) {
      final String suffix => l10n.savedApiKeyWithSuffix(suffix),
      null => l10n.savedApiKey,
    };
    if (!await _confirmConnectionRemoval(connectionName) || !mounted) return;

    setState(() => _removingConnectionIds.add(connection.id));
    try {
      await store.deleteApiKey(connection.id);
      await _loadConnections();
      if (mounted) {
        _showMessage(
          context.openchatL10n.connectionRemoveSucceeded,
          type: OpenChatToastType.success,
        );
      }
    } on ChatGptApiKeyStorageException catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while removing a saved API key'),
        ),
      );
      if (mounted) _showMessage(context.openchatL10n.connectionRemoveFailed);
    } on InvalidChatGptApiKeyConnectionIdException catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while validating a saved API key'),
        ),
      );
      if (mounted) _showMessage(context.openchatL10n.connectionRemoveFailed);
    } finally {
      if (mounted) setState(() => _removingConnectionIds.remove(connection.id));
    }
  }

  Future<void> _removeOAuthConnection(ChatGptConnection connection) async {
    final service = widget.serviceClient;
    if (service == null ||
        _removingConnectionIds.contains(connection.id) ||
        _isResetCreditActionPending) {
      return;
    }
    final connectionName =
        connection.email ?? context.openchatL10n.accountEmailUnavailable;
    if (!await _confirmConnectionRemoval(connectionName) || !mounted) return;

    setState(() => _removingConnectionIds.add(connection.id));
    try {
      await service.call(
        'chatgpt.connections.delete',
        params: <String, Object?>{'connectionId': connection.id},
      );
      await widget.onConnectionRemoved?.call(connection.id);
      await _loadOAuthConnections();
      if (mounted) {
        _showMessage(
          context.openchatL10n.connectionRemoveSucceeded,
          type: OpenChatToastType.success,
        );
      }
    } on OpenChatServiceException catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while removing a ChatGPT OAuth account'),
        ),
      );
      if (mounted) _showMessage(context.openchatL10n.connectionRemoveFailed);
    } finally {
      if (mounted) setState(() => _removingConnectionIds.remove(connection.id));
    }
  }

  void _showMessage(
    String message, {
    OpenChatToastType type = OpenChatToastType.error,
  }) {
    showOpenChatToast(context, message, type: type);
  }

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final l10n = context.openchatL10n;
    final textTheme = Theme.of(context).textTheme;
    final logoColor = Theme.of(context).brightness == Brightness.dark
        ? Colors.white
        : Colors.black;
    final noConnections =
        _oauthLoadState == _ConnectionLoadState.loaded &&
        _oauthConnections.isEmpty &&
        _loadState == _ConnectionLoadState.loaded &&
        _connections.isEmpty;

    final actions = Wrap(
      spacing: 8,
      runSpacing: 8,
      children: [
        OutlinedButton.icon(
          onPressed: _isSaving ? null : _toggleApiKeyForm,
          icon: const Icon(Icons.key_outlined, size: 16),
          label: Text(l10n.apiKey),
        ),
        OutlinedButton.icon(
          onPressed: _isSigningIn ? null : () => unawaited(_startOAuth()),
          icon: const Icon(Icons.login_rounded, size: 16),
          label: Text(_isSigningIn ? l10n.oauthSigningIn : l10n.oauth),
        ),
      ],
    );

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Card(
          margin: EdgeInsets.zero,
          color: palette.surface,
          elevation: 0,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(10),
            side: BorderSide(color: palette.border),
          ),
          child: Padding(
            padding: const EdgeInsets.all(16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                LayoutBuilder(
                  builder: (context, constraints) {
                    final provider = Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        SvgPicture.asset(
                          'assets/icons/chatgpt.svg',
                          width: 24,
                          height: 24,
                          colorFilter: ColorFilter.mode(
                            logoColor,
                            BlendMode.srcIn,
                          ),
                          excludeFromSemantics: true,
                        ),
                        const SizedBox(width: 12),
                        Flexible(
                          child: Text(
                            'ChatGPT',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: textTheme.titleMedium,
                          ),
                        ),
                      ],
                    );

                    if (constraints.maxWidth < 520) {
                      return Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          provider,
                          const SizedBox(height: 14),
                          actions,
                        ],
                      );
                    }

                    return Row(
                      children: [
                        Expanded(child: provider),
                        const SizedBox(width: 16),
                        actions,
                      ],
                    );
                  },
                ),
                if (noConnections) ...[
                  const SizedBox(height: 12),
                  Text(
                    l10n.noChatGptConnections,
                    style: TextStyle(
                      color: palette.secondaryText,
                      fontSize: 13,
                    ),
                  ),
                ],
              ],
            ),
          ),
        ),
        if (_showApiKeyForm)
          ChatGptApiKeyForm(
            controller: _apiKeyController,
            errorText: _formError,
            isSaving: _isSaving,
            onChanged: () => setState(() => _formError = null),
            onSave: () => unawaited(_saveApiKey()),
            onCancel: _toggleApiKeyForm,
          ),
        if (_isSigningIn)
          Padding(
            padding: const EdgeInsets.only(top: 12),
            child: Row(
              children: [
                const SizedBox.square(
                  dimension: 16,
                  child: CircularProgressIndicator(strokeWidth: 2),
                ),
                const SizedBox(width: 10),
                Expanded(child: Text(l10n.oauthBrowserWaiting)),
                TextButton(onPressed: _cancelOAuth, child: Text(l10n.cancel)),
              ],
            ),
          ),
        if (_oauthError case final String error)
          Padding(
            padding: const EdgeInsets.only(top: 12),
            child: Row(
              children: [
                Expanded(
                  child: Text(
                    error,
                    style: TextStyle(
                      color: Theme.of(context).colorScheme.error,
                    ),
                  ),
                ),
                TextButton.icon(
                  onPressed: () => unawaited(_startOAuth()),
                  icon: const Icon(Icons.refresh_rounded, size: 16),
                  label: Text(l10n.retry),
                ),
              ],
            ),
          ),
        if (_oauthLoadState == _ConnectionLoadState.loading)
          const Padding(
            padding: EdgeInsets.fromLTRB(12, 18, 12, 0),
            child: LinearProgressIndicator(),
          )
        else if (_oauthLoadState == _ConnectionLoadState.failed)
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 14, 12, 0),
            child: Row(
              children: [
                Expanded(
                  child: Text(
                    l10n.oauthConnectionsLoadFailed,
                    style: TextStyle(
                      color: palette.secondaryText,
                      fontSize: 13,
                    ),
                  ),
                ),
                TextButton.icon(
                  onPressed: () => unawaited(_loadOAuthConnections()),
                  icon: const Icon(Icons.refresh_rounded, size: 16),
                  label: Text(l10n.retry),
                ),
              ],
            ),
          )
        else if (_oauthConnections.isNotEmpty)
          _buildOAuthConnections(palette),
        if (_loadState == _ConnectionLoadState.loading)
          const Padding(
            padding: EdgeInsets.fromLTRB(12, 18, 12, 0),
            child: LinearProgressIndicator(),
          )
        else if (_loadState == _ConnectionLoadState.failed)
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 14, 12, 0),
            child: Row(
              children: [
                Expanded(
                  child: Text(
                    l10n.apiKeyLoadFailed,
                    style: TextStyle(
                      color: palette.secondaryText,
                      fontSize: 13,
                    ),
                  ),
                ),
                TextButton.icon(
                  onPressed: () => unawaited(_loadConnections()),
                  icon: const Icon(Icons.refresh_rounded, size: 16),
                  label: Text(l10n.retry),
                ),
              ],
            ),
          )
        else if (_connections.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(top: 8),
            child: Column(
              children: [
                for (var index = 0; index < _connections.length; index++) ...[
                  _ChatGptConnectionThread(
                    key: ValueKey(_connections[index].id),
                    connection: _connections[index],
                    isRemoving: _removingConnectionIds.contains(
                      _connections[index].id,
                    ),
                    onRemove: () =>
                        unawaited(_removeApiKeyConnection(_connections[index])),
                  ),
                  if (index < _connections.length - 1)
                    SettingsDivider(color: palette.border),
                ],
              ],
            ),
          ),
      ],
    );
  }

  Widget _buildOAuthConnections(OpenChatPalette palette) {
    ChatGptConnection? selectedConnection;
    for (final connection in _oauthConnections) {
      if (connection.isSelected) {
        selectedConnection = connection;
        break;
      }
    }
    ChatGptWorkspace? selectedWorkspace;
    for (final workspace
        in selectedConnection?.workspaces ?? const <ChatGptWorkspace>[]) {
      if (workspace.isSelected) {
        selectedWorkspace = workspace;
        break;
      }
    }
    return Padding(
      padding: const EdgeInsets.only(top: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          for (var index = 0; index < _oauthConnections.length; index++) ...[
            _buildOAuthConnectionRow(_oauthConnections[index]),
            if (index < _oauthConnections.length - 1)
              SettingsDivider(color: palette.border),
          ],
          if (selectedConnection != null && selectedWorkspace != null)
            _buildUsageDetails(selectedConnection, selectedWorkspace, palette),
          if (_oauthConnections.isNotEmpty) ...[
            Padding(
              padding: const EdgeInsets.only(top: 12),
              child: SettingsDivider(color: palette.border),
            ),
            _buildTitlePreference(palette),
          ],
        ],
      ),
    );
  }

  Widget _buildTitlePreference(OpenChatPalette palette) {
    return ChatGptTitlePreferenceSection(
      preference: _titlePreference,
      connections: _oauthConnections,
      palette: palette,
      isLoading: _titlePreferenceLoadState == _ConnectionLoadState.loading,
      hasError: _titlePreferenceLoadState == _ConnectionLoadState.failed,
      isSaving: _isSavingTitlePreference,
      onRetry: () => unawaited(_loadTitlePreference()),
      onConnectionChanged: _selectTitleConnection,
      onWorkspaceChanged: _selectTitleWorkspace,
    );
  }

  Widget _buildOAuthConnectionRow(ChatGptConnection connection) {
    final isRemoving = _removingConnectionIds.contains(connection.id);
    return ChatGptOAuthConnectionRow(
      connection: connection,
      isRemoving: isRemoving,
      isSigningIn: _isSigningIn || _isResetCreditActionPending,
      selectedWorkspaceId: _selectedWorkspaceId,
      onUseConnection: () => unawaited(_selectOAuthConnection(connection)),
      onRemove: () => unawaited(_removeOAuthConnection(connection)),
      onWorkspaceChanged: (workspaceId) =>
          unawaited(_selectOAuthWorkspace(connection, workspaceId)),
    );
  }

  Widget _buildUsageDetails(
    ChatGptConnection connection,
    ChatGptWorkspace workspace,
    OpenChatPalette palette,
  ) {
    return ChatGptUsageDetails(
      connection: connection,
      workspace: workspace,
      palette: palette,
      isLoading: _usageLoadState == _UsageLoadState.loading,
      isLoaded: _usageLoadState == _UsageLoadState.loaded,
      hasError: _usageLoadState == _UsageLoadState.failed,
      snapshot: _usageSnapshot,
      confirmingResetCreditId: _confirmingResetCreditId,
      redeemingResetCreditId: _redeemingResetCreditId,
      needsResetCreditRefresh: _needsResetCreditRefresh(
        connection.id,
        workspace.id,
      ),
      onRefresh: () => unawaited(
        _loadUsage(connection.id, workspace.id, userInitiated: true),
      ),
      onRedeemResetCredit: (credit) =>
          unawaited(_redeemResetCredit(connection, workspace, credit)),
    );
  }
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

class _ChatGptConnectionThread extends StatelessWidget {
  const _ChatGptConnectionThread({
    required this.connection,
    required this.isRemoving,
    required this.onRemove,
    super.key,
  });

  final ChatGptApiKeyConnection connection;
  final bool isRemoving;
  final VoidCallback onRemove;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final l10n = context.openchatL10n;
    final keySuffix = connection.keySuffix;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 12),
      child: Row(
        children: [
          Icon(Icons.key_outlined, size: 18, color: palette.secondaryIcon),
          const SizedBox(width: 10),
          Expanded(
            child: Text(
              keySuffix == null
                  ? l10n.savedApiKey
                  : l10n.savedApiKeyWithSuffix(keySuffix),
              style: TextStyle(
                color: palette.text,
                fontSize: 14,
                fontWeight: FontWeight.w500,
              ),
            ),
          ),
          Text(
            l10n.apiKey,
            style: TextStyle(color: palette.secondaryText, fontSize: 12),
          ),
          const SizedBox(width: 4),
          IconButton(
            tooltip: l10n.removeChatGptConnection,
            onPressed: isRemoving ? null : onRemove,
            icon: isRemoving
                ? const SizedBox.square(
                    dimension: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.delete_outline_rounded, size: 18),
          ),
        ],
      ),
    );
  }
}
