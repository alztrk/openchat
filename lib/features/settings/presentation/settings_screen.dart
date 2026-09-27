import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_svg/flutter_svg.dart';

import '../../../app/zihora_theme.dart';
import '../../../app/zihora_toast.dart';
import '../../chat/domain/chatgpt_connection.dart';
import '../data/chat_gpt_api_key_store.dart';
import '../domain/chat_gpt_api_key_connection.dart';
import '../domain/chat_gpt_usage_snapshot.dart';
import '../../chat/domain/history_storage_status.dart';
import '../../../l10n/zihora_localizations.dart';
import '../../../platform/windows/zihora_service_client.dart';
import '../../../platform/windows/window_controls.dart';
import '../../chat/presentation/widgets/window_control_bar.dart';

class SettingsScreen extends StatefulWidget {
  const SettingsScreen({
    required this.themeMode,
    required this.onThemeModeChanged,
    required this.historyStorageStatus,
    required this.hasConversationHistory,
    this.onClearConversationHistory,
    this.chatGptApiKeyStore,
    this.serviceClient,
    this.onProviderStateChanged,
    this.onConnectionRemoved,
    super.key,
  });

  final ThemeMode themeMode;
  final Future<void> Function(ThemeMode) onThemeModeChanged;
  final HistoryStorageStatus historyStorageStatus;
  final bool hasConversationHistory;
  final Future<void> Function()? onClearConversationHistory;
  final ChatGptApiKeyStore? chatGptApiKeyStore;
  final ZihoraServiceClient? serviceClient;
  final Future<void> Function()? onProviderStateChanged;
  final Future<void> Function(String connectionId)? onConnectionRemoved;

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  bool _isClearingHistory = false;

