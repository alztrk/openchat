import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:file_picker/file_picker.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../../../app/openchat_theme.dart';
import '../../../app/openchat_toast.dart';
import '../../../l10n/openchat_localizations.dart';
import '../../../platform/windows/openchat_service_client.dart';
import '../../settings/data/chat_gpt_api_key_store.dart';
import '../../settings/data/open_code_api_key_store.dart';
import '../../settings/data/settings_preferences.dart';
import '../../settings/presentation/settings_screen.dart';
import '../data/chat_repository.dart';
import '../domain/chat_conversation.dart';
import 'conversation_markdown_export.dart';
import '../domain/chat_message.dart' as chat;
import '../domain/chatgpt_connection.dart';
import '../domain/chat_project.dart';
import '../domain/conversation_sidebar_data.dart';
import '../domain/history_storage_status.dart';
import '../domain/model_favorite.dart';
import 'widgets/chat_navigation_rail.dart';
import 'widgets/conversation_pane.dart';
import 'widgets/conversation_sidebar.dart';
import 'widgets/create_project_dialog.dart';
import 'widgets/tool_permission_dialog.dart';

class ChatScreen extends StatefulWidget {
  const ChatScreen({
    required this.themeMode,
    required this.onThemeModeChanged,
    required this.onToggleTheme,
    required this.historyStorageStatus,
    this.settingsPreferences,
    this.locale,
    this.onLocaleChanged,
    this.chatRepository,
    this.chatGptApiKeyStore,
    this.openCodeApiKeyStore,
    this.serviceClient,
    this.selectedModelLabel,
    super.key,
  });

  final ThemeMode themeMode;
  final Locale? locale;
  final Future<void> Function(ThemeMode) onThemeModeChanged;
  final Future<void> Function(Locale?)? onLocaleChanged;
  final Future<void> Function() onToggleTheme;
  final ChatRepository? chatRepository;
  final ChatGptApiKeyStore? chatGptApiKeyStore;
  final OpenCodeApiKeyStore? openCodeApiKeyStore;
  final OpenChatServiceClient? serviceClient;
  final HistoryStorageStatus historyStorageStatus;
  final SettingsPreferences? settingsPreferences;
  final String? selectedModelLabel;

  @override
  State<ChatScreen> createState() => _ChatScreenState();
}

class _ChatScreenState extends State<ChatScreen> {
  late final SettingsPreferences _settingsPreferences =
      widget.settingsPreferences ??
      SettingsPreferences(SharedPreferencesAsync());
  final _scaffoldKey = GlobalKey<ScaffoldState>();
  final _searchController = TextEditingController();
  final _messageController = TextEditingController();
  final _messageScrollController = ScrollController();
  bool _settingsOpen = false;
  bool _sidebarsCollapsed = false;
  bool _isSending = false;
  bool _isLoadingToolPermissionMode = true;
  bool _isSavingToolPermissionMode = false;
  bool _toolPermissionModeReady = false;
  ToolPermissionMode _toolPermissionMode = ToolPermissionMode.requireApproval;
  String? _activeChatConversationId;
  String? _replacingAssistantMessageId;
  bool _isLoadingConnections = false;
  bool _isLoadingModels = false;
  bool _isUpdatingConversationModel = false;
  String? _selectedConversationId;
  String _selectedProviderId = 'chatgpt';
  String? _titleEditRequestId;
  String? _selectedConnectionId;
  String? _selectedWorkspaceId;
  String? _selectedModelId;
  String? _selectedReasoningEffort;
  String? _modelLoadError;
  String _modelFreshness = 'unavailable';
  int _modelLoadGeneration = 0;
  int _modelSelectionGeneration = 0;
  int _nextLocalId = 0;
  List<ChatGptModel> _models = const <ChatGptModel>[];
  bool _modelsLoaded = false;
  String? _loadedConnectionId;
  String? _loadedWorkspaceId;
  String? _loadedProviderId;
  OpenChatServiceOperation? _activeChatOperation;
  Stream<List<ChatConversation>>? _conversationStream;
  Stream<List<ChatProject>>? _projectStream;
  Stream<List<chat.ChatMessage>>? _messageStream;
  Stream<List<FavoriteModel>>? _favoriteModelsStream;

  @override
  void initState() {
    super.initState();
    _bindRepositoryStreams();
    unawaited(_loadToolPermissionMode());
    if (widget.historyStorageStatus == HistoryStorageStatus.available) {
      unawaited(_loadProviderState());
    }
  }