  @override
  Widget build(BuildContext context) {
    final palette = ZihoraPalette.of(context);
    final l10n = context.zihoraL10n;
    final textTheme = Theme.of(context).textTheme;

    return LayoutBuilder(
      builder: (context, constraints) {
        final horizontalInset = constraints.maxWidth < 640 ? 16.0 : 24.0;
        final windowControlInset = constraints.maxWidth < 640 ? 16.0 : 32.0;

        return ColoredBox(
          color: palette.surface,
          child: Stack(
            children: [
              SingleChildScrollView(
                padding: EdgeInsets.fromLTRB(
                  horizontalInset,
                  52,
                  horizontalInset,
                  32,
                ),
                child: Center(
                  child: ConstrainedBox(
                    constraints: const BoxConstraints(maxWidth: 840),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        SizedBox(
                          height: 40,
                          child: Align(
                            alignment: Alignment.centerLeft,
                            child: Text(
                              l10n.settings,
                              style: textTheme.headlineSmall?.copyWith(
                                fontSize: 30,
                                height: 4 / 3,
                              ),
                            ),
                          ),
                        ),
                        const SizedBox(height: 6),
                        SizedBox(
                          height: 22,
                          child: Align(
                            alignment: Alignment.centerLeft,
                            child: Text(
                              l10n.settingsDescription,
                              style: TextStyle(
                                color: palette.secondaryText,
                                fontSize: 14,
                                fontWeight: FontWeight.w400,
                              ),
                            ),
                          ),
                        ),
                        const SizedBox(height: 20),
                        _SettingsDivider(color: palette.border),
                        const SizedBox(height: 33),
                        _SectionHeading(
                          label: l10n.connections,
                          style: TextStyle(
                            color: palette.text,
                            fontSize: 16,
                            fontWeight: FontWeight.w600,
                          ),
                        ),
                        const SizedBox(height: 20),
                        _ChatGptConnectionSection(
                          apiKeyStore: widget.chatGptApiKeyStore,
                          serviceClient: widget.serviceClient,
                          onProviderStateChanged: widget.onProviderStateChanged,
                          onConnectionRemoved: widget.onConnectionRemoved,
                        ),
                        const SizedBox(height: 32),
                        _SettingsDivider(color: palette.border),
                        const SizedBox(height: 33),
                        _SectionHeading(
                          label: l10n.appearance,
                          style: TextStyle(
                            color: palette.text,
                            fontSize: 16,
                            fontWeight: FontWeight.w600,
                          ),
                        ),
                        const SizedBox(height: 22),
                        _SettingsRow(
                          title: l10n.theme,
                          description: l10n.themeSettingDescription,
                          textTheme: textTheme,
                          controlWidth: 264,
                          desktopControlTopInset: 0,
                          controlBuilder: (width) => _ThemeSelector(
                            width: width,
                            mode: widget.themeMode,
                            onChanged: (mode) =>
                                unawaited(_changeThemeMode(mode)),
                            systemLabel: l10n.systemTheme,
                            lightLabel: l10n.lightTheme,
                            darkLabel: l10n.darkTheme,
                            palette: palette,
                          ),
                        ),
                        const SizedBox(height: 32),
                        _SettingsDivider(color: palette.border),
                        const SizedBox(height: 33),
                        _SectionHeading(
                          label: l10n.localData,
                          style: TextStyle(
                            color: palette.text,
                            fontSize: 16,
                            fontWeight: FontWeight.w600,
                          ),
                        ),
                        const SizedBox(height: 22),
                        _SettingsRow(
                          title: l10n.conversationHistory,
                          description: switch (widget.historyStorageStatus) {
                            HistoryStorageStatus.loading =>
                              l10n.historyCheckingDescription,
                            HistoryStorageStatus.available =>
                              l10n.historyDeviceDescription,
                            HistoryStorageStatus.unavailable =>
                              l10n.historyStorageUnavailableDescription,
                          },
                          textTheme: textTheme,
                          controlWidth: 144,
                          desktopControlTopInset: 4,
                          controlBuilder: (width) => _StatusLabel(
                            width: width,
                            label: switch (widget.historyStorageStatus) {
                              HistoryStorageStatus.loading =>
                                l10n.historyCheckingStatus,
                              HistoryStorageStatus.available =>
                                l10n.historyDeviceStatus,
                              HistoryStorageStatus.unavailable =>
                                l10n.historyStorageUnavailableStatus,
                            },
                            palette: palette,
                          ),
                        ),
                        const SizedBox(height: 22),
                        _SettingsRow(
                          title: l10n.clearConversationHistory,
                          description: l10n.clearConversationHistoryDescription,
                          textTheme: textTheme,
                          controlWidth: 176,
                          desktopControlTopInset: 0,
                          controlBuilder: (width) => SizedBox(
                            width: width,
                            height: 36,
                            child: OutlinedButton.icon(
                              onPressed:
                                  widget.hasConversationHistory &&
                                      widget.onClearConversationHistory !=
                                          null &&
                                      !_isClearingHistory
                                  ? () => unawaited(_confirmClearHistory())
                                  : null,
                              style: OutlinedButton.styleFrom(
                                foregroundColor: Theme.of(context)
                                    .colorScheme
                                    .error,
                                disabledForegroundColor: palette.secondaryText,
                                side: BorderSide(
                                  color: Theme.of(context).colorScheme.error,
                                ),
                                shape: RoundedRectangleBorder(
                                  borderRadius: BorderRadius.circular(8),
                                ),
                              ),
                              icon: _isClearingHistory
                                  ? const SizedBox.square(
                                      dimension: 16,
                                      child: CircularProgressIndicator(
                                        strokeWidth: 2,
                                      ),
                                    )
                                  : const Icon(
                                      Icons.delete_outline_rounded,
                                      size: 18,
                                    ),
                              label: Text(
                                _isClearingHistory
                                    ? l10n.clearingHistory
                                    : l10n.deleteAll,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                              ),
                            ),
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
              if (ZihoraWindowControls.isSupported)
                Positioned(
                  right: windowControlInset,
                  top: 18,
                  child: const WindowControlBar(),
                ),
            ],
          ),
        );
      },
    );
  }

  Future<void> _changeThemeMode(ThemeMode mode) async {
    try {
      await widget.onThemeModeChanged(mode);
    } on PlatformException catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while saving the theme preference'),
        ),
      );
      if (!mounted) return;
      showZihoraToast(
        context,
        context.zihoraL10n.themeSaveFailed,
        type: ZihoraToastType.error,
      );
    }
  }

  Future<void> _confirmClearHistory() async {
    final clearHistory = widget.onClearConversationHistory;
    if (clearHistory == null || !widget.hasConversationHistory) return;

    final l10n = context.zihoraL10n;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: Text(l10n.confirmClearHistoryTitle),
        content: Text(l10n.confirmClearHistoryBody),
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
            child: Text(l10n.deleteAll),
          ),
        ],
      ),
    );
    if (!mounted || confirmed != true) return;

    setState(() => _isClearingHistory = true);
    try {
      await clearHistory();
      if (!mounted) return;
      showZihoraToast(
        context,
        context.zihoraL10n.clearHistorySucceeded,
        type: ZihoraToastType.success,
      );
    } on Exception catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while clearing local chat history'),
        ),
      );
      if (!mounted) return;
      showZihoraToast(
        context,
        context.zihoraL10n.clearHistoryFailed,
        type: ZihoraToastType.error,
      );
    } finally {
      if (mounted) setState(() => _isClearingHistory = false);
    }
  }
}

enum _ConnectionLoadState { loading, loaded, failed }

enum _UsageLoadState { idle, loading, loaded, failed }

class _ChatGptConnectionSection extends StatefulWidget {
  const _ChatGptConnectionSection({
    required this.apiKeyStore,
    required this.serviceClient,
    required this.onProviderStateChanged,
    required this.onConnectionRemoved,
  });

  final ChatGptApiKeyStore? apiKeyStore;
  final ZihoraServiceClient? serviceClient;
  final Future<void> Function()? onProviderStateChanged;
  final Future<void> Function(String connectionId)? onConnectionRemoved;

  @override
  State<_ChatGptConnectionSection> createState() =>
      _ChatGptConnectionSectionState();
}

class _ChatGptConnectionSectionState extends State<_ChatGptConnectionSection> {
  final _apiKeyController = TextEditingController();
  List<ChatGptApiKeyConnection> _connections = const [];
  List<ChatGptConnection> _oauthConnections = const [];
  _ConnectionLoadState _loadState = _ConnectionLoadState.loading;
  _ConnectionLoadState _oauthLoadState = _ConnectionLoadState.loading;
  _ConnectionLoadState _titlePreferenceLoadState = _ConnectionLoadState.loading;
  _UsageLoadState _usageLoadState = _UsageLoadState.idle;
  ChatGptUsageSnapshot? _usageSnapshot;
  ChatGptTitlePreference _titlePreference = const ChatGptTitlePreference();
  ZihoraServiceOperation? _oauthOperation;
  String? _oauthError;
  String? _selectedWorkspaceId;
  String? _formError;
  bool _showApiKeyForm = false;
  bool _showApiKey = false;
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
    } on ZihoraServiceException {
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
    } on ZihoraServiceException {
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
    } on ZihoraServiceException {
      if (mounted) _showMessage(context.zihoraL10n.titlePreferenceSaveFailed);
    } on FormatException {
      if (mounted) _showMessage(context.zihoraL10n.chatGptDataUnavailable);
    } finally {
      if (mounted) setState(() => _isSavingTitlePreference = false);
    }
  }

  Future<void> _startOAuth() async {
    final service = widget.serviceClient;
    if (service == null || _isSigningIn) {
      if (service == null) {
        _showMessage(context.zihoraL10n.oauthConnectionUnavailable);
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
        showZihoraToast(
          context,
          cleanupWarning == true
              ? context.zihoraL10n.oldCredentialCleanupFailed
              : context.zihoraL10n.oauthConnectionAdded,
          type: cleanupWarning == true
              ? ZihoraToastType.warning
              : ZihoraToastType.success,
        );
      }
    } on ZihoraServiceException catch (error) {
      if (!mounted || error.code == 'oauth_cancelled') return;
      setState(
        () => _oauthError =
            '${context.zihoraL10n.oauthSignInFailed} (${error.code})',
      );
    } on FormatException {
      if (mounted) {
        setState(() => _oauthError = context.zihoraL10n.chatGptDataUnavailable);
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
    if (service == null || connection.isSelected) return;
    try {
      await service.call(
        'chatgpt.connections.select',
        params: <String, Object?>{'connectionId': connection.id},
      );
      await _loadOAuthConnections();
      await widget.onProviderStateChanged?.call();
    } on ZihoraServiceException {
      if (mounted) _showMessage(context.zihoraL10n.connectionSelectionFailed);
    }
  }

  Future<void> _selectOAuthWorkspace(
    ChatGptConnection connection,
    String workspaceId,
  ) async {
    final service = widget.serviceClient;
    if (service == null || workspaceId == _selectedWorkspaceId) return;
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
    } on ZihoraServiceException {
      if (mounted) _showMessage(context.zihoraL10n.workspaceSelectionFailed);
    }
  }

  Future<void> _loadUsage(String connectionId, String workspaceId) async {
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
      });
    } on ZihoraServiceException {
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

  void _toggleApiKeyForm() {
    if (widget.apiKeyStore == null) {
      _showMessage(context.zihoraL10n.apiKeyConnectionUnavailable);
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
        setState(() => _formError = context.zihoraL10n.apiKeyRequired);
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
        context.zihoraL10n.apiKeySaved,
        type: ZihoraToastType.success,
      );
    } on EmptyChatGptApiKeyException {
      if (mounted) {
        setState(() => _formError = context.zihoraL10n.apiKeyRequired);
      }
    } on InvalidChatGptApiKeyFormatException {
      if (mounted) {
        setState(() => _formError = context.zihoraL10n.apiKeyInvalidFormat);
      }
    } on DuplicateChatGptApiKeyException {
      if (mounted) {
        setState(() => _formError = context.zihoraL10n.apiKeyAlreadySaved);
      }
    } on ChatGptApiKeyStorageException {
      if (!mounted) return;
      _showMessage(context.zihoraL10n.apiKeySaveFailed);
    } finally {
      if (mounted) setState(() => _isSaving = false);
    }
  }

  Future<bool> _confirmConnectionRemoval(String connectionName) async {
    final l10n = context.zihoraL10n;
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
    final l10n = context.zihoraL10n;
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
          context.zihoraL10n.connectionRemoveSucceeded,
          type: ZihoraToastType.success,
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
      if (mounted) _showMessage(context.zihoraL10n.connectionRemoveFailed);
    } on InvalidChatGptApiKeyConnectionIdException catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while validating a saved API key'),
        ),
      );
      if (mounted) _showMessage(context.zihoraL10n.connectionRemoveFailed);
    } finally {
      if (mounted) setState(() => _removingConnectionIds.remove(connection.id));
    }
  }

  Future<void> _removeOAuthConnection(ChatGptConnection connection) async {
    final service = widget.serviceClient;
    if (service == null || _removingConnectionIds.contains(connection.id)) {
      return;
    }
    final connectionName =
        connection.email ?? context.zihoraL10n.accountEmailUnavailable;
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
          context.zihoraL10n.connectionRemoveSucceeded,
          type: ZihoraToastType.success,
        );
      }
    } on ZihoraServiceException catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while removing a ChatGPT OAuth account'),
        ),
      );
      if (mounted) _showMessage(context.zihoraL10n.connectionRemoveFailed);
    } finally {
      if (mounted) setState(() => _removingConnectionIds.remove(connection.id));
    }
  }

  void _showMessage(
    String message, {
    ZihoraToastType type = ZihoraToastType.error,
  }) {
    showZihoraToast(context, message, type: type);
  }

  @override
  Widget build(BuildContext context) {
    final palette = ZihoraPalette.of(context);
    final l10n = context.zihoraL10n;
    final textTheme = Theme.of(context).textTheme;
    final logoColor = Theme.of(context).brightness == Brightness.dark
        ? Colors.white
        : Colors.black;

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
            child: LayoutBuilder(
              builder: (context, constraints) {
                final provider = Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    SvgPicture.asset(
                      'assets/icons/chatgpt.svg',
                      width: 24,
                      height: 24,
                      colorFilter: ColorFilter.mode(logoColor, BlendMode.srcIn),
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
                    children: [provider, const SizedBox(height: 14), actions],
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
          ),
        ),
        if (_showApiKeyForm) _buildApiKeyForm(palette),
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
                    _SettingsDivider(color: palette.border),
                ],
              ],
            ),
          )
        else if (_oauthLoadState == _ConnectionLoadState.loaded &&
            _oauthConnections.isEmpty)
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 14, 12, 0),
            child: Text(
              l10n.noChatGptConnections,
              style: TextStyle(color: palette.secondaryText, fontSize: 13),
            ),
          ),
      ],
    );
  }

  Widget _buildOAuthConnections(ZihoraPalette palette) {
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
            _buildOAuthConnectionRow(_oauthConnections[index], palette),
            if (index < _oauthConnections.length - 1)
              _SettingsDivider(color: palette.border),
          ],
          if (selectedConnection != null && selectedWorkspace != null)
            _buildUsageDetails(selectedConnection, selectedWorkspace, palette),
          if (_oauthConnections.isNotEmpty) ...[
            Padding(
              padding: const EdgeInsets.only(top: 12),
              child: _SettingsDivider(color: palette.border),
            ),
            _buildTitlePreference(palette),
          ],
        ],
      ),
    );
  }

  Widget _buildTitlePreference(ZihoraPalette palette) {
    final l10n = context.zihoraL10n;
    final connectionId = _titlePreference.connectionId;
    ChatGptConnection? titleConnection;
    for (final connection in _oauthConnections) {
      if (connection.id == connectionId) {
        titleConnection = connection;
        break;
      }
    }
    final connectionItems = <DropdownMenuItem<String>>[
      DropdownMenuItem<String>(
        value: '',
        child: Text(l10n.titleUseConversationAccount),
      ),
      for (final connection in _oauthConnections)
        DropdownMenuItem<String>(
          value: connection.id,
          child: Text(
            connection.email ?? l10n.accountEmailUnavailable,
            overflow: TextOverflow.ellipsis,
          ),
        ),
      if (connectionId != null && titleConnection == null)
        DropdownMenuItem<String>(
          value: connectionId,
          enabled: false,
          child: Text(l10n.titleAccountUnavailable),
        ),
    ];
    final selectedConnectionValue =
        connectionId != null &&
            connectionItems.any((item) => item.value == connectionId)
        ? connectionId
        : '';
    final workspaces =
        titleConnection?.workspaces ?? const <ChatGptWorkspace>[];
    final selectedWorkspaceId = _titlePreference.workspaceId;
    final selectedWorkspaceIsAvailable = workspaces.any(
      (workspace) => workspace.id == selectedWorkspaceId,
    );

    return Padding(
      padding: const EdgeInsets.fromLTRB(12, 18, 12, 4),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(
            l10n.titleGenerationTarget,
            style: TextStyle(
              color: palette.text,
              fontSize: 13,
              fontWeight: FontWeight.w600,
            ),
          ),
          const SizedBox(height: 4),
          Text(
            l10n.titleGenerationTargetDescription,
            style: TextStyle(color: palette.secondaryText, fontSize: 12),
          ),
          if (_titlePreferenceLoadState == _ConnectionLoadState.loading)
            const Padding(
              padding: EdgeInsets.only(top: 10),
              child: LinearProgressIndicator(),
            )
          else if (_titlePreferenceLoadState == _ConnectionLoadState.failed)
            Padding(
              padding: const EdgeInsets.only(top: 4),
              child: Row(
                children: [
                  Expanded(
                    child: Text(
                      l10n.titlePreferenceLoadFailed,
                      style: TextStyle(
                        color: palette.secondaryText,
                        fontSize: 12,
                      ),
                    ),
                  ),
                  TextButton.icon(
                    onPressed: () => unawaited(_loadTitlePreference()),
                    icon: const Icon(Icons.refresh_rounded, size: 16),
                    label: Text(l10n.retry),
                  ),
                ],
              ),
            )
          else ...[
            const SizedBox(height: 8),
            DropdownButtonFormField<String>(
              initialValue: selectedConnectionValue,
              isExpanded: true,
              decoration: InputDecoration(
                labelText: l10n.titleGenerationTarget,
                contentPadding: const EdgeInsets.symmetric(
                  horizontal: 12,
                  vertical: 10,
                ),
                border: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(8),
                ),
              ),
              items: connectionItems,
              onChanged: _isSavingTitlePreference
                  ? null
                  : _selectTitleConnection,
            ),
            if (_isSavingTitlePreference)
              const Padding(
                padding: EdgeInsets.only(top: 8),
                child: LinearProgressIndicator(),
              ),
            if (connectionId != null && titleConnection != null) ...[
              if (workspaces.length > 1 ||
                  (workspaces.length == 1 && !selectedWorkspaceIsAvailable))
                Padding(
                  padding: const EdgeInsets.only(top: 8),
                  child: DropdownButtonFormField<String>(
                    initialValue: selectedWorkspaceIsAvailable
                        ? selectedWorkspaceId
                        : null,
                    isExpanded: true,
                    decoration: InputDecoration(
                      labelText: l10n.titleWorkspaceHint,
                      contentPadding: const EdgeInsets.symmetric(
                        horizontal: 12,
                        vertical: 10,
                      ),
                      border: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(8),
                      ),
                    ),
                    items: workspaces
                        .map(
                          (workspace) => DropdownMenuItem<String>(
                            value: workspace.id,
                            child: Text(
                              workspace.displayName ??
                                  l10n.workspaceWithoutName,
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                        )
                        .toList(growable: false),
                    onChanged: _isSavingTitlePreference
                        ? null
                        : _selectTitleWorkspace,
                  ),
                )
              else if (workspaces.isEmpty)
                Padding(
                  padding: const EdgeInsets.only(top: 8),
                  child: Text(
                    l10n.workspaceUnavailable,
                    style: TextStyle(
                      color: palette.secondaryText,
                      fontSize: 12,
                    ),
                  ),
                )
              else if (workspaces.length == 1)
                Padding(
                  padding: const EdgeInsets.only(top: 8),
                  child: Text(
                    workspaces.single.displayName ?? l10n.workspace,
                    style: TextStyle(
                      color: palette.secondaryText,
                      fontSize: 12,
                    ),
                  ),
                ),
            ],
          ],
        ],
      ),
    );
  }

  Widget _buildOAuthConnectionRow(
    ChatGptConnection connection,
    ZihoraPalette palette,
  ) {
    final l10n = context.zihoraL10n;
    final accountName = connection.email ?? l10n.accountEmailUnavailable;
    final isRemoving = _removingConnectionIds.contains(connection.id);
    final planType = connection.planType;
    ChatGptWorkspace? workspace;
    for (final item in connection.workspaces) {
      if (item.isSelected) {
        workspace = item;
        break;
      }
    }
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Icon(
                Icons.account_circle_outlined,
                size: 20,
                color: palette.secondaryIcon,
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      accountName,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: palette.text,
                        fontSize: 14,
                        fontWeight: FontWeight.w500,
                      ),
                    ),
                    const SizedBox(height: 3),
                    Text(
                      connection.authStatus == 'active'
                          ? l10n.accountPlan(planType ?? l10n.planUnavailable)
                          : l10n.connectionNeedsSignIn,
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: palette.secondaryText,
                        fontSize: 12,
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(width: 10),
              connection.isSelected
                  ? TextButton.icon(
                      onPressed: null,
                      icon: const Icon(Icons.check_circle_outline, size: 16),
                      label: Text(l10n.connectionSelected),
                    )
                  : OutlinedButton(
                      onPressed: _isSigningIn || isRemoving
                          ? null
                          : () => unawaited(_selectOAuthConnection(connection)),
                      child: Text(l10n.useConnection),
                    ),
              IconButton(
                tooltip: l10n.removeChatGptConnection,
                onPressed: isRemoving || _isSigningIn
                    ? null
                    : () => unawaited(_removeOAuthConnection(connection)),
                icon: isRemoving
                    ? const SizedBox.square(
                        dimension: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.delete_outline_rounded, size: 18),
              ),
            ],
          ),
          if (connection.isSelected && connection.workspaces.length > 1)
            Padding(
              padding: const EdgeInsets.only(left: 30, top: 10),
              child: DropdownButton<String>(
                value:
                    connection.workspaces.any(
                      (item) => item.id == _selectedWorkspaceId,
                    )
                    ? _selectedWorkspaceId
                    : null,
                isExpanded: true,
                hint: Text(l10n.selectWorkspace),
                items: connection.workspaces
                    .map(
                      (workspace) => DropdownMenuItem<String>(
                        value: workspace.id,
                        child: Text(
                          workspace.displayName ?? l10n.workspaceWithoutName,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                    )
                    .toList(growable: false),
                onChanged: _isSigningIn || isRemoving
                    ? null
                    : (workspaceId) {
                        if (workspaceId != null) {
                          unawaited(
                            _selectOAuthWorkspace(connection, workspaceId),
                          );
                        }
                      },
              ),
            )
          else if (connection.isSelected && workspace != null)
            Padding(
              padding: const EdgeInsets.only(left: 30, top: 8),
              child: Text(
                workspace.displayName ?? l10n.workspace,
                style: TextStyle(color: palette.secondaryText, fontSize: 12),
              ),
            )
          else if (connection.isSelected && connection.workspaces.isEmpty)
            Padding(
              padding: const EdgeInsets.only(left: 30, top: 8),
              child: Text(
                l10n.workspaceUnavailable,
                style: TextStyle(color: palette.secondaryText, fontSize: 12),
              ),
            )
          else if (!connection.isSelected && connection.workspaces.length > 1)
            Padding(
              padding: const EdgeInsets.only(left: 30, top: 8),
              child: Text(
                l10n.selectAccountForWorkspace,
                style: TextStyle(color: palette.secondaryText, fontSize: 12),
              ),
            ),
        ],
      ),
    );
  }

  Widget _buildUsageDetails(
    ChatGptConnection connection,
    ChatGptWorkspace workspace,
    ZihoraPalette palette,
  ) {
    final l10n = context.zihoraL10n;
    final allowed = _usageSnapshot?.ordinaryUsageAllowed;
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
                onPressed: _usageLoadState == _UsageLoadState.loading
                    ? null
                    : () => unawaited(_loadUsage(connection.id, workspace.id)),
                icon: const Icon(Icons.refresh_rounded, size: 18),
              ),
            ],
          ),
          if (_usageLoadState == _UsageLoadState.loading)
            const LinearProgressIndicator()
          else if (_usageLoadState == _UsageLoadState.failed)
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
                TextButton(
                  onPressed: () =>
                      unawaited(_loadUsage(connection.id, workspace.id)),
                  child: Text(l10n.retry),
                ),
              ],
            )
          else if (_usageLoadState == _UsageLoadState.loaded &&
              _usageSnapshot != null)
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
                            _usageSnapshot!.fetchedAtUnixMs,
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
                for (final bucket in _usageSnapshot!.buckets) ...[
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
                  _usageSnapshot!.resetCreditCount == null
                      ? l10n.resetCreditCountUnavailable
                      : l10n.resetCreditsAvailable(
                          _usageSnapshot!.resetCreditCount!,
                        ),
                  style: TextStyle(color: palette.secondaryText, fontSize: 12),
                ),
                if (_usageSnapshot!.resetCredits.isNotEmpty)
                  for (final credit in _usageSnapshot!.resetCredits)
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
                else if (_usageSnapshot!.resetCreditDetailsState == 'available')
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
                else if (_usageSnapshot!.resetCreditDetailsState != 'available')
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
                for (final credit in _usageSnapshot!.resetCredits)
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

  Widget _buildApiKeyForm(ZihoraPalette palette) {
    final l10n = context.zihoraL10n;
    return Padding(
      padding: const EdgeInsets.only(top: 12),
      child: Card(
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
              TextField(
                controller: _apiKeyController,
                autofocus: true,
                autocorrect: false,
                enableSuggestions: false,
                obscureText: !_showApiKey,
                keyboardType: TextInputType.visiblePassword,
                textInputAction: TextInputAction.done,
                onChanged: (_) {
                  setState(() => _formError = null);
                },
                onSubmitted: (_) => unawaited(_saveApiKey()),
                decoration: InputDecoration(
                  labelText: l10n.apiKeyInputLabel,
                  errorText: _formError,
                  suffixIcon: IconButton(
                    tooltip: _showApiKey ? l10n.hideApiKey : l10n.showApiKey,
                    onPressed: () => setState(() => _showApiKey = !_showApiKey),
                    icon: Icon(
                      _showApiKey
                          ? Icons.visibility_off_outlined
                          : Icons.visibility_outlined,
                    ),
                  ),
                ),
              ),
              const SizedBox(height: 12),
              Wrap(
                alignment: WrapAlignment.end,
                spacing: 8,
                runSpacing: 8,
                children: [
                  TextButton(
                    onPressed: _isSaving ? null : _toggleApiKeyForm,
                    child: Text(l10n.cancel),
                  ),
                  FilledButton.icon(
                    onPressed:
                        _isSaving || _apiKeyController.text.trim().isEmpty
                        ? null
                        : () => unawaited(_saveApiKey()),
                    icon: _isSaving
                        ? const SizedBox.square(
                            dimension: 16,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          )
                        : const Icon(Icons.save_outlined, size: 16),
                    label: Text(_isSaving ? l10n.saving : l10n.save),
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
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

String _localizedTimestamp(BuildContext context, int timestamp) {
  final dateTime = DateTime.fromMillisecondsSinceEpoch(timestamp).toLocal();
  final localizations = MaterialLocalizations.of(context);
  final date = localizations.formatMediumDate(dateTime);
  final time = localizations.formatTimeOfDay(TimeOfDay.fromDateTime(dateTime));
  return '$date, $time';
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
    final palette = ZihoraPalette.of(context);
    final l10n = context.zihoraL10n;
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

typedef _SettingsControlBuilder = Widget Function(double width);

class _SettingsRow extends StatelessWidget {
  const _SettingsRow({
    required this.title,
    required this.description,
    required this.textTheme,
    required this.controlWidth,
    required this.desktopControlTopInset,
    required this.controlBuilder,
  });

  final String title;
  final String description;
  final TextTheme textTheme;
  final double controlWidth;
  final double desktopControlTopInset;
  final _SettingsControlBuilder controlBuilder;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        if (constraints.maxWidth < 560) {
          final width = constraints.maxWidth < controlWidth
              ? constraints.maxWidth
              : controlWidth;
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              _SettingDescription(
                title: title,
                description: description,
                textTheme: textTheme,
              ),
              const SizedBox(height: 12),
              Align(
                alignment: Alignment.centerLeft,
                child: controlBuilder(width),
              ),
            ],
          );
        }

        return SizedBox(
          height: 46,
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                child: _SettingDescription(
                  title: title,
                  description: description,
                  textTheme: textTheme,
                ),
              ),
              Padding(
                padding: EdgeInsets.only(top: desktopControlTopInset),
                child: controlBuilder(controlWidth),
              ),
            ],
          ),
        );
      },
    );
  }
}