  @override
  void didUpdateWidget(covariant ChatScreen oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.chatRepository != widget.chatRepository ||
        oldWidget.historyStorageStatus != widget.historyStorageStatus) {
      _bindRepositoryStreams();
      if (widget.historyStorageStatus == HistoryStorageStatus.available) {
        unawaited(_loadProviderState());
      }
    }
    if (oldWidget.serviceClient != widget.serviceClient &&
        widget.historyStorageStatus == HistoryStorageStatus.available) {
      unawaited(_loadProviderState());
    }
  }

  void _bindRepositoryStreams() {
    final repository = widget.chatRepository;
    if (repository == null ||
        widget.historyStorageStatus != HistoryStorageStatus.available) {
      _conversationStream = null;
      _projectStream = null;
      _messageStream = null;
      _favoriteModelsStream = null;
      return;
    }
    _conversationStream = repository.watchConversations();
    _projectStream = repository.watchProjects();
    _favoriteModelsStream = repository.watchModelFavorites();
    final selectedId = _selectedConversationId;
    _messageStream = selectedId == null
        ? null
        : repository.watchMessages(selectedId);
  }

  void _selectConversation(String conversationId) {
    _modelSelectionGeneration++;
    _resetMessageScroll();
    setState(() {
      _selectedConversationId = conversationId;
      _selectedProviderId = 'chatgpt';
      _messageStream = widget.chatRepository?.watchMessages(conversationId);
      _selectedModelId = null;
      _selectedReasoningEffort = null;
      _isUpdatingConversationModel = false;
      _titleEditRequestId = null;
    });
    unawaited(_loadConversationModels(conversationId));
  }

  Future<void> _loadProviderState({bool forceRefresh = false}) async {
    final service = widget.serviceClient;
    if (service == null || _isLoadingConnections) return;
    setState(() => _isLoadingConnections = true);
    try {
      final response = await service.call('chatgpt.connections.list');
      final rawConnections = response['connections'];
      if (rawConnections is! List<Object?>) {
        throw const FormatException('The ChatGPT connection list was invalid.');
      }
      final connections = rawConnections
          .map((value) => ChatGptConnection.fromJson(_objectMap(value)))
          .toList(growable: false);
      ChatGptConnection? selectedConnection;
      for (final connection in connections) {
        if (connection.isSelected) {
          selectedConnection = connection;
          break;
        }
      }
      ChatGptWorkspace? selectedWorkspace;
      if (selectedConnection != null) {
        for (final workspace in selectedConnection.workspaces) {
          if (workspace.isSelected) {
            selectedWorkspace = workspace;
            break;
          }
        }
      }
      if (!mounted) return;
      setState(() {
        _selectedConnectionId = selectedConnection?.id;
        _selectedWorkspaceId = selectedWorkspace?.id;
      });
      final currentId = _selectedConversationId;
      if (currentId != null) {
        await _loadConversationModels(currentId, forceRefresh: forceRefresh);
      } else if (selectedConnection != null && selectedWorkspace != null) {
        await _loadModels(
          selectedConnection.id,
          selectedWorkspace.id,
          selectedModelId: null,
          forceRefresh: forceRefresh,
        );
      } else {
        _clearModels();
      }
    } on OpenChatServiceException catch (error) {
      _showServiceFailure(error);
    } on FormatException {
      if (mounted) _showMessage(context.openchatL10n.providerDataUnavailable);
    } finally {
      if (mounted) setState(() => _isLoadingConnections = false);
    }
  }

  Future<void> _loadConversationModels(
    String conversationId, {
    bool forceRefresh = false,
  }) async {
    final repository = widget.chatRepository;
    if (repository == null) return;
    try {
      final conversation = await repository.getConversation(conversationId);
      if (!mounted || _selectedConversationId != conversationId) return;
      if (conversation?.providerId == 'opencode') {
        setState(() => _selectedProviderId = 'opencode');
        await _loadModels(
          '',
          '',
          selectedModelId: conversation?.modelId,
          forceRefresh: forceRefresh,
        );
      } else if (conversation?.connectionId case final String connectionId) {
        setState(() => _selectedProviderId = 'chatgpt');
        final workspaceId = conversation?.workspaceId;
        final modelId = conversation?.modelId;
        if (workspaceId == null || modelId == null) {
          throw StateError(
            'The conversation provider selection was incomplete.',
          );
        }
        if (!forceRefresh &&
            _modelsLoaded &&
            _loadedProviderId == 'chatgpt' &&
            _loadedConnectionId == connectionId &&
            _loadedWorkspaceId == workspaceId) {
          setState(() {
            _selectedModelId = modelId;
            _selectedReasoningEffort = null;
          });
        } else {
          await _loadModels(
            connectionId,
            workspaceId,
            selectedModelId: modelId,
            forceRefresh: forceRefresh,
          );
        }
      } else if (_selectedConnectionId != null &&
          _selectedWorkspaceId != null) {
        await _loadModels(
          _selectedConnectionId!,
          _selectedWorkspaceId!,
          selectedModelId: conversation?.modelId,
          forceRefresh: forceRefresh,
        );
      } else {
        _clearModels();
      }
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'chat_provider',
          context: ErrorDescription(
            'while loading the conversation model route',
          ),
        ),
      );
      if (mounted) _showMessage(context.openchatL10n.providerDataUnavailable);
    }
  }

  Future<void> _loadModels(
    String connectionId,
    String workspaceId, {
    required String? selectedModelId,
    bool forceRefresh = false,
  }) async {
    final service = widget.serviceClient;
    if (service == null) return;
    final sameRoute =
        _loadedProviderId == _selectedProviderId &&
        (_selectedProviderId == 'opencode' ||
            (_loadedConnectionId == connectionId &&
                _loadedWorkspaceId == workspaceId));
    if (!forceRefresh && sameRoute && _modelsLoaded) {
      ChatGptModel? selectedModel;
      for (final model in _models) {
        if (model.id == selectedModelId && model.isAvailable) {
          selectedModel = model;
          break;
        }
      }
      setState(() {
        _selectedModelId = selectedModel?.id;
        _selectedReasoningEffort = selectedModel?.defaultReasoningLevel;
      });
      return;
    }
    final generation = ++_modelLoadGeneration;
    setState(() {
      _isLoadingModels = true;
      _modelLoadError = null;
      _modelFreshness = 'unavailable';
      if (!sameRoute) {
        _models = const <ChatGptModel>[];
        _modelsLoaded = false;
      }
      _selectedModelId = selectedModelId;
      _selectedReasoningEffort = null;
      _loadedConnectionId = _selectedProviderId == 'chatgpt'
          ? connectionId
          : null;
      _loadedWorkspaceId = _selectedProviderId == 'chatgpt'
          ? workspaceId
          : null;
      _loadedProviderId = _selectedProviderId;
    });
    try {
      final openCodeApiKey = _selectedProviderId == 'opencode'
          ? await widget.openCodeApiKeyStore?.readApiKey()
          : null;
      final response = await service.call(
        _selectedProviderId == 'opencode'
            ? 'opencode.models.list'
            : 'chatgpt.models.list',
        params: _selectedProviderId == 'opencode'
            ? <String, Object?>{
                ...?switch (openCodeApiKey) {
                  final apiKey? => <String, Object?>{'apiKey': apiKey},
                  _ => null,
                },
                if (forceRefresh) 'forceRefresh': true,
              }
            : <String, Object?>{
                'connectionId': connectionId,
                'workspaceId': workspaceId,
                if (forceRefresh) 'forceRefresh': true,
              },
      );
      final rawModels = response['models'];
      if (rawModels is! List<Object?>) {
        throw const FormatException('The provider model list was invalid.');
      }
      final models = rawModels
          .map((value) => ChatGptModel.fromJson(_objectMap(value)))
          .toList(growable: false);
      final freshness = response['freshness'];
      if (freshness is! String ||
          (freshness != 'current' && freshness != 'stale')) {
        throw const FormatException('The ChatGPT model freshness was invalid.');
      }
      if (!mounted || generation != _modelLoadGeneration) return;
      setState(() {
        _models = models;
        _modelsLoaded = true;
        _modelFreshness = freshness;
        _selectedModelId =
            models.any(
              (model) => model.id == selectedModelId && model.isAvailable,
            )
            ? selectedModelId
            : null;
        _modelLoadError = models.any((model) => model.isAvailable)
            ? null
            : 'empty';
      });
    } on OpenCodeApiKeyStorageException {
      if (!mounted || generation != _modelLoadGeneration) return;
      setState(() {
        _modelLoadError = 'key_storage';
        _models = const <ChatGptModel>[];
        _modelsLoaded = false;
      });
      _showMessage(context.openchatL10n.openCodeKeyStorageFailed);
    } on OpenChatServiceException catch (error) {
      if (!mounted || generation != _modelLoadGeneration) return;
      setState(() {
        _modelLoadError = error.code;
        _models = const <ChatGptModel>[];
        _modelsLoaded = false;
      });
      _showServiceFailure(error);
    } on FormatException {
      if (!mounted || generation != _modelLoadGeneration) return;
      setState(() {
        _modelLoadError = 'invalid_response';
        _models = const <ChatGptModel>[];
        _modelsLoaded = false;
      });
      if (mounted) _showMessage(context.openchatL10n.providerDataUnavailable);
    } finally {
      if (mounted && generation == _modelLoadGeneration) {
        setState(() => _isLoadingModels = false);
      }
    }
  }

  void _clearModels() {
    _modelLoadGeneration++;
    if (!mounted) return;
    setState(() {
      _models = const <ChatGptModel>[];
      _modelsLoaded = false;
      _selectedModelId = null;
      _selectedReasoningEffort = null;
      _loadedConnectionId = null;
      _loadedWorkspaceId = null;
      _loadedProviderId = null;
      _modelLoadError = null;
      _modelFreshness = 'unavailable';
      _isLoadingModels = false;
    });
  }

  void _selectModel(String modelId) {
    ChatGptModel? model;
    for (final candidate in _models) {
      if (candidate.id == modelId && candidate.isAvailable) {
        model = candidate;
        break;
      }
    }
    final selectedModel = model;
    if (selectedModel == null) return;
    final conversationId = _selectedConversationId;
    final generation = ++_modelSelectionGeneration;
    final previousModelId = _selectedModelId;
    final previousReasoningEffort = _selectedReasoningEffort;
    setState(() {
      _selectedModelId = selectedModel.id;
      _selectedReasoningEffort = selectedModel.defaultReasoningLevel;
      _isUpdatingConversationModel = conversationId != null;
    });
    if (conversationId != null) {
      unawaited(
        _persistConversationModel(
          conversationId: conversationId,
          modelId: selectedModel.id,
          previousModelId: previousModelId,
          previousReasoningEffort: previousReasoningEffort,
          generation: generation,
        ),
      );
    }
  }

  Future<void> _setModelFavorite({
    required String providerId,
    required String modelId,
    required String displayName,
    required bool isFavorite,
  }) async {
    final repository = widget.chatRepository;
    if (repository == null) return;
    try {
      await repository.setModelFavorite(
        providerId: providerId,
        modelId: modelId,
        displayName: displayName,
        isFavorite: isFavorite,
      );
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'model_favorites',
          context: ErrorDescription('while saving a model favorite'),
        ),
      );
      if (mounted) _showMessage(context.openchatL10n.chatHistoryUnavailable);
    }
  }

  Future<void> _selectFavoriteModel(FavoriteModel favorite) async {
    final targetConversationId = _selectedConversationId;
    var connectionId = _selectedConnectionId;
    var workspaceId = _selectedWorkspaceId;

    if (targetConversationId != null) {
      final conversation = await widget.chatRepository?.getConversation(
        targetConversationId,
      );
      if (!mounted || _selectedConversationId != targetConversationId) return;
      final conversationProviderId =
          conversation?.providerId ??
          (conversation?.connectionId == null ? null : 'chatgpt');
      if (conversation == null ||
          conversationProviderId != favorite.providerId) {
        return;
      }
      connectionId = conversation.connectionId;
      workspaceId = conversation.workspaceId;
    }

    if (favorite.providerId == 'chatgpt' &&
        (connectionId == null || workspaceId == null)) {
      _showMessage(context.openchatL10n.modelCatalogUnavailable);
      return;
    }
    if (favorite.providerId != 'chatgpt' && favorite.providerId != 'opencode') {
      return;
    }

    if (_selectedProviderId != favorite.providerId) {
      setState(() {
        _selectedProviderId = favorite.providerId;
        _selectedModelId = null;
        _selectedReasoningEffort = null;
      });
    }
    await _loadModels(
      connectionId ?? '',
      workspaceId ?? '',
      selectedModelId: null,
    );
    if (!mounted ||
        _selectedConversationId != targetConversationId ||
        _selectedProviderId != favorite.providerId ||
        _modelLoadError != null) {
      return;
    }
    if (!_models.any(
      (model) => model.id == favorite.modelId && model.isAvailable,
    )) {
      _showMessage(context.openchatL10n.modelCatalogUnavailable);
      return;
    }
    _selectModel(favorite.modelId);
  }

  void _selectProvider(String providerId) {
    if (providerId == _selectedProviderId ||
        (providerId != 'chatgpt' && providerId != 'opencode')) {
      return;
    }
    setState(() {
      _selectedProviderId = providerId;
      _selectedModelId = null;
      _selectedReasoningEffort = null;
    });
    if (providerId == 'opencode') {
      unawaited(_loadModels('', '', selectedModelId: null));
    } else {
      final connectionId = _selectedConnectionId;
      final workspaceId = _selectedWorkspaceId;
      if (connectionId != null && workspaceId != null) {
        unawaited(
          _loadModels(connectionId, workspaceId, selectedModelId: null),
        );
      } else {
        _clearModels();
      }
    }
  }

  Future<void> _persistConversationModel({
    required String conversationId,
    required String modelId,
    required String? previousModelId,
    required String? previousReasoningEffort,
    required int generation,
  }) async {
    final repository = widget.chatRepository;
    try {
      if (repository == null) {
        throw StateError('Chat history storage is unavailable.');
      }
      final conversation = await repository.getConversation(conversationId);
      if (conversation == null) {
        throw StateError('The conversation no longer exists.');
      }
      if (conversation.providerId == 'opencode') {
        await repository.setConversationModel(
          conversationId: conversationId,
          modelId: modelId,
        );
      } else if (conversation.connectionId == null) {
        final connectionId = _loadedConnectionId;
        final workspaceId = _loadedWorkspaceId;
        if (connectionId == null || workspaceId == null) {
          throw StateError('The conversation model route is unavailable.');
        }
        await repository.bindConversationProvider(
          conversationId: conversationId,
          connectionId: connectionId,
          workspaceId: workspaceId,
          modelId: modelId,
        );
      } else {
        await repository.setConversationModel(
          conversationId: conversationId,
          modelId: modelId,
        );
      }
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'chat_history',
          context: ErrorDescription('while saving the conversation model'),
        ),
      );
      if (mounted &&
          generation == _modelSelectionGeneration &&
          _selectedConversationId == conversationId) {
        setState(() {
          _selectedModelId = previousModelId;
          _selectedReasoningEffort = previousReasoningEffort;
          _isUpdatingConversationModel = false;
        });
        showOpenChatToast(
          context,
          context.openchatL10n.conversationModelSaveFailed,
          type: OpenChatToastType.error,
        );
      }
      return;
    }

    if (mounted &&
        generation == _modelSelectionGeneration &&
        _selectedConversationId == conversationId) {
      setState(() => _isUpdatingConversationModel = false);
    }
  }

  void _selectReasoning(String effort) {
    setState(() => _selectedReasoningEffort = effort);
  }

  void _resetMessageScroll() {
    if (_messageScrollController.hasClients) {
      _messageScrollController.jumpTo(0);
    }
  }

  void _startNewConversation() {
    _modelSelectionGeneration++;
    _resetMessageScroll();
    setState(() {
      _selectedConversationId = null;
      _selectedProviderId = 'chatgpt';
      _messageStream = null;
      _selectedModelId = null;
      _selectedReasoningEffort = null;
      _isUpdatingConversationModel = false;
      _titleEditRequestId = null;
      _settingsOpen = false;
    });
    final connectionId = _selectedConnectionId;
    final workspaceId = _selectedWorkspaceId;
    if (connectionId != null && workspaceId != null) {
      unawaited(_loadModels(connectionId, workspaceId, selectedModelId: null));
    }
  }

  void _handleThemeToggle() {
    unawaited(_toggleThemeWithFeedback());
  }

  Future<void> _toggleThemeWithFeedback() async {
    try {
      await widget.onToggleTheme();
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
      _showMessage(context.openchatL10n.themeSaveFailed);
    }
  }

  Future<void> _clearConversationHistory() async {
    final repository = widget.chatRepository;
    if (repository == null) {
      throw StateError('Chat history storage is unavailable.');
    }
    await repository.deleteAllConversations();
    if (!mounted) return;
    _modelSelectionGeneration++;
    _resetMessageScroll();
    _messageController.clear();
    setState(() {
      _selectedConversationId = null;
      _messageStream = null;
      _isUpdatingConversationModel = false;
      _titleEditRequestId = null;
    });
  }

  Future<void> _sendMessage(
    ChatConversation? selectedConversation, {
    chat.ChatMessage? responseToReplace,
  }) async {
    if (_isSending ||
        _isUpdatingConversationModel ||
        !_toolPermissionModeReady ||
        _isLoadingToolPermissionMode ||
        _isSavingToolPermissionMode) {
      return;
    }
    final toolPermissionMode = _toolPermissionMode;
    final repository = widget.chatRepository;
    final service = widget.serviceClient;
    final l10n = context.openchatL10n;
    final text = responseToReplace == null
        ? _messageController.text.trim()
        : '';
    if (repository == null ||
        service == null ||
        (responseToReplace == null && text.isEmpty)) {
      return;
    }

    final providerId = selectedConversation?.providerId ?? _selectedProviderId;
    final connectionId = providerId == 'opencode'
        ? null
        : selectedConversation?.connectionId ?? _selectedConnectionId;
    final workspaceId = providerId == 'opencode'
        ? null
        : selectedConversation?.workspaceId ?? _selectedWorkspaceId;
    final modelId = _selectedModelId ?? selectedConversation?.modelId;
    if (modelId == null ||
        (providerId == 'chatgpt' &&
            (connectionId == null || workspaceId == null))) {
      _showMessage(context.openchatL10n.modelRequired);
      return;
    }
    if (_loadedProviderId != providerId ||
        (providerId == 'chatgpt' &&
            (_loadedConnectionId != connectionId ||
                _loadedWorkspaceId != workspaceId)) ||
        !_models.any((model) => model.id == modelId && model.isAvailable)) {
      _showMessage(l10n.modelCatalogUnavailable);
      return;
    }

    final conversationId = selectedConversation?.id ?? _newLocalId();
    if (mounted) {
      setState(() {
        _isSending = true;
        _activeChatConversationId = conversationId;
        _replacingAssistantMessageId = responseToReplace?.id;
      });
    }

    late final String sharedInstructions;
    try {
      sharedInstructions = await _settingsPreferences.readSharedInstructions();
    } on PlatformException catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while loading instructions for a chat'),
        ),
      );
      if (mounted) {
        _showMessage(l10n.sharedInstructionsLoadFailed);
        setState(() {
          _isSending = false;
          _activeChatConversationId = null;
          _replacingAssistantMessageId = null;
        });
      }
      return;
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while loading instructions for a chat'),
        ),
      );
      if (mounted) {
        _showMessage(l10n.sharedInstructionsLoadFailed);
        setState(() {
          _isSending = false;
          _activeChatConversationId = null;
          _replacingAssistantMessageId = null;
        });
      }
      return;
    }

    if (responseToReplace != null) {
      if (selectedConversation == null) {
        if (mounted) {
          setState(() {
            _isSending = false;
            _activeChatConversationId = null;
            _replacingAssistantMessageId = null;
          });
        }
        return;
      }
      try {
        final messages = await repository.getMessages(conversationId);
        if (!mounted) return;
        if (_selectedConversationId != conversationId) {
          setState(() {
            _isSending = false;
            _activeChatConversationId = null;
            _replacingAssistantMessageId = null;
          });
          return;
        }
        if (messages.length < 2 ||
            messages.last.id != responseToReplace.id ||
            messages.last.role != chat.ChatMessageRole.assistant ||
            messages.last.status == chat.ChatMessageStatus.streaming ||
            messages[messages.length - 2].role != chat.ChatMessageRole.user) {
          _showMessage(l10n.responseRetryUnavailable);
          setState(() {
            _isSending = false;
            _activeChatConversationId = null;
            _replacingAssistantMessageId = null;
          });
          return;
        }
      } on Object catch (error, stackTrace) {
        FlutterError.reportError(
          FlutterErrorDetails(
            exception: error,
            stack: stackTrace,
            library: 'chat_history',
            context: ErrorDescription('while preparing a response retry'),
          ),
        );
        if (mounted) {
          _showMessage(l10n.responseRetryUnavailable);
          setState(() {
            _isSending = false;
            _activeChatConversationId = null;
            _replacingAssistantMessageId = null;
          });
        }
        return;
      }
    }

    try {
      if (responseToReplace == null && selectedConversation == null) {
        await repository.createConversation(
          id: conversationId,
          title: l10n.conversationTitle,
          createdAt: DateTime.now().toUtc(),
          providerId: providerId == 'opencode' ? providerId : null,
          connectionId: connectionId,
          workspaceId: workspaceId,
          modelId: modelId,
        );
        if (mounted) {
          setState(() {
            _selectedConversationId = conversationId;
            _messageStream = repository.watchMessages(conversationId);
          });
        }
      } else if (responseToReplace == null && providerId == 'chatgpt') {
        final currentConversation = selectedConversation;
        if (currentConversation == null) {
          throw StateError('The conversation route is unavailable.');
        }
        if (currentConversation.connectionId == null) {
          final selectedConnectionId = connectionId;
          final selectedWorkspaceId = workspaceId;
          if (selectedConnectionId == null || selectedWorkspaceId == null) {
            throw StateError('The ChatGPT connection route is unavailable.');
          }
          await repository.bindConversationProvider(
            conversationId: conversationId,
            connectionId: selectedConnectionId,
            workspaceId: selectedWorkspaceId,
            modelId: modelId,
          );
        }
      }
      if (responseToReplace == null) {
        await repository.saveMessage(
          conversationId: conversationId,
          message: chat.ChatMessage(
            id: _newLocalId(),
            role: chat.ChatMessageRole.user,
            content: text,
            createdAt: DateTime.now().toUtc(),
          ),
        );
      }
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'chat_history',
          context: ErrorDescription('while saving a new chat message'),
        ),
      );
      if (mounted) _showMessage(l10n.messageSaveFailed);
      if (mounted) {
        setState(() {
          _isSending = false;
          _activeChatConversationId = null;
          _replacingAssistantMessageId = null;
        });
      }
      return;
    }

    if (responseToReplace == null) _messageController.clear();
    if (mounted) {
      setState(() {
        _activeChatConversationId = conversationId;
      });
    }
    String? assistantMessageId;
    String assistantContent = '';
    DateTime? assistantCreatedAt;
    var assistantReasoningSummaries = const <chat.ChatReasoningSummary>[];
    var assistantToolActivities = const <chat.ChatToolActivity>[];
    var persistence = Future<void>.value();
    StreamSubscription<OpenChatServiceEvent>? subscription;

    void updateReasoningSummaries(Object? value) {
      if (value == null) return;
      try {
        assistantReasoningSummaries = chat.ChatReasoningSummary.listFromJson(
          value,
        );
      } on FormatException catch (error, stackTrace) {
        FlutterError.reportError(
          FlutterErrorDetails(
            exception: error,
            stack: stackTrace,
            library: 'local_service',
            context: ErrorDescription(
              'while reading ChatGPT reasoning summary events',
            ),
          ),
        );
      }
    }

    void updateToolActivity(Object? value) {
      try {
        final activity = chat.ChatToolActivity.fromJson(value);
        final existingIndex = assistantToolActivities.indexWhere(
          (item) => item.callId == activity.callId,
        );
        final updated = List<chat.ChatToolActivity>.of(assistantToolActivities);
        if (existingIndex == -1) {
          updated.add(activity);
        } else {
          updated[existingIndex] = activity;
        }
        assistantToolActivities = List<chat.ChatToolActivity>.unmodifiable(
          updated,
        );
      } on FormatException catch (error, stackTrace) {
        FlutterError.reportError(
          FlutterErrorDetails(
            exception: error,
            stack: stackTrace,
            library: 'local_service',
            context: ErrorDescription('while reading tool activity events'),
          ),
        );
      }
    }

    try {
      final openCodeApiKey = providerId == 'opencode'
          ? await widget.openCodeApiKeyStore?.readApiKey()
          : null;
      final operation = await service.startOperation(
        'chat.send',
        params: <String, Object?>{
          'conversationId': conversationId,
          if (sharedInstructions.trim().isNotEmpty)
            'customInstructions': sharedInstructions,
          ...?switch (openCodeApiKey) {
            final apiKey? => <String, Object?>{'apiKey': apiKey},
            _ => null,
          },
          ...?switch (responseToReplace) {
            final replacement? => <String, Object?>{
              'excludedAssistantMessageId': replacement.id,
            },
            _ => null,
          },
          if (_selectedReasoningEffort != null)
            'reasoningEffort': _selectedReasoningEffort,
          'toolPermissionMode': toolPermissionMode.serviceValue,
        },
      );
      _activeChatOperation = operation;
      subscription = operation.events.listen(
        (event) {
          if (event.name == 'chat.tool.permission.requested') {
            unawaited(_handleToolPermissionRequest(service, event.data));
            return;
          }
          if (event.name != 'chat.started' &&
              event.name != 'chat.delta' &&
              event.name != 'chat.reasoning.delta' &&
              event.name != 'chat.tool.updated') {
            return;
          }
          final messageId = event.data['messageId'];
          final content = event.data['content'];
          final createdAt = event.data['createdAtUnixMs'];
          if (messageId is! String || content is! String || createdAt is! int) {
            return;
          }
          assistantMessageId = messageId;
          assistantContent = content;
          assistantCreatedAt = DateTime.fromMillisecondsSinceEpoch(
            createdAt,
            isUtc: true,
          );
          if (event.name == 'chat.reasoning.delta') {
            updateReasoningSummaries(event.data['reasoningGroups']);
          }
          if (event.name == 'chat.tool.updated') {
            updateToolActivity(event.data['toolActivity']);
          }
          final snapshot = chat.ChatMessage(
            id: messageId,
            role: chat.ChatMessageRole.assistant,
            content: content,
            createdAt: assistantCreatedAt,
            reasoningSummaries: assistantReasoningSummaries,
            toolActivities: assistantToolActivities,
            status: chat.ChatMessageStatus.streaming,
          );
          persistence = persistence.then(
            (_) => repository.saveMessage(
              conversationId: conversationId,
              message: snapshot,
            ),
          );
        },
        onError: (Object error, StackTrace stackTrace) {
          FlutterError.reportError(
            FlutterErrorDetails(
              exception: error,
              stack: stackTrace,
              library: 'local_service',
              context: ErrorDescription(
                'while receiving ChatGPT stream events',
              ),
            ),
          );
        },
      );
      final result = await operation.result;
      await persistence;
      final resultStatus = result['status'];
      final status = switch (resultStatus) {
        'completed' => chat.ChatMessageStatus.completed,
        'stopped' => chat.ChatMessageStatus.stopped,
        _ => throw const FormatException(
          'The ChatGPT response status was invalid.',
        ),
      };
      final outputTokens = switch (result['outputTokens']) {
        null => null,
        int value => value,
        _ => throw const FormatException(
          'The ChatGPT response metrics were invalid.',
        ),
      };
      final tokensPerSecond = switch (result['tokensPerSecond']) {
        null => null,
        num value => value.toDouble(),
        _ => throw const FormatException(
          'The ChatGPT response metrics were invalid.',
        ),
      };
      final elapsedMicroseconds = switch (result['elapsedMicroseconds']) {
        null => null,
        int value => value,
        _ => throw const FormatException(
          'The ChatGPT response metrics were invalid.',
        ),
      };
      updateReasoningSummaries(result['reasoningGroups']);
      final savedMessageId = assistantMessageId;
      final savedMessageCreatedAt = assistantCreatedAt;
      if (savedMessageId != null) {
        await repository.saveMessage(
          conversationId: conversationId,
          message: chat.ChatMessage(
            id: savedMessageId,
            role: chat.ChatMessageRole.assistant,
            content: assistantContent,
            createdAt: savedMessageCreatedAt,
            outputTokens: outputTokens,
            tokensPerSecond: tokensPerSecond,
            elapsed: elapsedMicroseconds == null
                ? null
                : Duration(microseconds: elapsedMicroseconds),
            reasoningSummaries: assistantReasoningSummaries,
            toolActivities: assistantToolActivities,
            status: status,
          ),
        );
        if (responseToReplace != null) {
          if (status == chat.ChatMessageStatus.completed) {
            try {
              await repository.deleteMessage(
                conversationId: conversationId,
                messageId: responseToReplace.id,
              );
            } on Object catch (error, stackTrace) {
              FlutterError.reportError(
                FlutterErrorDetails(
                  exception: error,
                  stack: stackTrace,
                  library: 'chat_history',
                  context: ErrorDescription(
                    'while replacing an earlier assistant response',
                  ),
                ),
              );
              if (mounted) {
                _showMessage(context.openchatL10n.responseReplaceFailed);
              }
            }
          } else {
            final discarded = await _discardReplacementAttempt(
              repository,
              conversationId,
              savedMessageId,
            );
            if (mounted) {
              _showMessage(
                discarded
                    ? context.openchatL10n.responseRetryNotCompleted
                    : context.openchatL10n.responseRetryCleanupFailed,
              );
            }
          }
        }
      }
    } on OpenChatServiceException catch (error) {
      await persistence;
      final failedMessageId = assistantMessageId;
      var retryCleanupSucceeded = true;
      if (failedMessageId != null) {
        if (responseToReplace == null) {
          await repository.saveMessage(
            conversationId: conversationId,
            message: chat.ChatMessage(
              id: failedMessageId,
              role: chat.ChatMessageRole.assistant,
              content: assistantContent,
              createdAt: assistantCreatedAt,
              reasoningSummaries: assistantReasoningSummaries,
              toolActivities: assistantToolActivities,
              status: chat.ChatMessageStatus.failed,
            ),
          );
        } else {
          retryCleanupSucceeded = await _discardReplacementAttempt(
            repository,
            conversationId,
            failedMessageId,
          );
        }
      }
      if (mounted) {
        if (retryCleanupSucceeded) {
          _showServiceFailure(error);
        } else {
          _showMessage(context.openchatL10n.responseRetryCleanupFailed);
        }
      }
    } on Object catch (error, stackTrace) {
      await persistence;
      final failedMessageId = assistantMessageId;
      var retryCleanupSucceeded = true;
      if (failedMessageId != null) {
        if (responseToReplace == null) {
          await repository.saveMessage(
            conversationId: conversationId,
            message: chat.ChatMessage(
              id: failedMessageId,
              role: chat.ChatMessageRole.assistant,
              content: assistantContent,
              createdAt: assistantCreatedAt,
              reasoningSummaries: assistantReasoningSummaries,
              toolActivities: assistantToolActivities,
              status: chat.ChatMessageStatus.failed,
            ),
          );
        } else {
          retryCleanupSucceeded = await _discardReplacementAttempt(
            repository,
            conversationId,
            failedMessageId,
          );
        }
      }
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'chat',
          context: ErrorDescription('while sending a chat message'),
        ),
      );
      if (mounted) {
        _showMessage(
          retryCleanupSucceeded
              ? context.openchatL10n.chatRequestFailed
              : context.openchatL10n.responseRetryCleanupFailed,
        );
      }
    } finally {
      await subscription?.cancel();
      _activeChatOperation = null;
      if (mounted) {
        setState(() {
          _isSending = false;
          if (_activeChatConversationId == conversationId) {
            _activeChatConversationId = null;
            _replacingAssistantMessageId = null;
          }
        });
      }
    }
  }

  Future<bool> _discardReplacementAttempt(
    ChatRepository repository,
    String conversationId,
    String messageId,
  ) async {
    try {
      await repository.deleteMessage(
        conversationId: conversationId,
        messageId: messageId,
      );
      return true;
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'chat_history',
          context: ErrorDescription('while discarding an incomplete retry'),
        ),
      );
      return false;
    }
  }

  void _stopMessage() {
    final operation = _activeChatOperation;
    if (operation != null) unawaited(operation.cancel());
  }

  Future<void> _handleToolPermissionRequest(
    OpenChatServiceClient service,
    Map<String, Object?> data,
  ) async {
    final requestId = data['approvalRequestId'];
    final toolName = data['toolName'];
    final targetPath = data['targetPath'];
    if (requestId is! String ||
        requestId.isEmpty ||
        toolName is! String ||
        targetPath is! String) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: const FormatException(
            'A tool permission request was invalid.',
          ),
          library: 'local_service',
        ),
      );
      unawaited(_activeChatOperation?.cancel());
      return;
    }

    var approved = false;
    try {
      if (mounted) {
        final decision = await showToolPermissionDialog(
          context: context,
          toolName: toolName,
          targetPath: targetPath,
          arguments: data['arguments'],
        );
        if (decision == ToolPermissionDecision.stop) {
          await _activeChatOperation?.cancel();
          return;
        }
        approved = decision == ToolPermissionDecision.allow;
      }
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'chat_tool_permissions',
          context: ErrorDescription('while asking for tool permission'),
        ),
      );
    }

    try {
      await service.call(
        'chat.tool.permission.respond',
        params: <String, Object?>{
          'approvalRequestId': requestId,
          'approved': approved,
        },
      );
    } on Object catch (error, stackTrace) {
      if (error is OpenChatServiceException &&
          error.code == 'tool_permission_request_unavailable') {
        unawaited(_activeChatOperation?.cancel());
        return;
      }
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'local_service',
          context: ErrorDescription('while responding to tool permission'),
        ),
      );
      unawaited(_activeChatOperation?.cancel());
      if (mounted) _showMessage(context.openchatL10n.chatRequestFailed);
    }
  }

  void _showServiceFailure(OpenChatServiceException error) {
    final l10n = context.openchatL10n;
    final message = switch (error.code) {
      'rate_limited' => l10n.providerRateLimited,
      'authentication_required' ||
      'refresh_rejected' => l10n.providerAuthenticationRequired,
      'provider_endpoint_unavailable' ||
      'invalid_provider_response' => l10n.providerRequestFailed,
      'model_unavailable' || 'conversation_not_routed' => l10n.modelRequired,
      'invalid_retry_target' => l10n.responseRetryUnavailable,
      _ => l10n.providerRequestFailed,
    };
    showOpenChatToast(context, message, type: OpenChatToastType.error);
  }

  void _showMessage(String message) {
    if (!mounted) return;
    showOpenChatToast(context, message, type: OpenChatToastType.error);
  }

  Future<void> _loadToolPermissionMode() async {
    try {
      final mode = await _settingsPreferences.readToolPermissionMode();
      if (!mounted) return;
      setState(() {
        _toolPermissionMode = mode;
        _toolPermissionModeReady = true;
        _isLoadingToolPermissionMode = false;
      });
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while loading tool access settings'),
        ),
      );
      if (!mounted) return;
      setState(() => _isLoadingToolPermissionMode = false);
      _showMessage(context.openchatL10n.toolPermissionSettingsLoadFailed);
    }
  }

  Future<void> _selectToolPermissionMode(ToolPermissionMode mode) async {
    if (_isLoadingToolPermissionMode ||
        _isSavingToolPermissionMode ||
        (mode == _toolPermissionMode && _toolPermissionModeReady)) {
      return;
    }
    setState(() => _isSavingToolPermissionMode = true);
    try {
      await _settingsPreferences.writeToolPermissionMode(mode);
      if (!mounted) return;
      setState(() {
        _toolPermissionMode = mode;
        _toolPermissionModeReady = true;
        _isSavingToolPermissionMode = false;
      });
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while saving tool access settings'),
        ),
      );
      if (!mounted) return;
      setState(() => _isSavingToolPermissionMode = false);
      _showMessage(context.openchatL10n.toolPermissionSettingsSaveFailed);
    }
  }

  Future<void> _toggleConversationPinned(String conversationId) async {
    await _updateConversationPinned(conversationId);
  }

  Future<void> _pinConversation(String conversationId) async {
    await _updateConversationPinned(conversationId, isPinned: true);
  }

  Future<void> _updateConversationPinned(
    String conversationId, {
    bool? isPinned,
  }) async {
    final repository = widget.chatRepository;
    if (repository == null) return;
    try {
      final conversation = await repository.getConversation(conversationId);
      if (conversation == null) {
        throw StateError('The conversation no longer exists.');
      }
      final nextPinnedState = isPinned ?? !conversation.isPinned;
      if (conversation.isPinned == nextPinnedState) return;
      await repository.setConversationPinned(
        conversationId: conversationId,
        isPinned: nextPinnedState,
      );
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'chat_history',
          context: ErrorDescription('while changing the pinned state'),
        ),
      );
      if (mounted) _showMessage(context.openchatL10n.chatHistoryUnavailable);
    }
  }

  Future<void> _moveConversationToChats(String conversationId) async {
    final repository = widget.chatRepository;
    if (repository == null) return;
    try {
      await repository.moveConversationToChats(conversationId);
    } on Exception catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'chat_projects',
          context: ErrorDescription('while moving a chat out of a project'),
        ),
      );
      if (mounted) _showMessage(context.openchatL10n.projectMoveFailed);
    }
  }

  Future<void> _createProject() async {
    final repository = widget.chatRepository;
    if (repository == null ||
        widget.historyStorageStatus != HistoryStorageStatus.available) {
      return;
    }

    final projectDetails = await showDialog<({String name, String folderPath})>(
      context: context,
      builder: (context) => const CreateProjectDialog(),
    );
    if (!mounted || projectDetails == null) return;

    try {
      await repository.createProject(
        id: _newLocalId(),
        name: projectDetails.name,
        folderPath: projectDetails.folderPath,
        createdAt: DateTime.now().toUtc(),
      );
      if (mounted) {
        showOpenChatToast(
          context,
          context.openchatL10n.projectCreated,
          type: OpenChatToastType.success,
        );
      }
    } on Exception catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'chat_projects',
          context: ErrorDescription('while creating a local project'),
        ),
      );
      if (mounted) _showMessage(context.openchatL10n.projectCreateFailed);
    }
  }

  Future<void> _moveConversationToProject(
    String conversationId,
    String projectId,
  ) async {
    final repository = widget.chatRepository;
    if (repository == null) return;
    try {
      await repository.moveConversationToProject(
        conversationId: conversationId,
        projectId: projectId,
      );
    } on Exception catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'chat_projects',
          context: ErrorDescription('while moving a chat into a project'),
        ),
      );
      if (mounted) _showMessage(context.openchatL10n.projectMoveFailed);
    }
  }

  Future<void> _handleConnectionRemoved(String connectionId) async {
    final repository = widget.chatRepository;
    if (repository != null) {
      try {
        await repository.clearConversationProviderForConnection(connectionId);
      } on Exception catch (error, stackTrace) {
        FlutterError.reportError(
          FlutterErrorDetails(
            exception: error,
            stack: stackTrace,
            library: 'chat_history',
            context: ErrorDescription(
              'while clearing a removed ChatGPT connection from chats',
            ),
          ),
        );
      }
    }
    await _loadProviderState();
  }

  Future<void> _refreshProviderState() async {
    if (_selectedProviderId == 'opencode') {
      await _loadModels(
        '',
        '',
        selectedModelId: _selectedModelId,
        forceRefresh: true,
      );
    } else {
      await _loadProviderState(forceRefresh: true);
    }
  }

  void _requestConversationRename(String conversationId) {
    if (_selectedConversationId != conversationId) {
      _selectConversation(conversationId);
    }
    setState(() => _titleEditRequestId = conversationId);
  }

  void _finishConversationTitleEdit() {
    if (_titleEditRequestId == null || !mounted) return;
    setState(() => _titleEditRequestId = null);
  }

  Future<void> _renameConversation(String conversationId, String title) async {
    final repository = widget.chatRepository;
    if (repository == null) {
      throw StateError('Chat history storage is unavailable.');
    }
    try {
      await repository.renameConversation(
        conversationId: conversationId,
        title: title,
      );
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'chat_history',
          context: ErrorDescription('while renaming a conversation'),
        ),
      );
      if (mounted) {
        showOpenChatToast(
          context,
          context.openchatL10n.conversationTitleSaveFailed,
          type: OpenChatToastType.error,
        );
      }
      rethrow;
    }
  }

  Future<void> _deleteConversation(String conversationId) async {
    final repository = widget.chatRepository;
    if (repository == null) return;
    if (_activeChatConversationId == conversationId) {
      _showMessage(context.openchatL10n.stopResponseBeforeDelete);
      return;
    }

    try {
      final conversation = await repository.getConversation(conversationId);
      if (conversation == null || !mounted) return;
      final l10n = context.openchatL10n;
      final confirmed = await showDialog<bool>(
        context: context,
        builder: (dialogContext) => AlertDialog(
          title: Text(l10n.confirmDeleteConversationTitle),
          content: Text(l10n.confirmDeleteConversation(conversation.title)),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(dialogContext).pop(false),
              child: Text(l10n.cancel),
            ),
            FilledButton(
              onPressed: () => Navigator.of(dialogContext).pop(true),
              child: Text(l10n.deleteConversation),
            ),
          ],
        ),
      );
      if (confirmed != true || !mounted) return;
      if (_activeChatConversationId == conversationId) {
        _showMessage(context.openchatL10n.stopResponseBeforeDelete);
        return;
      }

      await repository.deleteConversation(conversationId);
      if (!mounted) return;
      if (_selectedConversationId == conversationId) _startNewConversation();
      showOpenChatToast(
        context,
        context.openchatL10n.conversationDeleted,
        type: OpenChatToastType.success,
      );
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'chat_history',
          context: ErrorDescription('while deleting a conversation'),
        ),
      );
      if (mounted) _showMessage(context.openchatL10n.conversationDeleteFailed);
    }
  }

  Future<void> _exportConversation(String conversationId) async {
    final repository = widget.chatRepository;
    if (repository == null) return;

    try {
      final conversation = await repository.getConversation(conversationId);
      if (conversation == null || !mounted) return;
      final messages = await repository.getMessages(conversationId);
      if (!mounted) return;

      final l10n = context.openchatL10n;
      final markdown = buildConversationMarkdown(conversation, messages, l10n);

      final savedPath = await FilePicker.saveFile(
        fileName: conversationExportFileName(conversation.title),
        bytes: Uint8List.fromList(utf8.encode(markdown.toString())),
        mimeType: 'text/markdown',
        dialogTitle: l10n.exportConversation,
        type: FileType.custom,
        allowedExtensions: const ['md'],
        windowsOptions: const WindowsOptions(lockParentWindow: true),
        linuxOptions: const LinuxOptions(lockParentWindow: true),
      );
      if (savedPath == null || !mounted) return;
      showOpenChatToast(
        context,
        context.openchatL10n.conversationExported,
        type: OpenChatToastType.success,
      );
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'chat_history',
          context: ErrorDescription('while exporting a conversation'),
        ),
      );
      if (mounted) _showMessage(context.openchatL10n.conversationExportFailed);
    }
  }

  String _newLocalId() {
    _nextLocalId++;
    return '${DateTime.now().toUtc().microsecondsSinceEpoch}-$_nextLocalId';
  }

  @override
  void dispose() {
    _searchController.dispose();
    _messageController.dispose();
    _messageScrollController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    return StreamBuilder<List<ChatConversation>>(
      stream: _conversationStream,
      builder: (context, conversationSnapshot) {
        final conversations =
            conversationSnapshot.data ?? const <ChatConversation>[];
        ChatConversation? selectedConversation;
        for (final conversation in conversations) {
          if (conversation.id == _selectedConversationId) {
            selectedConversation = conversation;
            break;
          }
        }

        final isLoading =
            widget.historyStorageStatus == HistoryStorageStatus.loading ||
            (_conversationStream != null &&
                !conversationSnapshot.hasData &&
                !conversationSnapshot.hasError &&
                conversationSnapshot.connectionState ==
                    ConnectionState.waiting);
        final l10n = context.openchatL10n;
        final historyError = conversationSnapshot.hasError
            ? l10n.historyLoadFailed
            : widget.historyStorageStatus == HistoryStorageStatus.unavailable
            ? l10n.historyStorageUnavailableDescription
            : null;
        final resolvedStorageStatus = conversationSnapshot.hasError
            ? HistoryStorageStatus.unavailable
            : widget.historyStorageStatus;

        return LayoutBuilder(
          builder: (context, constraints) {
            final showSidebar =
                !_settingsOpen &&
                !_sidebarsCollapsed &&
                constraints.maxWidth >= OpenChatSpacing.sidebarBreakpoint;
            final expandedRail =
                constraints.maxWidth >= OpenChatSpacing.expandedRailBreakpoint;
            final sidebarWidth =
                constraints.maxWidth >= OpenChatSpacing.fullSidebarBreakpoint
                ? OpenChatSpacing.sidebarWidth
                : OpenChatSpacing.compactSidebarWidth;
            final drawerWidth = constraints.maxWidth < sidebarWidth
                ? constraints.maxWidth
                : sidebarWidth;

            return Scaffold(
              key: _scaffoldKey,
              drawer:
                  _settingsOpen ||
                      constraints.maxWidth >= OpenChatSpacing.sidebarBreakpoint
                  ? null
                  : Drawer(
                      width: drawerWidth,
                      backgroundColor: palette.surface,
                      child: SafeArea(
                        child: _buildSidebar(
                          width: drawerWidth,
                          showDivider: false,
                          conversations: conversations,
                          selectedConversationId: _selectedConversationId,
                          isLoading: isLoading,
                          errorMessage: historyError,
                        ),
                      ),
                    ),
              body: Stack(
                children: [
                  Row(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      if (!_sidebarsCollapsed)
                        ChatNavigationRail(
                          expanded: expandedRail,
                          settingsSelected: _settingsOpen,
                          onOpenChat: _startNewConversation,
                          onOpenSettings: () =>
                              setState(() => _settingsOpen = true),
                          onToggleTheme: _handleThemeToggle,
                          onCollapseSidebars: () =>
                              setState(() => _sidebarsCollapsed = true),
                        ),
                      if (_settingsOpen)
                        Expanded(
                          child: SettingsScreen(
                            themeMode: widget.themeMode,
                            locale: widget.locale,
                            settingsPreferences: _settingsPreferences,
                            onThemeModeChanged: widget.onThemeModeChanged,
                            onLocaleChanged: widget.onLocaleChanged,
                            historyStorageStatus: resolvedStorageStatus,
                            hasConversationHistory: conversations.isNotEmpty,
                            onClearConversationHistory:
                                widget.chatRepository == null
                                ? null
                                : _clearConversationHistory,
                            chatGptApiKeyStore: widget.chatGptApiKeyStore,
                            openCodeApiKeyStore: widget.openCodeApiKeyStore,
                            serviceClient: widget.serviceClient,
                            onProviderStateChanged: _refreshProviderState,
                            onConnectionRemoved: _handleConnectionRemoved,
                          ),
                        )
                      else ...[
                        if (showSidebar)
                          _buildSidebar(
                            width: sidebarWidth,
                            conversations: conversations,
                            selectedConversationId: _selectedConversationId,
                            isLoading: isLoading,
                            errorMessage: historyError,
                          ),
                        Expanded(
                          child: _buildConversationPane(
                            selectedConversation: selectedConversation,
                          ),
                        ),
                      ],
                    ],
                  ),
                  if (_sidebarsCollapsed && _settingsOpen)
                    Positioned(
                      left: 12,
                      top: 8,
                      child: IconButton(
                        tooltip: context.openchatL10n.showSidebars,
                        onPressed: () =>
                            setState(() => _sidebarsCollapsed = false),
                        icon: const Icon(Icons.chevron_right_rounded),
                      ),
                    ),
                ],
              ),
            );
          },
        );
      },
    );
  }

  Widget _buildSidebar({
    required double width,
    required List<ChatConversation> conversations,
    required String? selectedConversationId,
    required bool isLoading,
    required String? errorMessage,
    bool showDivider = true,
  }) {
    return StreamBuilder<List<ChatProject>>(
      stream: _projectStream,
      builder: (context, projectSnapshot) {
        final projectRows = projectSnapshot.data ?? const <ChatProject>[];
        final sidebarProjects = projectRows
            .map(
              (project) => ConversationSidebarProject(
                id: project.id,
                title: project.name,
                conversations: conversations
                    .where(
                      (conversation) =>
                          conversation.projectId == project.id &&
                          !conversation.isPinned,
                    )
                    .map(_sidebarConversationFromModel)
                    .toList(growable: false),
              ),
            )
            .toList(growable: false);
        final pinnedConversations = conversations
            .where((conversation) => conversation.isPinned)
            .toList(growable: false);
        final unassignedConversations = conversations
            .where(
              (conversation) =>
                  conversation.projectId == null && !conversation.isPinned,
            )
            .toList(growable: false);

        return ConversationSidebar(
          width: width,
          showDivider: showDivider,
          searchController: _searchController,
          projects: sidebarProjects,
          projectsLoading:
              _projectStream != null &&
              !projectSnapshot.hasData &&
              !projectSnapshot.hasError,
          projectLoadError: projectSnapshot.hasError
              ? context.openchatL10n.projectLoadFailed
              : null,
          pinnedConversations: pinnedConversations
              .map(_sidebarConversationFromModel)
              .toList(growable: false),
          conversations: unassignedConversations
              .map(_sidebarConversationFromModel)
              .toList(growable: false),
          selectedConversationId: selectedConversationId,
          onSelectConversation: _selectConversation,
          onToggleConversationPinned: (conversationId) =>
              unawaited(_toggleConversationPinned(conversationId)),
          onPinConversation: (conversationId) =>
              unawaited(_pinConversation(conversationId)),
          onRenameConversation: _requestConversationRename,
          onDeleteConversation: (conversationId) =>
              unawaited(_deleteConversation(conversationId)),
          onExportConversation: (conversationId) =>
              unawaited(_exportConversation(conversationId)),
          onMoveConversationToProject: (conversationId, projectId) =>
              unawaited(_moveConversationToProject(conversationId, projectId)),
          onMoveConversationToChats: (conversationId) =>
              unawaited(_moveConversationToChats(conversationId)),
          onCreateConversation: _startNewConversation,
          onCreateProject:
              widget.chatRepository == null ||
                  widget.historyStorageStatus != HistoryStorageStatus.available
              ? null
              : () => unawaited(_createProject()),
          isLoading: isLoading,
          errorMessage: errorMessage,
        );
      },
    );
  }

  ConversationSidebarConversation _sidebarConversationFromModel(
    ChatConversation conversation,
  ) {
    return ConversationSidebarConversation(
      id: conversation.id,
      title: conversation.title,
      isPinned: conversation.isPinned,
    );
  }

  Widget _buildConversationPane({
    required ChatConversation? selectedConversation,
  }) {
    final selectedModelId = _selectedModelId ?? selectedConversation?.modelId;
    ChatGptModel? selectedModel;
    for (final model in _models) {
      if (model.id == selectedModelId && model.isAvailable) {
        selectedModel = model;
        break;
      }
    }
    final modelLabel =
        selectedModel?.displayName ??
        selectedModelId ??
        widget.selectedModelLabel;
    final modelOptions = _models.where((model) => model.isAvailable).toList();
    final reasoningOptions = selectedModel?.reasoningLevels ?? const <String>[];
    final selectedReasoning =
        reasoningOptions.contains(_selectedReasoningEffort)
        ? _selectedReasoningEffort
        : selectedModel?.defaultReasoningLevel;
    final routeProviderId =
        selectedConversation?.providerId ?? _selectedProviderId;
    final routeConnectionId =
        selectedConversation?.connectionId ?? _selectedConnectionId;
    final routeWorkspaceId =
        selectedConversation?.workspaceId ?? _selectedWorkspaceId;
    final routeReady =
        routeProviderId == 'opencode' ||
        (routeConnectionId != null && routeWorkspaceId != null);
    final modelRouteReady =
        _loadedProviderId == routeProviderId &&
        (routeProviderId == 'opencode' ||
            (routeConnectionId == _loadedConnectionId &&
                routeWorkspaceId == _loadedWorkspaceId));
    final canSend =
        widget.historyStorageStatus == HistoryStorageStatus.available &&
        _toolPermissionModeReady &&
        !_isLoadingToolPermissionMode &&
        !_isSavingToolPermissionMode &&
        routeReady &&
        modelRouteReady &&
        selectedModel != null &&
        !_isUpdatingConversationModel &&
        !_isSending;
    final l10n = context.openchatL10n;
    final modelsEmptyLabel =
        _modelLoadError == null || _modelLoadError == 'empty'
        ? l10n.noModelsAvailable
        : l10n.modelsUnavailable;
    final resolvedModelLabel = _isLoadingModels
        ? l10n.modelsLoading
        : _modelLoadError == 'empty'
        ? l10n.noModelsAvailable
        : _modelLoadError != null
        ? l10n.modelsUnavailable
        : _modelFreshness == 'stale' && modelLabel != null
        ? '$modelLabel · ${l10n.cachedCatalog}'
        : modelLabel;

    return StreamBuilder<List<chat.ChatMessage>>(
      stream: selectedConversation == null ? null : _messageStream,
      builder: (context, messageSnapshot) {
        final storedMessages = selectedConversation == null
            ? const <chat.ChatMessage>[]
            : messageSnapshot.data ?? const <chat.ChatMessage>[];
        final messages =
            _replacingAssistantMessageId != null &&
                _activeChatConversationId == selectedConversation?.id
            ? storedMessages
                  .where(
                    (message) => message.id != _replacingAssistantMessageId,
                  )
                  .toList(growable: false)
            : storedMessages;
        return StreamBuilder<List<FavoriteModel>>(
          stream: _favoriteModelsStream,
          builder: (context, favoriteSnapshot) {
            return ConversationPane(
              messageController: _messageController,
              messageScrollController: _messageScrollController,
              showHistoryButton:
                  _sidebarsCollapsed ||
                  MediaQuery.sizeOf(context).width <
                      OpenChatSpacing.sidebarBreakpoint,
              historyButtonTooltip: _sidebarsCollapsed
                  ? context.openchatL10n.showSidebars
                  : null,
              onOpenHistory: () {
                if (_sidebarsCollapsed) {
                  setState(() => _sidebarsCollapsed = false);
                  if (MediaQuery.sizeOf(context).width <
                      OpenChatSpacing.sidebarBreakpoint) {
                    _scaffoldKey.currentState?.openDrawer();
                  }
                } else {
                  _scaffoldKey.currentState?.openDrawer();
                }
              },
              onSendMessage: () =>
                  unawaited(_sendMessage(selectedConversation)),
              onRetryResponse: selectedConversation == null
                  ? null
                  : (message) => unawaited(
                      _sendMessage(
                        selectedConversation,
                        responseToReplace: message,
                      ),
                    ),
              onStopMessage: _stopMessage,
              canSendMessage: canSend,
              isSending: _isSending,
              isLoadingModels: _isLoadingModels,
              models: modelOptions,
              favoriteModels: favoriteSnapshot.data ?? const <FavoriteModel>[],
              providerId: routeProviderId,
              onProviderSelected: selectedConversation == null
                  ? _selectProvider
                  : null,
              selectedModelId: selectedModelId,
              onModelSelected: _isUpdatingConversationModel
                  ? null
                  : _selectModel,
              onModelFavoriteChanged:
                  (providerId, modelId, displayName, isFavorite) => unawaited(
                    _setModelFavorite(
                      providerId: providerId,
                      modelId: modelId,
                      displayName: displayName,
                      isFavorite: isFavorite,
                    ),
                  ),
              onFavoriteModelSelected: (favorite) =>
                  unawaited(_selectFavoriteModel(favorite)),
              reasoningOptions: reasoningOptions,
              onReasoningSelected: _selectReasoning,
              selectedModelLabel: resolvedModelLabel,
              modelsEmptyLabel: modelsEmptyLabel,
              reasoningLevel: selectedReasoning,
              toolPermissionMode: _toolPermissionMode,
              onToolPermissionModeChanged:
                  _isLoadingToolPermissionMode || _isSavingToolPermissionMode
                  ? null
                  : _selectToolPermissionMode,
              conversationTitle: selectedConversation?.title,
              conversationId: selectedConversation?.id,
              titleEditRequestId: _titleEditRequestId,
              onRenameConversation: _renameConversation,
              onConversationTitleEditFinished: _finishConversationTitleEdit,
              assistantModelLabel: modelLabel,
              messages: messages,
              showAssistantLoading:
                  _isSending &&
                  _activeChatConversationId != null &&
                  _activeChatConversationId == _selectedConversationId,
              messagesLoading:
                  selectedConversation != null &&
                  !messageSnapshot.hasData &&
                  !messageSnapshot.hasError,
              messagesErrorDescription: messageSnapshot.hasError
                  ? context.openchatL10n.messageHistoryLoadFailed
                  : null,
            );
          },
        );
      },
    );
  }
}

Map<String, Object?> _objectMap(Object? value) {
  if (value is! Map) {
    throw const FormatException('A ChatGPT response item was invalid.');
  }
  final result = <String, Object?>{};
  for (final entry in value.entries) {
    if (entry.key is! String) {
      throw const FormatException('A ChatGPT response key was invalid.');
    }
    result[entry.key as String] = entry.value;
  }
  return result;
}