class _SectionHeading extends StatelessWidget {
  const _SectionHeading({required this.label, required this.style});

  final String label;
  final TextStyle? style;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 24,
      child: Align(
        alignment: Alignment.centerLeft,
        child: Text(label, style: style),
      ),
    );
  }
}

class _SettingDescription extends StatelessWidget {
  const _SettingDescription({
    required this.title,
    required this.description,
    required this.textTheme,
  });

  final String title;
  final String description;
  final TextTheme textTheme;

  @override
  Widget build(BuildContext context) {
    final palette = ZihoraPalette.of(context);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SizedBox(
          height: 22,
          child: Align(
            alignment: Alignment.centerLeft,
            child: Text(
              title,
              style: TextStyle(
                color: palette.text,
                fontSize: 15,
                fontWeight: FontWeight.w500,
              ),
            ),
          ),
        ),
        const SizedBox(height: 2),
        Text(
          description,
          style: TextStyle(
            color: palette.secondaryText,
            fontSize: 13,
            fontWeight: FontWeight.w400,
          ),
        ),
      ],
    );
  }
}

class _ThemeSelector extends StatelessWidget {
  const _ThemeSelector({
    required this.width,
    required this.mode,
    required this.onChanged,
    required this.systemLabel,
    required this.lightLabel,
    required this.darkLabel,
    required this.palette,
  });

  final double width;
  final ThemeMode mode;
  final ValueChanged<ThemeMode> onChanged;
  final String systemLabel;
  final String lightLabel;
  final String darkLabel;
  final ZihoraPalette palette;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: width,
      height: 40,
      padding: const EdgeInsets.all(3),
      decoration: BoxDecoration(
        color: palette.hover,
        border: Border.all(color: palette.controlBorder),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Row(
        children: [
          Expanded(
            child: _ThemeChoice(
              label: systemLabel,
              selected: mode == ThemeMode.system,
              selectedSemanticsLabel: systemLabel,
              palette: palette,
              onPressed: () => onChanged(ThemeMode.system),
            ),
          ),
          const SizedBox(width: 4),
          Expanded(
            child: _ThemeChoice(
              label: lightLabel,
              selected: mode == ThemeMode.light,
              selectedSemanticsLabel: lightLabel,
              palette: palette,
              onPressed: () => onChanged(ThemeMode.light),
            ),
          ),
          const SizedBox(width: 4),
          Expanded(
            child: _ThemeChoice(
              label: darkLabel,
              selected: mode == ThemeMode.dark,
              selectedSemanticsLabel: darkLabel,
              palette: palette,
              onPressed: () => onChanged(ThemeMode.dark),
            ),
          ),
        ],
      ),
    );
  }
}

class _ThemeChoice extends StatelessWidget {
  const _ThemeChoice({
    required this.label,
    required this.selected,
    required this.selectedSemanticsLabel,
    required this.palette,
    required this.onPressed,
  });

  final String label;
  final bool selected;
  final String selectedSemanticsLabel;
  final ZihoraPalette palette;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    return Semantics(
      button: true,
      selected: selected,
      label: selectedSemanticsLabel,
      child: ExcludeSemantics(
        child: Material(
          color: selected ? palette.selected : Colors.transparent,
          borderRadius: BorderRadius.circular(6),
          child: InkWell(
            onTap: onPressed,
            borderRadius: BorderRadius.circular(6),
            child: Center(
              child: Text(
                label,
                style: TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.w500,
                  color: selected ? palette.text : palette.secondaryText,
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _StatusLabel extends StatelessWidget {
  const _StatusLabel({
    required this.width,
    required this.label,
    required this.palette,
  });

  final double width;
  final String label;
  final ZihoraPalette palette;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: width,
      height: 30,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: palette.hover,
        borderRadius: BorderRadius.circular(8),
      ),
      child: Text(
        label,
        style: TextStyle(
          color: palette.accent,
          fontSize: 12,
          fontWeight: FontWeight.w500,
        ),
      ),
    );
  }
}

class _SettingsDivider extends StatelessWidget {
  const _SettingsDivider({required this.color});

  final Color color;

  @override
  Widget build(BuildContext context) {
    return SizedBox(height: 1, child: ColoredBox(color: color));
  }
}
