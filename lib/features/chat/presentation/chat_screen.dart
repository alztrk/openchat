import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:file_picker/file_picker.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/openchat_toast.dart';
import 'package:openchat/app/openchat_window_title_bar.dart';
import 'package:openchat/features/chat/data/chat_repository.dart';
import 'package:openchat/features/chat/data/chat_stream_message_persister.dart';
import 'package:openchat/features/chat/data/chat_attachment_store.dart';
import 'package:openchat/features/chat/data/chat_file_changes_repository.dart';
import 'package:openchat/features/chat/data/conversation_memory_repository.dart';
import 'package:openchat/features/chat/data/history_search_repository.dart';
import 'package:openchat/features/chat/domain/chat_conversation.dart';
import 'package:openchat/features/chat/domain/agent_question.dart';
import 'package:openchat/features/chat/domain/agent_goal.dart';
import 'package:openchat/features/chat/domain/chat_file_change.dart';
import 'package:openchat/features/chat/domain/chat_message.dart' as chat;
import 'package:openchat/features/chat/domain/chat_attachment.dart';
import 'package:openchat/features/chat/domain/chat_project.dart';
import 'package:openchat/features/chat/domain/chatgpt_connection.dart';
import 'package:openchat/features/chat/domain/conversation_sidebar_data.dart';
import 'package:openchat/features/chat/domain/default_model_preference.dart';
import 'package:openchat/features/chat/domain/history_storage_status.dart';
import 'package:openchat/features/chat/domain/history_search_result.dart';
import 'package:openchat/features/chat/domain/model_favorite.dart';
import 'package:openchat/features/chat/domain/tool_permission_request.dart';
import 'package:openchat/features/models/data/hugging_face_models_repository.dart';
import 'package:openchat/features/models/presentation/local_models_page.dart';
import 'package:openchat/features/models/presentation/models_page.dart';
import 'package:openchat/features/settings/data/api_compatible_provider_key_store.dart';
import 'package:openchat/features/settings/data/chat_gpt_api_key_store.dart';
import 'package:openchat/features/settings/data/open_code_api_key_store.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/features/settings/domain/chat_gpt_api_key_connection.dart';
import 'package:openchat/features/settings/presentation/settings_screen.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';
import 'package:openchat/platform/windows/user_question_notifications.dart';
import 'package:openchat/platform/windows/window_controls.dart';

import 'package:openchat/features/chat/presentation/conversation_markdown_export.dart';
import 'package:openchat/features/chat/presentation/widgets/chat_navigation_rail.dart';
import 'package:openchat/features/chat/presentation/widgets/conversation_pane.dart';
import 'package:openchat/features/chat/presentation/widgets/conversation_sidebar.dart';
import 'package:openchat/features/chat/presentation/widgets/agent_run_manager_dialog.dart';
import 'package:openchat/features/chat/presentation/widgets/create_project_dialog.dart';
import 'package:openchat/features/chat/presentation/widgets/project_tool_permissions_dialog.dart';
import 'package:openchat/features/chat/presentation/widgets/project_worktrees_dialog.dart';

const _localEngineProviderIds = <String>{'llama_cpp', 'vllm', 'exllama'};

enum _ProjectOptionsAction { toolPermissions, worktrees }

class ChatScreen extends StatefulWidget {
  const ChatScreen({
    required this.themeMode,
    required this.onThemeModeChanged,
    required this.onToggleTheme,
    required this.historyStorageStatus,
    this.settingsPreferences,
    this.locale,
    this.conversationWidth = ConversationWidthPreference.normal,
    this.conversationTextSize = ConversationTextSizePreference.normal,
    this.appFont = AppFontPreference.manrope,
    this.onLocaleChanged,
    this.onConversationWidthChanged,
    this.onConversationTextSizeChanged,
    this.onAppFontChanged,
    this.chatRepository,
    this.apiCompatibleProviderKeyStore,
    this.chatGptApiKeyStore,
    this.openCodeApiKeyStore,
    this.serviceClient,
    this.onRetryStorage,
    this.selectedModelLabel,
    super.key,
  });

  final ThemeMode themeMode;
  final Locale? locale;
  final ConversationWidthPreference conversationWidth;
  final ConversationTextSizePreference conversationTextSize;
  final AppFontPreference appFont;
  final Future<void> Function(ThemeMode) onThemeModeChanged;
  final Future<void> Function(Locale?)? onLocaleChanged;
  final Future<void> Function(ConversationWidthPreference)?
  onConversationWidthChanged;
  final Future<void> Function(ConversationTextSizePreference)?
  onConversationTextSizeChanged;
  final Future<void> Function(AppFontPreference)? onAppFontChanged;
  final Future<void> Function() onToggleTheme;
  final ChatRepository? chatRepository;
  final ApiCompatibleProviderKeyStore? apiCompatibleProviderKeyStore;
  final ChatGptApiKeyStore? chatGptApiKeyStore;
  final OpenCodeApiKeyStore? openCodeApiKeyStore;
  final OpenChatServiceClient? serviceClient;
  final VoidCallback? onRetryStorage;
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
  final List<ChatAttachment> _pendingAttachments = <ChatAttachment>[];
  bool _settingsOpen = false;
  bool _modelsPageOpen = false;
  bool _localModelsPageOpen = false;
  bool _sidebarsCompact = false;
  Set<String> _collapsedSidebarSections = const <String>{};
  bool _isSending = false;
  bool _isCreatingMessageBranch = false;
  bool _isLoadingToolPermissionMode = true;
  bool _isSavingToolPermissionMode = false;
  bool _toolPermissionModeReady = false;
  ToolPermissionMode _toolPermissionMode = ToolPermissionMode.requireApproval;
  bool _chatGptFastModeEnabled = false;
  bool _isLoadingChatGptFastMode = true;
  bool _isSavingChatGptFastMode = false;
  ToolPermissionRequest? _pendingToolPermissionRequest;
  bool _isRespondingToToolPermission = false;
  String? _toolPermissionError;
  List<AgentQuestionGroup> _pendingQuestionGroups =
      const <AgentQuestionGroup>[];
  AgentGoal? _activeGoal;
  bool _isChangingGoal = false;
  int _goalLoadGeneration = 0;
  bool _isResumingQuestion = false;
  bool _isLoadingPendingQuestions = false;
  bool _questionLoadFailed = false;
  int _questionListGeneration = 0;
  String? _focusedQuestionGroupId;
  final Set<String> _questionRunRefreshConversations = <String>{};
  StreamSubscription<OpenChatServiceEvent>? _questionServiceEvents;
  late final UserQuestionNotifications _questionNotifications =
      UserQuestionNotifications();
  late final Future<void> _questionNotificationsReady;
  final Set<String> _notificationRepliesInFlight = <String>{};
  String? _activeChatConversationId;
  String? _replacingAssistantMessageId;
  bool _isLoadingConnections = false;
  bool _providerStateReloadRequested = false;
  bool _forceProviderStateReloadRequested = false;
  bool _isLoadingModels = false;
  bool _isUpdatingConversationModel = false;
  String? _selectedConversationId;
  String? _pendingProjectId;
  String _selectedProviderId = 'opencode';
  String? _titleEditRequestId;
  String? _selectedConnectionId;
  String? _selectedWorkspaceId;
  String? _selectedApiKeyConnectionId;
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
  Set<String> _loadedApiKeyConnectionIds = const <String>{};
  Set<String> _availableCompatibleProviderIds = const <String>{};
  Set<String> _availableChatGptApiKeyConnectionIds = const <String>{};
  Set<String> _hiddenModelKeys = const <String>{};
  DefaultModelPreference? _defaultModelPreference;
  bool _isChatGptOAuthAvailable = false;
  bool _isChatGptConnected = false;
  List<ChatGptApiKeyConnection> _chatGptApiKeyConnections =
      const <ChatGptApiKeyConnection>[];
  bool _isChatGptAvailable = false;
  bool _hasUserSelectedProvider = false;
  OpenChatServiceOperation? _activeChatOperation;
  Stream<List<ChatConversation>>? _conversationStream;
  Stream<List<ChatProject>>? _projectStream;
  Stream<List<chat.ChatMessage>>? _messageStream;
  Stream<List<FavoriteModel>>? _favoriteModelsStream;
  ConversationMemoryRepository? _conversationMemoryRepository;
  ChatFileChangesRepository? _chatFileChangesRepository;
  List<ChatFileChange> _conversationFileChanges = const <ChatFileChange>[];
  String? _fileChangesConversationId;
  int _fileChangesRevision = 0;
  int _fileChangesLoadGeneration = 0;
  bool _isFileChangesPanelOpen = false;
  HistorySearchRepository? _historySearchRepository;
  String? _historySearchQuery;
  List<HistorySearchResult> _historySearchResults =
      const <HistorySearchResult>[];
  String? _historySearchErrorCode;
  bool _isSearchingHistory = false;
  int _historySearchGeneration = 0;
  Timer? _historySearchDebounceTimer;
  bool _isHistorySearchOpen = false;
  String? _historyTargetConversationId;
  String? _historyTargetMessageId;
  int _historyTargetRequestId = 0;
  HuggingFaceDownloadController? _modelDownloadController;

  @override
  void initState() {
    super.initState();
    final serviceClient = widget.serviceClient;
    _conversationMemoryRepository = serviceClient == null
        ? null
        : ConversationMemoryRepository(serviceClient);
    _chatFileChangesRepository = serviceClient == null
        ? null
        : ChatFileChangesRepository(serviceClient);
    _historySearchRepository = serviceClient == null
        ? null
        : HistorySearchRepository(serviceClient);
    _modelDownloadController = serviceClient == null
        ? null
        : HuggingFaceDownloadController(
            HuggingFaceModelsRepository(serviceClient),
          );
    _modelDownloadController?.addListener(_handleModelDownloadChanged);
    _bindRepositoryStreams();
    _bindQuestionServiceEvents();
    _questionNotificationsReady = _initializeQuestionNotifications();
    unawaited(_questionNotificationsReady);
    unawaited(_loadChatGptFastMode());
    unawaited(_loadToolPermissionMode());
    unawaited(_loadCollapsedSidebarSections());
    if (widget.historyStorageStatus == HistoryStorageStatus.available) {
      unawaited(_loadProviderState());
    }
  }

  @override
  void didUpdateWidget(covariant ChatScreen oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.serviceClient != widget.serviceClient) {
      _fileChangesLoadGeneration++;
      _isFileChangesPanelOpen = false;
      _fileChangesConversationId = null;
      _conversationFileChanges = const <ChatFileChange>[];
      unawaited(_questionServiceEvents?.cancel());
      _questionServiceEvents = null;
      _bindQuestionServiceEvents();
      final serviceClient = widget.serviceClient;
      _historySearchDebounceTimer?.cancel();
      _historySearchDebounceTimer = null;
      _historySearchGeneration++;
      _conversationMemoryRepository = serviceClient == null
          ? null
          : ConversationMemoryRepository(serviceClient);
      _chatFileChangesRepository = serviceClient == null
          ? null
          : ChatFileChangesRepository(serviceClient);
      _historySearchRepository = serviceClient == null
          ? null
          : HistorySearchRepository(serviceClient);
      _isSearchingHistory = false;
      _historySearchErrorCode = _historySearchQuery == null ? null : 'failed';
      final oldController = _modelDownloadController;
      oldController?.removeListener(_handleModelDownloadChanged);
      oldController?.dispose();
      _modelDownloadController = serviceClient == null
          ? null
          : HuggingFaceDownloadController(
              HuggingFaceModelsRepository(serviceClient),
            );
      _modelDownloadController?.addListener(_handleModelDownloadChanged);
    }
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
      final conversationId = _selectedConversationId;
      if (conversationId != null) unawaited(_loadActiveGoal(conversationId));
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

  void _selectConversation(
    String conversationId, {
    String? historyMessageId,
    bool loadModels = true,
    bool loadPendingQuestions = true,
  }) {
    _modelSelectionGeneration++;
    _questionListGeneration++;
    _goalLoadGeneration++;
    _fileChangesLoadGeneration++;
    _resetMessageScroll();
    _historyTargetRequestId++;
    setState(() {
      _selectedConversationId = conversationId;
      _pendingProjectId = null;
      _isFileChangesPanelOpen = false;
      _fileChangesConversationId = null;
      _conversationFileChanges = const <ChatFileChange>[];
      _historyTargetConversationId = historyMessageId == null
          ? null
          : conversationId;
      _historyTargetMessageId = historyMessageId;
      _settingsOpen = false;
      _modelsPageOpen = false;
      _localModelsPageOpen = false;
      _messageStream = widget.chatRepository?.watchMessages(conversationId);
      _selectedModelId = null;
      _selectedReasoningEffort = null;
      _isUpdatingConversationModel = false;
      _titleEditRequestId = null;
      _pendingQuestionGroups = const <AgentQuestionGroup>[];
      _activeGoal = null;
      _isLoadingPendingQuestions = true;
      _questionLoadFailed = false;
    });
    if (loadModels) unawaited(_loadConversationModels(conversationId));
    if (loadPendingQuestions) {
      unawaited(_loadPendingQuestionGroups(conversationId));
    }
    unawaited(_loadActiveGoal(conversationId));
    unawaited(_loadConversationFileChanges(conversationId));
  }

  Future<void> _loadActiveGoal(String conversationId) async {
    final service = widget.serviceClient;
    if (service == null) return;
    final generation = ++_goalLoadGeneration;
    try {
      final response = await service.call(
        'chat.goal.active',
        params: <String, Object?>{'conversationId': conversationId},
      );
      if (!response.containsKey('goal')) {
        throw const FormatException('The active goal response was invalid.');
      }
      final goal = AgentGoal.fromJson(response['goal']);
      if (!mounted ||
          generation != _goalLoadGeneration ||
          _selectedConversationId != conversationId) {
        return;
      }
      setState(() => _activeGoal = goal);
    } on OpenChatServiceException catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'chat_goal',
          context: ErrorDescription('while loading the active goal'),
        ),
      );
    } on FormatException catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'chat_goal',
          context: ErrorDescription('while parsing the active goal'),
        ),
      );
    }
  }

  Future<void> _loadConversationFileChanges(String conversationId) async {
    final repository = _chatFileChangesRepository;
    if (repository == null) return;
    final generation = ++_fileChangesLoadGeneration;
    try {
      final changes = await repository.list(conversationId);
      if (!mounted ||
          generation != _fileChangesLoadGeneration ||
          _selectedConversationId != conversationId) {
        return;
      }
      setState(() {
        _fileChangesConversationId = conversationId;
        _conversationFileChanges = changes;
      });
    } on OpenChatServiceException catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'chat_file_changes',
          context: ErrorDescription('while loading conversation changes'),
        ),
      );
    } on FormatException catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'chat_file_changes',
          context: ErrorDescription('while parsing conversation changes'),
        ),
      );
    }
  }

  void _updateConversationFileChanges(
    String conversationId,
    List<ChatFileChange> changes,
  ) {
    if (!mounted || _selectedConversationId != conversationId) return;
    setState(() {
      _fileChangesConversationId = conversationId;
      _conversationFileChanges = List<ChatFileChange>.unmodifiable(changes);
    });
  }

  void _openConversationFileChanges() {
    if (_selectedConversationId == null || _chatFileChangesRepository == null) {
      return;
    }
    setState(() => _isFileChangesPanelOpen = true);
  }

  void _closeConversationFileChanges() {
    if (!_isFileChangesPanelOpen) return;
    setState(() => _isFileChangesPanelOpen = false);
  }

  void _openHistorySearch() {
    if (_isHistorySearchOpen) return;
    setState(() => _isHistorySearchOpen = true);
  }

  Future<void> _openRunManager() async {
    final serviceClient = widget.serviceClient;
    if (serviceClient == null ||
        widget.historyStorageStatus != HistoryStorageStatus.available) {
      return;
    }
    await showDialog<void>(
      context: context,
      builder: (context) => AgentRunManagerDialog(
        serviceClient: serviceClient,
        onOpenConversation: _selectConversation,
      ),
    );
  }

  void _closeHistorySearch() {
    _historySearchDebounceTimer?.cancel();
    _historySearchDebounceTimer = null;
    _historySearchGeneration++;
    _searchController.clear();
    setState(() {
      _isHistorySearchOpen = false;
      _historySearchQuery = null;
      _historySearchResults = const <HistorySearchResult>[];
      _historySearchErrorCode = null;
      _isSearchingHistory = false;
    });
  }

  void _handleHistorySearchChanged(String rawQuery) {
    final query = rawQuery.trim();
    final queryLength = query.runes.length;
    _historySearchDebounceTimer?.cancel();
    _historySearchDebounceTimer = null;
    if (query.isEmpty || queryLength < 2 || queryLength > 512) {
      _submitHistorySearch(query);
      return;
    }

    final repository = _historySearchRepository;
    if (repository == null) {
      _submitHistorySearch(query);
      return;
    }

    final generation = ++_historySearchGeneration;
    setState(() {
      _historySearchQuery = query;
      _historySearchResults = const <HistorySearchResult>[];
      _historySearchErrorCode = null;
      _isSearchingHistory = true;
    });
    _historySearchDebounceTimer = Timer(const Duration(milliseconds: 240), () {
      _historySearchDebounceTimer = null;
      unawaited(_loadHistorySearchResults(repository, query, generation));
    });
  }

  void _submitHistorySearch(String rawQuery) {
    _historySearchDebounceTimer?.cancel();
    _historySearchDebounceTimer = null;
    final query = rawQuery.trim();
    final generation = ++_historySearchGeneration;
    final queryLength = query.runes.length;
    if (query.isEmpty) {
      setState(() {
        _historySearchQuery = null;
        _historySearchResults = const <HistorySearchResult>[];
        _historySearchErrorCode = null;
        _isSearchingHistory = false;
      });
      return;
    }
    if (queryLength < 2 || queryLength > 512) {
      setState(() {
        _historySearchQuery = query;
        _historySearchResults = const <HistorySearchResult>[];
        _historySearchErrorCode = queryLength < 2 ? 'too_short' : 'too_long';
        _isSearchingHistory = false;
      });
      return;
    }
    final repository = _historySearchRepository;
    if (repository == null) {
      setState(() {
        _historySearchQuery = query;
        _historySearchResults = const <HistorySearchResult>[];
        _historySearchErrorCode = 'failed';
        _isSearchingHistory = false;
      });
      return;
    }
    setState(() {
      _historySearchQuery = query;
      _historySearchResults = const <HistorySearchResult>[];
      _historySearchErrorCode = null;
      _isSearchingHistory = true;
    });
    unawaited(_loadHistorySearchResults(repository, query, generation));
  }

  Future<void> _loadHistorySearchResults(
    HistorySearchRepository repository,
    String query,
    int generation,
  ) async {
    try {
      final results = await repository.search(query);
      if (!mounted || generation != _historySearchGeneration) return;
      setState(() {
        _historySearchResults = results;
        _historySearchErrorCode = null;
        _isSearchingHistory = false;
      });
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'chat_history_search',
          context: ErrorDescription('while searching saved chat messages'),
        ),
      );
      if (!mounted || generation != _historySearchGeneration) return;
      setState(() {
        _historySearchResults = const <HistorySearchResult>[];
        _historySearchErrorCode = 'failed';
        _isSearchingHistory = false;
      });
    }
  }

  void _selectHistorySearchResult(HistorySearchResult result) {
    _closeHistorySearch();
    _selectConversation(
      result.conversationId,
      historyMessageId: result.messageId,
    );
    _scaffoldKey.currentState?.closeDrawer();
  }

  void _handleHistorySearchTarget(String messageId, bool found) {
    if (!mounted || _historyTargetMessageId != messageId) return;
    setState(() {
      _historyTargetConversationId = null;
      _historyTargetMessageId = null;
    });
    if (!found) _showMessage(context.openchatL10n.searchMessageUnavailable);
  }

  void _bindQuestionServiceEvents() {
    final service = widget.serviceClient;
    if (service == null) return;
    _questionServiceEvents = service.events.listen(
      (event) {
        final conversationId = _selectedConversationId;
        if (event.name == 'local_service.recovered' && conversationId != null) {
          unawaited(_loadPendingQuestionGroups(conversationId));
        }
      },
      onError: (Object error, StackTrace stackTrace) {
        FlutterError.reportError(
          FlutterErrorDetails(
            exception: error,
            stack: stackTrace,
            library: 'user_questions',
            context: ErrorDescription(
              'while listening for local service recovery',
            ),
          ),
        );
      },
    );
  }

  Future<void> _initializeQuestionNotifications() async {
    try {
      final launchTarget = await _questionNotifications.initialize(
        onSelected: _openQuestionNotification,
      );
      if (launchTarget != null) _openQuestionNotification(launchTarget);
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'notifications',
          context: ErrorDescription(
            'while initializing AI question notifications',
          ),
        ),
      );
    }
  }

  void _openQuestionNotification(ConversationNotificationTarget target) {
    if (!mounted) return;
    if (target.kind == ConversationNotificationKind.assistantResponseReply) {
      unawaited(_replyToAssistantNotification(target));
      return;
    }
    if (_selectedConversationId != target.conversationId) {
      _selectConversation(target.conversationId);
    } else {
      setState(() {
        _settingsOpen = false;
        _modelsPageOpen = false;
        _localModelsPageOpen = false;
      });
    }
    if (target.kind == ConversationNotificationKind.userQuestion) {
      unawaited(_focusQuestionNotification(target));
    }
  }

  Future<void> _replyToAssistantNotification(
    ConversationNotificationTarget target,
  ) async {
    final assistantMessageId = target.assistantMessageId;
    if (target.kind != ConversationNotificationKind.assistantResponseReply ||
        assistantMessageId == null) {
      return;
    }
    final actionKey = '${target.conversationId}:$assistantMessageId';
    if (!_notificationRepliesInFlight.add(actionKey)) return;
    var userMessageSaved = false;

    try {
      if (!mounted) return;
      final l10n = context.openchatL10n;
      final replyText = target.replyText?.trim() ?? '';
      final repository = widget.chatRepository;
      final service = widget.serviceClient;
      final shouldLoadReplyState =
          repository != null &&
          service != null &&
          !target.replyInputTooLong &&
          replyText.isNotEmpty;
      if (_selectedConversationId != target.conversationId) {
        _selectConversation(
          target.conversationId,
          loadModels: !shouldLoadReplyState,
          loadPendingQuestions: !shouldLoadReplyState,
        );
      } else {
        setState(() {
          _settingsOpen = false;
          _modelsPageOpen = false;
          _localModelsPageOpen = false;
        });
      }

      if (target.replyInputTooLong) {
        _showMessage(l10n.assistantResponseNotificationReplyTooLong);
        return;
      }
      if (replyText.isEmpty) {
        _showMessage(l10n.assistantResponseNotificationReplyEmpty);
        return;
      }

      if (repository == null || service == null) {
        _keepNotificationReplyInComposer(replyText);
        _showMessage(l10n.assistantResponseNotificationReplyNotSent);
        return;
      }

      await Future.wait<void>(<Future<void>>[
        _loadConversationModels(target.conversationId),
        _loadPendingQuestionGroups(target.conversationId),
      ]);
      if (!mounted) return;
      if (_selectedConversationId != target.conversationId) {
        _showMessage(l10n.assistantResponseNotificationReplyNotSent);
        return;
      }
      if (_questionLoadFailed) {
        _keepNotificationReplyInComposer(replyText);
        return;
      }
      if (_pendingQuestionGroups.isNotEmpty) {
        _keepNotificationReplyInComposer(replyText);
        _showMessage(l10n.userQuestionRequiredValidation);
        return;
      }
      if (_isSending ||
          _isUpdatingConversationModel ||
          !_toolPermissionModeReady ||
          _isLoadingToolPermissionMode ||
          _isSavingToolPermissionMode) {
        _keepNotificationReplyInComposer(replyText);
        _showMessage(l10n.assistantResponseNotificationReplyNotSent);
        return;
      }

      final conversation = await repository.getConversation(
        target.conversationId,
      );
      if (!mounted) return;
      if (_selectedConversationId != target.conversationId) {
        _showMessage(l10n.assistantResponseNotificationReplyNotSent);
        return;
      }
      if (conversation == null) {
        _keepNotificationReplyInComposer(replyText);
        _showMessage(l10n.assistantResponseNotificationReplyUnavailable);
        return;
      }

      final messages = await repository.getMessages(target.conversationId);
      if (!mounted) return;
      if (_selectedConversationId != target.conversationId) {
        _showMessage(l10n.assistantResponseNotificationReplyNotSent);
        return;
      }
      final assistantMessageIndex = messages.indexWhere(
        (message) => message.id == assistantMessageId,
      );
      if (assistantMessageIndex != messages.length - 1 ||
          assistantMessageIndex < 0 ||
          messages[assistantMessageIndex].role !=
              chat.ChatMessageRole.assistant ||
          messages[assistantMessageIndex].status !=
              chat.ChatMessageStatus.completed) {
        _keepNotificationReplyInComposer(replyText);
        _showMessage(l10n.assistantResponseNotificationReplyUnavailable);
        return;
      }

      await _sendMessage(
        conversation,
        messageTextOverride: replyText,
        onUserMessageSaved: () => userMessageSaved = true,
      );
      if (!userMessageSaved) _keepNotificationReplyInComposer(replyText);
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'notifications',
          context: ErrorDescription(
            'while sending a reply from an assistant response notification',
          ),
        ),
      );
      if (mounted && !userMessageSaved) {
        if (_selectedConversationId == target.conversationId) {
          _keepNotificationReplyInComposer(target.replyText ?? '');
        }
        _showMessage(
          context.openchatL10n.assistantResponseNotificationReplyNotSent,
        );
      }
    } finally {
      _notificationRepliesInFlight.remove(actionKey);
    }
  }

  void _keepNotificationReplyInComposer(String replyText) {
    if (!mounted ||
        _messageController.text.trim().isNotEmpty ||
        replyText.isEmpty) {
      return;
    }
    _messageController.value = TextEditingValue(
      text: replyText,
      selection: TextSelection.collapsed(offset: replyText.length),
    );
  }

  Future<void> _focusQuestionNotification(
    ConversationNotificationTarget target,
  ) async {
    final groupId = target.groupId;
    if (groupId == null) return;
    await _loadPendingQuestionGroups(target.conversationId);
    if (!mounted ||
        _selectedConversationId != target.conversationId ||
        _questionLoadFailed) {
      return;
    }
    if (!_pendingQuestionGroups.any((group) => group.id == groupId)) {
      _showMessage(context.openchatL10n.userQuestionUnavailable);
      return;
    }
    setState(() => _focusedQuestionGroupId = groupId);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && _focusedQuestionGroupId == groupId) {
        setState(() => _focusedQuestionGroupId = null);
      }
    });
  }

  Future<void> _showAssistantResponseNotification({
    required String conversationId,
    required String assistantMessageId,
    required String providerId,
    required String content,
  }) async {
    if (!mounted ||
        !Platform.isWindows ||
        !UserQuestionNotifications.isApplicationBackgrounded(
          WidgetsBinding.instance.lifecycleState,
        )) {
      return;
    }
    final l10n = context.openchatL10n;
    final repository = widget.chatRepository;
    if (repository == null ||
        UserQuestionNotifications.assistantResponsePreview(content).isEmpty) {
      return;
    }
    try {
      await _questionNotificationsReady;
      if (!UserQuestionNotifications.isApplicationBackgrounded(
        WidgetsBinding.instance.lifecycleState,
      )) {
        return;
      }
      final conversation = await repository.getConversation(conversationId);
      if (conversation == null) {
        throw StateError('The completed conversation was not found.');
      }
      if (!UserQuestionNotifications.isApplicationBackgrounded(
        WidgetsBinding.instance.lifecycleState,
      )) {
        return;
      }
      await _questionNotifications.showAssistantResponse(
        conversationId: conversationId,
        assistantMessageId: assistantMessageId,
        providerId: providerId,
        title: conversation.title,
        content: content,
        replyLabel: l10n.messageHint,
        sendLabel: l10n.send,
      );
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'notifications',
          context: ErrorDescription(
            'while notifying about a completed assistant response',
          ),
        ),
      );
    }
  }

  Future<void> _loadPendingQuestionGroups(String conversationId) async {
    final generation = ++_questionListGeneration;
    final service = widget.serviceClient;
    if (service == null) {
      if (mounted &&
          generation == _questionListGeneration &&
          _selectedConversationId == conversationId) {
        setState(() {
          _isLoadingPendingQuestions = false;
          _questionLoadFailed = true;
        });
      }
      return;
    }
    if (mounted &&
        generation == _questionListGeneration &&
        _selectedConversationId == conversationId) {
      setState(() => _isLoadingPendingQuestions = true);
    }
    try {
      final response = await service.call(
        'chat.questions.list',
        params: <String, Object?>{'conversationId': conversationId},
      );
      final rawGroups = response['questions'];
      if (rawGroups is! List<Object?>) {
        throw const FormatException('The pending question list was invalid.');
      }
      final groups = rawGroups
          .map(AgentQuestionGroup.fromJson)
          .toList(growable: false);
      if (!mounted ||
          generation != _questionListGeneration ||
          _selectedConversationId != conversationId) {
        return;
      }
      setState(() {
        _pendingQuestionGroups = groups;
        _isLoadingPendingQuestions = false;
        _questionLoadFailed = false;
      });
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'user_questions',
          context: ErrorDescription('while loading pending AI questions'),
        ),
      );
      if (!mounted ||
          generation != _questionListGeneration ||
          _selectedConversationId != conversationId) {
        return;
      }
      setState(() {
        _isLoadingPendingQuestions = false;
        _questionLoadFailed = true;
      });
      _showMessage(context.openchatL10n.userQuestionLoadFailed);
    }
  }

  Future<void> _handleQuestionRequested(Map<String, Object?> data) async {
    AgentQuestionGroup group;
    try {
      group = AgentQuestionGroup.fromJson(data);
    } on FormatException catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'user_questions',
          context: ErrorDescription('while reading a pending AI question'),
        ),
      );
      final activeConversationId = _selectedConversationId;
      if (mounted && activeConversationId != null) {
        unawaited(_loadPendingQuestionGroups(activeConversationId));
      }
      if (mounted) _showMessage(context.openchatL10n.userQuestionLoadFailed);
      return;
    }

    if (mounted && _selectedConversationId == group.conversationId) {
      _questionListGeneration++;
      setState(() {
        _pendingQuestionGroups = <AgentQuestionGroup>[
          ..._pendingQuestionGroups.where((item) => item.id != group.id),
          group,
        ];
        _isLoadingPendingQuestions = false;
        _questionLoadFailed = false;
      });
    }
    if (!Platform.isWindows) return;
    final l10n = context.openchatL10n;
    try {
      await _questionNotificationsReady;
      await _questionNotifications.show(
        groupId: group.id,
        conversationId: group.conversationId,
        title: l10n.userQuestionNotificationTitle,
        body: l10n.userQuestionNotificationBody,
      );
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'notifications',
          context: ErrorDescription(
            'while notifying about a pending AI question',
          ),
        ),
      );
    }
  }

  Future<String?> _submitQuestionAnswers(
    ChatConversation? selectedConversation,
    AgentQuestionGroup group,
    List<AgentQuestionAnswer> answers,
  ) async {
    final l10n = context.openchatL10n;
    if (selectedConversation == null ||
        selectedConversation.id != group.conversationId) {
      return l10n.userQuestionUnavailable;
    }
    if (group.isAnswered) {
      return _resumeAnsweredQuestion(selectedConversation, group);
    }
    final service = widget.serviceClient;
    if (service == null) return l10n.userQuestionUnavailable;

    setState(() => _isResumingQuestion = true);
    try {
      final response = await service.call(
        'chat.questions.respond',
        params: <String, Object?>{
          'conversationId': group.conversationId,
          'groupId': group.id,
          'revision': group.revision,
          'answers': answers.map((answer) => answer.toJson()).toList(),
        },
      );
      final executionActive = response['executionActive'];
      final idempotent = response['idempotent'];
      final question = AgentQuestionGroup.fromJson(response['question']);
      if (executionActive is! bool || idempotent is! bool) {
        throw const FormatException('The question response was invalid.');
      }
      if (executionActive) {
        _questionRunRefreshConversations.add(group.conversationId);
        _removeQuestionGroup(group.id);
        return null;
      }
      _replaceQuestionGroup(question);
      if (idempotent) return null;

      await _persistQuestionToolAnswer(question, answers, finishMessage: true);
      await _sendMessage(selectedConversation, resumeRunId: question.runId);
      await _loadPendingQuestionGroups(group.conversationId);
      return null;
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'user_questions',
          context: ErrorDescription('while saving and resuming an AI question'),
        ),
      );
      await _loadPendingQuestionGroups(group.conversationId);
      return l10n.userQuestionSubmitFailed;
    } finally {
      if (mounted) setState(() => _isResumingQuestion = false);
    }
  }

  Future<String?> _resumeAnsweredQuestion(
    ChatConversation selectedConversation,
    AgentQuestionGroup group,
  ) async {
    final l10n = context.openchatL10n;
    setState(() => _isResumingQuestion = true);
    try {
      await _persistQuestionToolAnswer(
        group,
        group.savedAnswers,
        finishMessage: true,
      );
      await _sendMessage(selectedConversation, resumeRunId: group.runId);
      await _loadPendingQuestionGroups(group.conversationId);
      return null;
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'user_questions',
          context: ErrorDescription('while resuming a saved AI question'),
        ),
      );
      await _loadPendingQuestionGroups(group.conversationId);
      return l10n.userQuestionResumeFailed;
    } finally {
      if (mounted) setState(() => _isResumingQuestion = false);
    }
  }

  Future<void> _persistQuestionToolAnswer(
    AgentQuestionGroup group,
    List<AgentQuestionAnswer> answers, {
    required bool finishMessage,
  }) async {
    final repository = widget.chatRepository;
    final assistantMessageId = group.assistantMessageId;
    final toolCallId = group.toolCallId;
    final toolName = group.toolName;
    final arguments = group.toolArguments;
    if (repository == null ||
        assistantMessageId == null ||
        toolCallId == null ||
        toolName == null ||
        arguments == null) {
      throw const FormatException(
        'The saved question continuation was incomplete.',
      );
    }
    final messages = await repository.getMessages(group.conversationId);
    chat.ChatMessage? message;
    for (final candidate in messages) {
      if (candidate.id == assistantMessageId) {
        message = candidate;
        break;
      }
    }
    if (message == null || message.role != chat.ChatMessageRole.assistant) {
      throw const FormatException(
        'The assistant message for this question is missing.',
      );
    }
    final output = <String, Object?>{
      'answers': answers.map((answer) => answer.toJson()).toList(),
    };
    final activities = List<chat.ChatToolActivity>.of(message.toolActivities);
    final activityIndex = activities.indexWhere(
      (activity) => activity.callId == toolCallId,
    );
    final completed = chat.ChatToolActivity(
      callId: toolCallId,
      name: toolName,
      arguments: arguments,
      roundId: activityIndex < 0 ? null : activities[activityIndex].roundId,
      assistantTextBeforeByteOffset: activityIndex < 0
          ? null
          : activities[activityIndex].assistantTextBeforeByteOffset,
      output: output,
      targetPath: null,
      status: chat.ChatToolActivityStatus.completed,
    );
    if (activityIndex < 0) {
      activities.add(completed);
    } else {
      activities[activityIndex] = completed;
    }
    await repository.saveMessage(
      conversationId: group.conversationId,
      message: chat.ChatMessage(
        id: message.id,
        role: message.role,
        content: message.content,
        attachments: message.attachments,
        createdAt: message.createdAt,
        outputTokens: message.outputTokens,
        tokensPerSecond: message.tokensPerSecond,
        elapsed: message.elapsed,
        reasoningSummaries: message.reasoningSummaries,
        toolActivities: List<chat.ChatToolActivity>.unmodifiable(activities),
        status: finishMessage && !_isSending
            ? chat.ChatMessageStatus.completed
            : message.status,
        failureCode: message.failureCode,
      ),
    );
  }

  void _replaceQuestionGroup(AgentQuestionGroup group) {
    if (!mounted || _selectedConversationId != group.conversationId) return;
    _questionListGeneration++;
    setState(() {
      _pendingQuestionGroups = <AgentQuestionGroup>[
        for (final current in _pendingQuestionGroups)
          if (current.id == group.id) group else current,
      ];
    });
  }

  void _removeQuestionGroup(String groupId) {
    if (!mounted) return;
    _questionListGeneration++;
    setState(() {
      _pendingQuestionGroups = _pendingQuestionGroups
          .where((group) => group.id != groupId)
          .toList(growable: false);
    });
  }

  Future<void> _loadProviderState({bool forceRefresh = false}) async {
    final service = widget.serviceClient;
    if (service == null) return;
    if (_isLoadingConnections) {
      _providerStateReloadRequested = true;
      _forceProviderStateReloadRequested |= forceRefresh;
      return;
    }
    setState(() {
      _isLoadingConnections = true;
      _modelsLoaded = false;
    });
    try {
      ChatGptConnection? selectedConnection;
      ChatGptWorkspace? selectedWorkspace;
      try {
        final response = await service.call('chatgpt.connections.list');
        final rawConnections = response['connections'];
        if (rawConnections is! List<Object?>) {
          throw const FormatException(
            'The ChatGPT connection list was invalid.',
          );
        }
        final connections = rawConnections
            .map((value) => ChatGptConnection.fromJson(_objectMap(value)))
            .toList(growable: false);
        selectedConnection = _firstOrNull(
          connections.where(
            (connection) =>
                connection.isSelected && connection.authStatus == 'active',
          ),
        );
        selectedWorkspace = _firstOrNull(
          selectedConnection?.workspaces.where(
                (workspace) => workspace.isSelected,
              ) ??
              const <ChatGptWorkspace>[],
        );
      } on OpenChatServiceException catch (error) {
        _showServiceFailure(error);
      }
      List<ChatGptApiKeyConnection> apiConnections =
          const <ChatGptApiKeyConnection>[];
      var compatibleProviderIds = const <String>{};
      var apiKeyStorageUnavailable = false;
      var compatibleKeyStorageUnavailable = false;
      try {
        apiConnections =
            await widget.chatGptApiKeyStore?.readConnections() ??
            const <ChatGptApiKeyConnection>[];
      } on ChatGptApiKeyStorageException {
        apiKeyStorageUnavailable = true;
      }
      try {
        compatibleProviderIds =
            await widget.apiCompatibleProviderKeyStore
                ?.readConfiguredProviderIds() ??
            const <String>{};
      } on ApiCompatibleProviderKeyStorageException {
        compatibleKeyStorageUnavailable = true;
      }
      final hiddenKeys = await _settingsPreferences.readHiddenModelKeys();
      final defaultModel = await _settingsPreferences.readDefaultModel();
      if (!mounted) return;
      setState(() {
        _selectedConnectionId = selectedConnection?.id;
        _selectedWorkspaceId = selectedWorkspace?.id;
        _chatGptApiKeyConnections = apiConnections;
        _availableCompatibleProviderIds = compatibleProviderIds;
        _hiddenModelKeys = hiddenKeys;
        _defaultModelPreference = defaultModel;
        _isChatGptConnected =
            (selectedConnection != null && selectedWorkspace != null) ||
            apiConnections.isNotEmpty;
        _isChatGptAvailable = false;
        _isChatGptOAuthAvailable = false;
        _availableChatGptApiKeyConnectionIds = const <String>{};
      });
      if (apiKeyStorageUnavailable) {
        _showMessage(context.openchatL10n.providerDataUnavailable);
      }
      if (compatibleKeyStorageUnavailable) {
        _showMessage(context.openchatL10n.providerKeyStorageFailed);
      }
      final currentId = _selectedConversationId;
      if (currentId != null) {
        await _loadConversationModels(currentId, forceRefresh: forceRefresh);
      } else {
        final hasChatGptConnection =
            (selectedConnection != null && selectedWorkspace != null) ||
            apiConnections.isNotEmpty;
        if (hasChatGptConnection) {
          final chatGptRoute =
              selectedConnection != null && selectedWorkspace != null
              ? 'chatgpt'
              : 'chatgpt_api';
          await _loadModels(
            providerId: chatGptRoute,
            selectedModelId: null,
            selectedModelRouteKey: null,
            forceRefresh: forceRefresh,
          );
        }
        var providerId = _hasUserSelectedProvider
            ? _selectedProviderId
            : _preferredProviderId();
        var defaultModelAvailable = false;
        if (!_hasUserSelectedProvider && defaultModel != null) {
          final defaultProvider = defaultModel.providerId;
          final isDefaultFamilyChatGpt =
              _providerFamily(defaultProvider) == 'chatgpt';
          final isDefaultCompatible = ApiCompatibleProviderKeyStore.providerIds
              .contains(defaultProvider);
          if (defaultProvider == 'opencode') {
            defaultModelAvailable = true;
          } else if (isDefaultFamilyChatGpt && _isChatGptConnected) {
            defaultModelAvailable = true;
          } else if (isDefaultCompatible &&
              compatibleProviderIds.contains(defaultProvider)) {
            defaultModelAvailable = true;
          }
          if (defaultModelAvailable) {
            providerId = defaultProvider;
          }
        }
        if (_providerFamily(providerId) == 'chatgpt') {
          final selectedApiKeyIsAvailable = _availableChatGptApiKeyConnectionIds
              .contains(_selectedApiKeyConnectionId);
          final selectedRouteIsAvailable = providerId == 'chatgpt_api'
              ? selectedApiKeyIsAvailable
              : _isChatGptOAuthAvailable;
          if (!selectedRouteIsAvailable) {
            providerId = _preferredProviderId();
          }
          if (_providerFamily(providerId) == 'chatgpt' &&
              !_isChatGptAvailable) {
            providerId = 'opencode';
          }
        }
        if (ApiCompatibleProviderKeyStore.providerIds.contains(providerId) &&
            !compatibleProviderIds.contains(providerId)) {
          providerId = 'opencode';
        }
        if (providerId == 'chatgpt_api' &&
            !_availableChatGptApiKeyConnectionIds.contains(
              _selectedApiKeyConnectionId,
            )) {
          _selectedApiKeyConnectionId = _firstOrNull(
            _availableChatGptApiKeyConnectionIds,
          );
        }
        setState(() {
          _selectedProviderId = providerId;
          if (defaultModelAvailable && defaultModel != null) {
            if (defaultModel.providerId == 'chatgpt') {
              _selectedConnectionId = defaultModel.connectionId;
              _selectedWorkspaceId = defaultModel.workspaceId;
            }
            _selectedApiKeyConnectionId = defaultModel.apiKeyConnectionId;
          } else {
            if (!_isApiKeyRouteProvider(providerId)) {
              _selectedApiKeyConnectionId = null;
            } else if (ApiCompatibleProviderKeyStore.providerIds.contains(
              providerId,
            )) {
              _selectedApiKeyConnectionId = providerId;
            }
          }
        });
        await _loadModels(
          providerId: providerId,
          selectedModelId: defaultModelAvailable ? defaultModel?.modelId : null,
          selectedModelRouteKey: defaultModelAvailable
              ? defaultModel?.routeKey
              : null,
          forceRefresh: forceRefresh,
        );
      }
    } on ChatGptApiKeyStorageException {
      if (mounted) _showMessage(context.openchatL10n.providerDataUnavailable);
    } on OpenChatServiceException catch (error) {
      _showServiceFailure(error);
    } on FormatException {
      if (mounted) _showMessage(context.openchatL10n.providerDataUnavailable);
    } finally {
      if (mounted) {
        setState(() => _isLoadingConnections = false);
        final shouldReload = _providerStateReloadRequested;
        final shouldForceRefresh = _forceProviderStateReloadRequested;
        _providerStateReloadRequested = false;
        _forceProviderStateReloadRequested = false;
        if (shouldReload) {
          unawaited(_loadProviderState(forceRefresh: shouldForceRefresh));
        }
      }
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
      final providerId = switch (conversation?.providerId) {
        'opencode' => 'opencode',
        'chatgpt_api' => 'chatgpt_api',
        'chatgpt' => 'chatgpt',
        'gemini' => 'gemini',
        'groq' => 'groq',
        'cerebras' => 'cerebras',
        'openrouter' => 'openrouter',
        'mistral' => 'mistral',
        _ =>
          conversation?.connectionId != null
              ? 'chatgpt'
              : _preferredProviderId(),
      };
      final connectionId = conversation?.connectionId;
      final workspaceId = conversation?.workspaceId;
      final apiKeyConnectionId = conversation?.apiKeyConnectionId;
      if ((providerId == 'chatgpt' &&
              (connectionId == null || workspaceId == null)) ||
          (providerId == 'chatgpt_api' && apiKeyConnectionId == null) ||
          (ApiCompatibleProviderKeyStore.providerIds.contains(providerId) &&
              apiKeyConnectionId != providerId)) {
        throw StateError('The conversation provider route was incomplete.');
      }
      setState(() {
        _selectedProviderId = providerId;
        if (providerId == 'chatgpt') {
          _selectedConnectionId = connectionId;
          _selectedWorkspaceId = workspaceId;
        }
        _selectedApiKeyConnectionId = apiKeyConnectionId;
      });
      if (providerId == 'chatgpt_api' && _chatGptApiKeyConnections.isEmpty) {
        final apiConnections =
            await widget.chatGptApiKeyStore?.readConnections() ??
            const <ChatGptApiKeyConnection>[];
        if (!mounted || _selectedConversationId != conversationId) return;
        setState(() => _chatGptApiKeyConnections = apiConnections);
      }
      await _loadModels(
        providerId: providerId,
        selectedModelId: conversation?.modelId,
        selectedModelRouteKey: conversation == null
            ? null
            : _routeKey(
                providerId,
                modelId: conversation.modelId,
                connectionId: connectionId,
                workspaceId: workspaceId,
                apiKeyConnectionId: apiKeyConnectionId,
              ),
        forceRefresh: forceRefresh,
      );
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

  Future<void> _loadModels({
    required String providerId,
    required String? selectedModelId,
    required String? selectedModelRouteKey,
    bool forceRefresh = false,
  }) async {
    final service = widget.serviceClient;
    if (service == null) return;
    final providerFamily = _providerFamily(providerId);
    final sameRoute = _loadedProviderId == providerFamily && _modelsLoaded;
    if (!forceRefresh && sameRoute && _modelsLoaded) {
      ChatGptModel? selectedModel;
      for (final model in _models) {
        if (model.id == selectedModelId &&
            model.isAvailable &&
            (selectedModelRouteKey == null ||
                model.routeKey == selectedModelRouteKey)) {
          selectedModel = model;
          break;
        }
      }
      setState(() {
        _selectedModelId = selectedModel?.id;
        _selectedReasoningEffort = selectedModel?.defaultReasoningLevel;
        if (selectedModel != null) {
          _selectedProviderId = selectedModel.providerId;
          if (selectedModel.providerId == 'chatgpt') {
            _selectedConnectionId = selectedModel.connectionId;
            _selectedWorkspaceId = selectedModel.workspaceId;
          }
          _selectedApiKeyConnectionId =
              selectedModel.providerId == 'chatgpt_api'
              ? selectedModel.connectionId
              : ApiCompatibleProviderKeyStore.providerIds.contains(
                  selectedModel.providerId,
                )
              ? selectedModel.providerId
              : null;
        }
      });
      return;
    }
    final generation = ++_modelLoadGeneration;
    setState(() {
      _isLoadingModels = true;
      _modelLoadError = null;
      _modelFreshness = 'unavailable';
      _models = const <ChatGptModel>[];
      _modelsLoaded = false;
      _selectedModelId = selectedModelId;
      _selectedReasoningEffort = null;
      _loadedConnectionId = null;
      _loadedWorkspaceId = null;
      _loadedApiKeyConnectionIds = const <String>{};
      _loadedProviderId = providerFamily;
    });
    try {
      final models = <ChatGptModel>[];
      final loadedApiKeys = <String>{};
      var isStale = false;
      var sourceFailure = false;
      OpenChatServiceException? sourceServiceError;

      ({List<ChatGptModel> models, bool stale}) parseCatalog(
        Map<String, Object?> response, {
        required String routeProviderId,
        required String groupId,
        String? localEngineId,
        String? connectionId,
        String? workspaceId,
        String? sourceLabel,
      }) {
        final rawModels = response['models'];
        final freshness = response['freshness'];
        if (rawModels is! List<Object?> ||
            (freshness != 'current' && freshness != 'stale')) {
          throw const FormatException(
            'The provider model catalog was invalid.',
          );
        }
        final scopedModels = localEngineId == null
            ? rawModels
            : rawModels
                  .where(
                    (value) => _objectMap(value)['engineId'] == localEngineId,
                  )
                  .toList(growable: false);
        return (
          models: scopedModels
              .map((value) => ChatGptModel.fromJson(_objectMap(value)))
              .map(
                (model) => model.withRoute(
                  providerId: routeProviderId,
                  connectionId: connectionId,
                  workspaceId: workspaceId,
                  sourceLabel: sourceLabel,
                  groupId: model.groupId ?? groupId,
                ),
              )
              .toList(growable: false),
          stale: freshness == 'stale',
        );
      }

      if (_localEngineProviderIds.contains(providerFamily)) {
        final response = await service.call(
          'local.engines.chat_models.list',
          params: <String, Object?>{'engineId': providerFamily},
        );
        final catalog = parseCatalog(
          response,
          routeProviderId: providerFamily,
          groupId: 'models',
          localEngineId: providerFamily,
        );
        models.addAll(catalog.models);
      } else if (providerFamily == 'opencode') {
        final apiKey = await widget.openCodeApiKeyStore?.readApiKey();
        final response = await service.call(
          'opencode.models.list',
          params: <String, Object?>{
            ...?switch (apiKey) {
              final value? => <String, Object?>{'apiKey': value},
              _ => null,
            },
            if (forceRefresh) 'forceRefresh': true,
          },
        );
        final catalog = parseCatalog(
          response,
          routeProviderId: 'opencode',
          groupId: 'models',
        );
        models.addAll(catalog.models);
        isStale = catalog.stale;
      } else if (providerFamily == 'chatgpt') {
        final oauthConnectionId = _selectedConnectionId;
        final oauthWorkspaceId = _selectedWorkspaceId;
        if (oauthConnectionId != null && oauthWorkspaceId != null) {
          try {
            final response = await service.call(
              'chatgpt.models.list',
              params: <String, Object?>{
                'connectionId': oauthConnectionId,
                'workspaceId': oauthWorkspaceId,
                if (forceRefresh) 'forceRefresh': true,
              },
            );
            final catalog = parseCatalog(
              response,
              routeProviderId: 'chatgpt',
              groupId: 'oauth',
              connectionId: oauthConnectionId,
              workspaceId: oauthWorkspaceId,
            );
            models.addAll(catalog.models);
            isStale = isStale || catalog.stale;
          } on OpenChatServiceException catch (error) {
            sourceFailure = true;
            sourceServiceError ??= error;
          }
        }

        final apiConnections = _chatGptApiKeyConnections;
        for (final apiConnection in apiConnections) {
          try {
            final apiKey = await widget.chatGptApiKeyStore?.readApiKey(
              apiConnection.id,
            );
            if (apiKey == null) continue;
            final response = await service.call(
              'chatgpt.api.models.list',
              params: <String, Object?>{
                'apiKey': apiKey,
                if (forceRefresh) 'forceRefresh': true,
              },
            );
            final catalog = parseCatalog(
              response,
              routeProviderId: 'chatgpt_api',
              groupId: 'api',
              connectionId: apiConnection.id,
              sourceLabel:
                  apiConnections.length > 1 && apiConnection.keySuffix != null
                  ? '••••${apiConnection.keySuffix}'
                  : null,
            );
            models.addAll(catalog.models);
            loadedApiKeys.add(apiConnection.id);
            isStale = isStale || catalog.stale;
          } on ChatGptApiKeyStorageException {
            sourceFailure = true;
          } on OpenChatServiceException catch (error) {
            sourceFailure = true;
            sourceServiceError ??= error;
          }
        }
      } else if (ApiCompatibleProviderKeyStore.providerIds.contains(
        providerFamily,
      )) {
        try {
          final apiKey = await widget.apiCompatibleProviderKeyStore?.readApiKey(
            providerFamily,
          );
          if (apiKey == null) {
            sourceFailure = true;
          } else {
            final response = await service.call(
              'compatible.models.list',
              params: <String, Object?>{
                'providerId': providerFamily,
                'apiKey': apiKey,
                if (forceRefresh) 'forceRefresh': true,
              },
            );
            final catalog = parseCatalog(
              response,
              routeProviderId: providerFamily,
              groupId: 'models',
              connectionId: providerFamily,
            );
            models.addAll(catalog.models);
            loadedApiKeys.add(providerFamily);
            isStale = catalog.stale;
          }
        } on ApiCompatibleProviderKeyStorageException {
          sourceFailure = true;
        } on OpenChatServiceException catch (error) {
          sourceFailure = true;
          sourceServiceError ??= error;
        }
      }
      if (!mounted || generation != _modelLoadGeneration) return;
      final availableModels = models.where((model) => model.isAvailable);
      final selectedModel = _firstOrNull(
        availableModels.where(
          (model) =>
              model.id == selectedModelId &&
              (selectedModelRouteKey == null ||
                  model.routeKey == selectedModelRouteKey),
        ),
      );
      setState(() {
        _models = List<ChatGptModel>.unmodifiable(models);
        _modelsLoaded = true;
        _modelFreshness = isStale ? 'stale' : 'current';
        _loadedConnectionId = providerFamily == 'chatgpt'
            ? _selectedConnectionId
            : null;
        _loadedWorkspaceId = providerFamily == 'chatgpt'
            ? _selectedWorkspaceId
            : null;
        _loadedApiKeyConnectionIds = Set<String>.unmodifiable(loadedApiKeys);
        if (providerFamily == 'chatgpt') {
          _isChatGptOAuthAvailable = models.any(
            (model) => model.providerId == 'chatgpt' && model.isAvailable,
          );
          _availableChatGptApiKeyConnectionIds = Set<String>.unmodifiable(
            models
                .where(
                  (model) =>
                      model.providerId == 'chatgpt_api' && model.isAvailable,
                )
                .map((model) => model.connectionId)
                .whereType<String>(),
          );
          _isChatGptAvailable =
              _isChatGptOAuthAvailable ||
              _availableChatGptApiKeyConnectionIds.isNotEmpty;
        }
        _selectedModelId = selectedModel?.id;
        _selectedReasoningEffort = selectedModel?.defaultReasoningLevel;
        if (selectedModel != null) {
          _selectedProviderId = selectedModel.providerId;
          if (selectedModel.providerId == 'chatgpt') {
            _selectedConnectionId = selectedModel.connectionId;
            _selectedWorkspaceId = selectedModel.workspaceId;
          }
          _selectedApiKeyConnectionId =
              selectedModel.providerId == 'chatgpt_api'
              ? selectedModel.connectionId
              : ApiCompatibleProviderKeyStore.providerIds.contains(
                  selectedModel.providerId,
                )
              ? selectedModel.providerId
              : null;
        }
        _modelLoadError = models.any((model) => model.isAvailable)
            ? null
            : sourceFailure
            ? 'unavailable'
            : 'empty';
      });
      if (sourceFailure && models.isNotEmpty) {
        _showMessage(context.openchatL10n.providerDataUnavailable);
      } else if (sourceFailure && sourceServiceError != null) {
        _showServiceFailure(sourceServiceError);
      } else if (sourceFailure) {
        _showMessage(context.openchatL10n.providerDataUnavailable);
      }
    } on OpenCodeApiKeyStorageException {
      if (!mounted || generation != _modelLoadGeneration) return;
      setState(() {
        _modelLoadError = 'key_storage';
        _models = const <ChatGptModel>[];
        _modelsLoaded = false;
      });
      _showMessage(context.openchatL10n.openCodeKeyStorageFailed);
    } on ChatGptApiKeyStorageException {
      if (!mounted || generation != _modelLoadGeneration) return;
      setState(() {
        _modelLoadError = 'key_storage';
        _models = const <ChatGptModel>[];
        _modelsLoaded = false;
        _isChatGptAvailable = false;
      });
      _showMessage(context.openchatL10n.providerDataUnavailable);
    } on ApiCompatibleProviderKeyStorageException {
      if (!mounted || generation != _modelLoadGeneration) return;
      setState(() {
        _modelLoadError = 'key_storage';
        _models = const <ChatGptModel>[];
        _modelsLoaded = false;
      });
      _showMessage(context.openchatL10n.providerKeyStorageFailed);
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

  String _providerFamily(String providerId) =>
      providerId == 'chatgpt_api' ? 'chatgpt' : providerId;

  String _preferredProviderId() {
    if (!_isChatGptAvailable) return 'opencode';
    return _isChatGptOAuthAvailable ? 'chatgpt' : 'chatgpt_api';
  }

  String _routeKey(
    String providerId, {
    required String? modelId,
    String? connectionId,
    String? workspaceId,
    String? apiKeyConnectionId,
  }) =>
      '$providerId:${providerId == 'chatgpt_api'
          ? apiKeyConnectionId ?? ''
          : ApiCompatibleProviderKeyStore.providerIds.contains(providerId)
          ? apiKeyConnectionId ?? providerId
          : providerId == 'chatgpt'
          ? connectionId ?? ''
          : ''}:${providerId == 'chatgpt' ? workspaceId ?? '' : ''}:${modelId ?? ''}';

  void _selectModel(ChatGptModel selectedModel) {
    if (!selectedModel.isAvailable) return;
    final conversationId = _selectedConversationId;
    final generation = ++_modelSelectionGeneration;
    final previousModelId = _selectedModelId;
    final previousReasoningEffort = _selectedReasoningEffort;
    final previousProviderId = _selectedProviderId;
    final previousConnectionId = _selectedConnectionId;
    final previousWorkspaceId = _selectedWorkspaceId;
    final previousApiKeyConnectionId = _selectedApiKeyConnectionId;
    setState(() {
      _selectedModelId = selectedModel.id;
      _selectedReasoningEffort = selectedModel.defaultReasoningLevel;
      _selectedProviderId = selectedModel.providerId;
      if (selectedModel.providerId == 'chatgpt') {
        _selectedConnectionId = selectedModel.connectionId;
        _selectedWorkspaceId = selectedModel.workspaceId;
      }
      _selectedApiKeyConnectionId = selectedModel.providerId == 'chatgpt_api'
          ? selectedModel.connectionId
          : ApiCompatibleProviderKeyStore.providerIds.contains(
              selectedModel.providerId,
            )
          ? selectedModel.providerId
          : null;
      _isUpdatingConversationModel = conversationId != null;
    });
    if (conversationId != null) {
      unawaited(
        _persistConversationModel(
          conversationId: conversationId,
          model: selectedModel,
          previousModelId: previousModelId,
          previousReasoningEffort: previousReasoningEffort,
          previousProviderId: previousProviderId,
          previousConnectionId: previousConnectionId,
          previousWorkspaceId: previousWorkspaceId,
          previousApiKeyConnectionId: previousApiKeyConnectionId,
          generation: generation,
        ),
      );
    }
  }

  Future<void> _setModelFavorite({
    required String providerId,
    required String modelId,
    required String displayName,
    String? sourceConnectionId,
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
        sourceConnectionId: sourceConnectionId,
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
    final favoriteFamily = _providerFamily(favorite.providerId);

    if (targetConversationId != null) {
      final conversation = await widget.chatRepository?.getConversation(
        targetConversationId,
      );
      if (!mounted || _selectedConversationId != targetConversationId) return;
      if (conversation == null ||
          _providerFamily(
                conversation.providerId ??
                    (conversation.connectionId == null
                        ? 'opencode'
                        : 'chatgpt'),
              ) !=
              favoriteFamily) {
        return;
      }
    }

    if (favorite.providerId == 'chatgpt_api' &&
        (favorite.sourceConnectionId == null ||
            !_chatGptApiKeyConnections.any(
              (key) => key.id == favorite.sourceConnectionId,
            ))) {
      _showMessage(context.openchatL10n.modelCatalogUnavailable);
      return;
    }
    if (favorite.providerId == 'chatgpt' &&
        (_selectedConnectionId == null || _selectedWorkspaceId == null)) {
      _showMessage(context.openchatL10n.modelCatalogUnavailable);
      return;
    }
    if (ApiCompatibleProviderKeyStore.providerIds.contains(
          favorite.providerId,
        ) &&
        !_availableCompatibleProviderIds.contains(favorite.providerId)) {
      _showMessage(context.openchatL10n.modelCatalogUnavailable);
      return;
    }
    if (favorite.providerId != 'chatgpt' &&
        favorite.providerId != 'chatgpt_api' &&
        favorite.providerId != 'opencode' &&
        !_localEngineProviderIds.contains(favorite.providerId) &&
        !ApiCompatibleProviderKeyStore.providerIds.contains(
          favorite.providerId,
        )) {
      return;
    }

    if (_selectedProviderId != favorite.providerId) {
      setState(() {
        _selectedProviderId = favorite.providerId;
        _selectedApiKeyConnectionId = favorite.providerId == 'chatgpt_api'
            ? favorite.sourceConnectionId
            : ApiCompatibleProviderKeyStore.providerIds.contains(
                favorite.providerId,
              )
            ? favorite.providerId
            : null;
        _selectedModelId = null;
        _selectedReasoningEffort = null;
      });
    }
    await _loadModels(
      providerId: favorite.providerId,
      selectedModelId: null,
      selectedModelRouteKey: null,
    );
    if (!mounted ||
        _selectedConversationId != targetConversationId ||
        _providerFamily(_selectedProviderId) != favoriteFamily ||
        _modelLoadError != null) {
      return;
    }
    final model = _firstOrNull(
      _models.where(
        (candidate) =>
            candidate.id == favorite.modelId &&
            candidate.providerId == favorite.providerId &&
            candidate.isAvailable &&
            (!_isApiKeyRouteProvider(favorite.providerId) ||
                candidate.connectionId == favorite.sourceConnectionId),
      ),
    );
    if (model == null) {
      _showMessage(context.openchatL10n.modelCatalogUnavailable);
      return;
    }
    _selectModel(model);
  }

  void _selectProvider(String providerId) {
    final isCompatibleProvider = ApiCompatibleProviderKeyStore.providerIds
        .contains(providerId);
    if ((providerId != 'chatgpt' &&
            providerId != 'opencode' &&
            !_localEngineProviderIds.contains(providerId) &&
            !isCompatibleProvider) ||
        (providerId == 'chatgpt' && !_isChatGptConnected) ||
        (isCompatibleProvider &&
            !_availableCompatibleProviderIds.contains(providerId))) {
      return;
    }
    _hasUserSelectedProvider = true;
    final routeProviderId = providerId == 'opencode'
        ? 'opencode'
        : providerId != 'chatgpt'
        ? providerId
        : _selectedConnectionId != null && _selectedWorkspaceId != null
        ? 'chatgpt'
        : 'chatgpt_api';
    setState(() {
      _selectedProviderId = routeProviderId;
      _selectedApiKeyConnectionId = routeProviderId == 'chatgpt_api'
          ? _firstOrNull(_chatGptApiKeyConnections)?.id
          : isCompatibleProvider
          ? providerId
          : null;
      _selectedModelId = null;
      _selectedReasoningEffort = null;
    });
    unawaited(
      _loadModels(
        providerId: routeProviderId,
        selectedModelId: null,
        selectedModelRouteKey: null,
      ),
    );
  }

  Future<void> _persistConversationModel({
    required String conversationId,
    required ChatGptModel model,
    required String? previousModelId,
    required String? previousReasoningEffort,
    required String previousProviderId,
    required String? previousConnectionId,
    required String? previousWorkspaceId,
    required String? previousApiKeyConnectionId,
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
      await repository.setConversationRoute(
        conversationId: conversationId,
        providerId: model.providerId,
        modelId: model.id,
        connectionId: model.providerId == 'chatgpt' ? model.connectionId : null,
        workspaceId: model.providerId == 'chatgpt' ? model.workspaceId : null,
        apiKeyConnectionId: model.providerId == 'chatgpt_api'
            ? model.connectionId
            : ApiCompatibleProviderKeyStore.providerIds.contains(
                model.providerId,
              )
            ? model.providerId
            : null,
      );
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
          _selectedProviderId = previousProviderId;
          _selectedConnectionId = previousConnectionId;
          _selectedWorkspaceId = previousWorkspaceId;
          _selectedApiKeyConnectionId = previousApiKeyConnectionId;
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

  void _selectReasoning(String? effort) {
    setState(() => _selectedReasoningEffort = effort);
  }

  void _handleModelDownloadChanged() {
    final controller = _modelDownloadController;
    final model = controller?.downloadedModel;
    if (controller?.status != HuggingFaceDownloadStatus.completed ||
        model == null ||
        _providerFamily(_selectedProviderId) != model.engineId) {
      return;
    }
    unawaited(
      _loadModels(
        providerId: _selectedProviderId,
        selectedModelId: _selectedModelId,
        selectedModelRouteKey: _selectedModelId == null
            ? null
            : _routeKey(_selectedProviderId, modelId: _selectedModelId),
        forceRefresh: true,
      ),
    );
  }

  void _resetMessageScroll() {
    if (_messageScrollController.hasClients) {
      _messageScrollController.jumpTo(0);
    }
  }

  void _startNewConversation() {
    _modelSelectionGeneration++;
    _goalLoadGeneration++;
    _fileChangesLoadGeneration++;
    _historyTargetRequestId++;
    _resetMessageScroll();
    _hasUserSelectedProvider = false;
    final defaultModel = _defaultModelPreference;
    setState(() {
      _selectedConversationId = null;
      _pendingProjectId = null;
      _activeGoal = null;
      _isFileChangesPanelOpen = false;
      _fileChangesConversationId = null;
      _conversationFileChanges = const <ChatFileChange>[];
      _historyTargetConversationId = null;
      _historyTargetMessageId = null;
      _selectedProviderId = defaultModel != null
          ? defaultModel.providerId
          : _preferredProviderId();
      _selectedApiKeyConnectionId =
          defaultModel?.apiKeyConnectionId ??
          (_selectedProviderId == 'chatgpt_api'
              ? _firstOrNull(_chatGptApiKeyConnections)?.id
              : null);
      _selectedConnectionId = defaultModel?.connectionId;
      _selectedWorkspaceId = defaultModel?.workspaceId;
      _messageStream = null;
      _pendingQuestionGroups = const <AgentQuestionGroup>[];
      _isLoadingPendingQuestions = false;
      _isResumingQuestion = false;
      _questionLoadFailed = false;
      _selectedModelId = defaultModel?.modelId;
      _selectedReasoningEffort = null;
      _isUpdatingConversationModel = false;
      _titleEditRequestId = null;
      _settingsOpen = false;
      _modelsPageOpen = false;
      _localModelsPageOpen = false;
    });
    unawaited(_loadProviderState());
  }

  void _startProjectConversation(String projectId) {
    if (widget.historyStorageStatus != HistoryStorageStatus.available) return;
    _startNewConversation();
    setState(() => _pendingProjectId = projectId);
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
    final fileChangesRemoved = await _deleteFileChangesWithReporting(
      () => _chatFileChangesRepository?.deleteAll() ?? Future<void>.value(),
      contextDescription: 'while clearing all conversations file changes',
    );
    if (!mounted) return;
    _modelSelectionGeneration++;
    _goalLoadGeneration++;
    _historyTargetRequestId++;
    _resetMessageScroll();
    _messageController.clear();
    setState(() {
      _selectedConversationId = null;
      _activeGoal = null;
      _historyTargetConversationId = null;
      _historyTargetMessageId = null;
      _messageStream = null;
      _pendingQuestionGroups = const <AgentQuestionGroup>[];
      _isLoadingPendingQuestions = false;
      _questionLoadFailed = false;
      _isFileChangesPanelOpen = false;
      _fileChangesConversationId = null;
      _conversationFileChanges = const <ChatFileChange>[];
      _isUpdatingConversationModel = false;
      _titleEditRequestId = null;
    });
    if (!fileChangesRemoved) {
      _showMessage(
        context.openchatL10n.conversationHistoryClearedFileChangesCleanupFailed,
      );
    }
  }

  Future<void> _sendMessage(
    ChatConversation? selectedConversation, {
    chat.ChatMessage? responseToReplace,
    String? resumeRunId,
    String? messageTextOverride,
    bool userMessageAlreadySaved = false,
    VoidCallback? onUserMessageSaved,
  }) async {
    if (_isSending ||
        _isCreatingMessageBranch ||
        _isUpdatingConversationModel ||
        _isLoadingChatGptFastMode ||
        !_toolPermissionModeReady ||
        _isLoadingToolPermissionMode ||
        _isSavingToolPermissionMode) {
      return;
    }
    if (resumeRunId == null &&
        (_isLoadingPendingQuestions ||
            _questionLoadFailed ||
            _pendingQuestionGroups.isNotEmpty)) {
      return;
    }
    final toolPermissionMode = _toolPermissionMode;
    final repository = widget.chatRepository;
    final service = widget.serviceClient;
    final l10n = context.openchatL10n;
    final rawText = responseToReplace == null && resumeRunId == null
        ? messageTextOverride ?? _messageController.text.trim()
        : '';
    String? goalObjective;
    if (messageTextOverride == null &&
        responseToReplace == null &&
        resumeRunId == null &&
        RegExp(r'^/goal(?:\s|$)', caseSensitive: false).hasMatch(rawText)) {
      final objective = rawText.substring('/goal'.length).trim();
      if (objective.isEmpty) {
        _showMessage(l10n.goalObjectiveRequired);
        return;
      }
      goalObjective = objective;
    }
    final text = goalObjective ?? rawText;
    final isGoalSend = goalObjective != null;
    final attachments =
        responseToReplace == null &&
            resumeRunId == null &&
            messageTextOverride == null
        ? List<ChatAttachment>.unmodifiable(_pendingAttachments)
        : const <ChatAttachment>[];
    if (repository == null ||
        service == null ||
        (resumeRunId != null && selectedConversation == null) ||
        (responseToReplace == null &&
            resumeRunId == null &&
            !userMessageAlreadySaved &&
            text.isEmpty &&
            attachments.isEmpty)) {
      return;
    }

    final providerId =
        selectedConversation?.providerId ??
        (selectedConversation?.connectionId != null
            ? 'chatgpt'
            : _selectedProviderId);
    final connectionId = providerId == 'chatgpt'
        ? selectedConversation?.connectionId ?? _selectedConnectionId
        : null;
    final workspaceId = providerId == 'chatgpt'
        ? selectedConversation?.workspaceId ?? _selectedWorkspaceId
        : null;
    final apiKeyConnectionId = _isApiKeyRouteProvider(providerId)
        ? selectedConversation?.apiKeyConnectionId ??
              _selectedApiKeyConnectionId ??
              (ApiCompatibleProviderKeyStore.providerIds.contains(providerId)
                  ? providerId
                  : null)
        : null;
    final modelId = _selectedModelId ?? selectedConversation?.modelId;
    if (modelId == null ||
        (providerId == 'chatgpt' &&
            (connectionId == null || workspaceId == null)) ||
        (_isApiKeyRouteProvider(providerId) && apiKeyConnectionId == null)) {
      _showMessage(context.openchatL10n.modelRequired);
      return;
    }

    final modelRouteKey = _routeKey(
      providerId,
      modelId: modelId,
      connectionId: connectionId,
      workspaceId: workspaceId,
      apiKeyConnectionId: apiKeyConnectionId,
    );
    final oauthModelSupportsFastMode =
        providerId == 'chatgpt' &&
        _models.any(
          (model) =>
              model.routeKey == modelRouteKey &&
              model.isAvailable &&
              model.supportsFastMode,
        );
    final chatGptFastMode =
        _chatGptFastModeEnabled &&
        (providerId == 'chatgpt_api' || oauthModelSupportsFastMode);
    if (_loadedProviderId != _providerFamily(providerId) ||
        (providerId == 'chatgpt' &&
            (_loadedConnectionId != connectionId ||
                _loadedWorkspaceId != workspaceId)) ||
        (_isApiKeyRouteProvider(providerId) &&
            !_loadedApiKeyConnectionIds.contains(apiKeyConnectionId)) ||
        !_models.any(
          (model) => model.routeKey == modelRouteKey && model.isAvailable,
        )) {
      _showMessage(l10n.modelCatalogUnavailable);
      return;
    }
    if (attachments.any((attachment) => attachment.isImage) &&
        !_models.any(
          (model) => model.routeKey == modelRouteKey && model.supportsImages,
        )) {
      _showMessage(l10n.modelDoesNotSupportImages);
      return;
    }

    String? apiKey;
    try {
      apiKey = switch (providerId) {
        'opencode' => await widget.openCodeApiKeyStore?.readApiKey(),
        'chatgpt_api' when apiKeyConnectionId != null =>
          await widget.chatGptApiKeyStore?.readApiKey(apiKeyConnectionId),
        final providerId
            when ApiCompatibleProviderKeyStore.providerIds.contains(
              providerId,
            ) =>
          await widget.apiCompatibleProviderKeyStore?.readApiKey(providerId),
        _ => null,
      };
    } on OpenCodeApiKeyStorageException {
      if (!mounted) return;
      _showMessage(l10n.openCodeKeyStorageFailed);
      return;
    } on ChatGptApiKeyStorageException {
      if (!mounted) return;
      _showMessage(l10n.providerDataUnavailable);
      return;
    } on ApiCompatibleProviderKeyStorageException {
      if (!mounted) return;
      _showMessage(l10n.providerKeyStorageFailed);
      return;
    }

    final conversationId = selectedConversation?.id ?? _newLocalId();
    final projectToolRules = <String, Object?>{};
    final projectId = selectedConversation?.projectId ?? _pendingProjectId;
    if (projectId != null) {
      try {
        final savedRules = await _settingsPreferences
            .readProjectToolPermissionRules(projectId);
        for (final entry in savedRules.entries) {
          final serviceValue = entry.value.serviceValue;
          if (serviceValue != null) {
            projectToolRules[entry.key] = serviceValue;
          }
        }
      } on Exception catch (error, stackTrace) {
        FlutterError.reportError(
          FlutterErrorDetails(
            exception: error,
            stack: stackTrace,
            library: 'tool_permissions',
            context: ErrorDescription(
              'while loading project tool permission rules before sending',
            ),
          ),
        );
        if (mounted) _showMessage(l10n.projectToolRulesLoadFailed);
        return;
      }
    }
    final userMessageId = _newLocalId();
    if (mounted) {
      setState(() {
        _isSending = true;
        _activeChatConversationId = conversationId;
        _replacingAssistantMessageId = responseToReplace?.id;
      });
    }

    if (isGoalSend && selectedConversation != null) {
      try {
        final response = await service.call(
          'chat.goal.active',
          params: <String, Object?>{'conversationId': conversationId},
        );
        if (!response.containsKey('goal')) {
          throw const FormatException('The active goal response was invalid.');
        }
        final activeGoal = AgentGoal.fromJson(response['goal']);
        if (activeGoal != null) {
          if (mounted && _selectedConversationId == conversationId) {
            setState(() => _activeGoal = activeGoal);
            _showMessage(l10n.goalAlreadyActive);
          }
          _clearActiveSendState();
          return;
        }
      } on OpenChatServiceException catch (error) {
        if (mounted) _showServiceFailure(error);
        _clearActiveSendState();
        return;
      } on FormatException catch (error, stackTrace) {
        FlutterError.reportError(
          FlutterErrorDetails(
            exception: error,
            stack: stackTrace,
            library: 'chat_goal',
            context: ErrorDescription('while checking for an active goal'),
          ),
        );
        if (mounted) _showMessage(l10n.goalStateUnavailable);
        _clearActiveSendState();
        return;
      }
    }

    late final String sharedInstructions;
    try {
      sharedInstructions = await _settingsPreferences.readSharedInstructions();
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
      }
      _clearActiveSendState();
      return;
    }

    if (responseToReplace != null) {
      if (selectedConversation == null) {
        _clearActiveSendState();
        return;
      }
      try {
        final messages = await repository.getMessages(conversationId);
        if (!mounted) return;
        if (_selectedConversationId != conversationId) {
          _clearActiveSendState();
          return;
        }
        if (messages.length < 2 ||
            messages.last.id != responseToReplace.id ||
            messages.last.role != chat.ChatMessageRole.assistant ||
            messages.last.status == chat.ChatMessageStatus.streaming ||
            messages[messages.length - 2].role != chat.ChatMessageRole.user) {
          _showMessage(l10n.responseRetryUnavailable);
          _clearActiveSendState();
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
        if (mounted) _showMessage(l10n.responseRetryUnavailable);
        _clearActiveSendState();
        return;
      }
    }

    try {
      if (responseToReplace == null &&
          resumeRunId == null &&
          selectedConversation == null) {
        await repository.createConversation(
          id: conversationId,
          title: l10n.conversationTitle,
          createdAt: DateTime.now().toUtc(),
          providerId: providerId,
          connectionId: connectionId,
          workspaceId: workspaceId,
          apiKeyConnectionId: apiKeyConnectionId,
          modelId: modelId,
          projectId: _pendingProjectId,
        );
        if (mounted) {
          setState(() {
            _selectedConversationId = conversationId;
            _pendingProjectId = null;
            _messageStream = repository.watchMessages(conversationId);
          });
        }
      }
      if (responseToReplace == null &&
          resumeRunId == null &&
          !userMessageAlreadySaved) {
        await repository.saveMessage(
          conversationId: conversationId,
          message: chat.ChatMessage(
            id: userMessageId,
            role: chat.ChatMessageRole.user,
            content: text,
            attachments: attachments,
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
      if (mounted) {
        _showMessage(
          error is ChatAttachmentStorageException
              ? l10n.attachmentSaveFailed
              : l10n.messageSaveFailed,
        );
      }
      _clearActiveSendState();
      return;
    }

    if (responseToReplace == null && resumeRunId == null) {
      if (messageTextOverride == null) {
        _messageController.clear();
        if (mounted) setState(_pendingAttachments.clear);
      }
      onUserMessageSaved?.call();
    }
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
    var assistantCitationSources = const <chat.ChatCitationSource>[];
    Stopwatch? streamStopwatch;
    var streamChunkCount = 0;
    int? liveOutputTokens;
    double? liveTokensPerSecond;
    final streamMessagePersister = ChatStreamMessagePersister(
      write: (message) => repository.saveMessage(
        conversationId: conversationId,
        message: message,
        updateConversationTimestamp: false,
      ),
    );
    StreamSubscription<OpenChatServiceEvent>? subscription;
    OpenChatServiceOperation? operation;
    var goalStateMayHaveChanged = false;

    Future<void> flushFailedStreamPersistence() async {
      try {
        await streamMessagePersister.flush();
      } on Object catch (error, stackTrace) {
        FlutterError.reportError(
          FlutterErrorDetails(
            exception: error,
            stack: stackTrace,
            library: 'chat_history',
            context: ErrorDescription(
              'while flushing streamed assistant message updates',
            ),
          ),
        );
      }
    }

    Future<bool> preserveFailedResponse({
      String failureCode = 'chat_request_failed',
    }) async {
      if (responseToReplace != null) {
        final replacementId = assistantMessageId;
        return replacementId == null
            ? true
            : _discardReplacementAttempt(
                repository,
                conversationId,
                replacementId,
              );
      }

      final failedMessageId = assistantMessageId ?? _newLocalId();
      final failedMessageCreatedAt =
          assistantCreatedAt ?? DateTime.now().toUtc();
      assistantMessageId = failedMessageId;
      assistantCreatedAt = failedMessageCreatedAt;
      await repository.saveMessage(
        conversationId: conversationId,
        message: chat.ChatMessage(
          id: failedMessageId,
          role: chat.ChatMessageRole.assistant,
          content: assistantContent,
          createdAt: failedMessageCreatedAt,
          providerId: providerId,
          modelId: modelId,
          citationSources: assistantCitationSources,
          reasoningSummaries: assistantReasoningSummaries,
          toolActivities: assistantToolActivities,
          status: chat.ChatMessageStatus.failed,
          failureCode: failureCode,
        ),
      );
      return true;
    }

    Future<bool> preserveStoppedResponse() async {
      if (responseToReplace != null) {
        final replacementId = assistantMessageId;
        return replacementId == null
            ? true
            : _discardReplacementAttempt(
                repository,
                conversationId,
                replacementId,
              );
      }

      final stoppedMessageId = assistantMessageId ?? _newLocalId();
      await repository.saveMessage(
        conversationId: conversationId,
        message: chat.ChatMessage(
          id: stoppedMessageId,
          role: chat.ChatMessageRole.assistant,
          content: assistantContent,
          createdAt: assistantCreatedAt ?? DateTime.now().toUtc(),
          providerId: providerId,
          modelId: modelId,
          citationSources: assistantCitationSources,
          reasoningSummaries: assistantReasoningSummaries,
          toolActivities: assistantToolActivities,
          status: chat.ChatMessageStatus.stopped,
        ),
      );
      return true;
    }

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
        if (activity.fileChanges.isNotEmpty &&
            _selectedConversationId == conversationId) {
          final changesById = <String, ChatFileChange>{
            for (final change in _conversationFileChanges) change.id: change,
            for (final change in activity.fileChanges) change.id: change,
          };
          setState(() {
            _fileChangesConversationId = conversationId;
            _conversationFileChanges = List<ChatFileChange>.unmodifiable(
              changesById.values,
            );
            _fileChangesRevision++;
          });
        }
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

    void updateCitationSources(Object? value) {
      if (value == null) return;
      try {
        assistantCitationSources = chat.ChatCitationSource.listFromJson(value);
      } on FormatException catch (error, stackTrace) {
        FlutterError.reportError(
          FlutterErrorDetails(
            exception: error,
            stack: stackTrace,
            library: 'local_service',
            context: ErrorDescription('while reading citation source records'),
          ),
        );
      }
    }

    try {
      final activeOperation = await service.startOperation(
        'chat.send',
        params: <String, Object?>{
          'conversationId': conversationId,
          if (goalObjective != null) ...<String, Object?>{
            'goal': true,
            'goalObjective': goalObjective,
          },
          ...?switch (resumeRunId) {
            final runId? => <String, Object?>{'resumeRunId': runId},
            _ => null,
          },
          if (sharedInstructions.trim().isNotEmpty)
            'customInstructions': sharedInstructions,
          ...?switch (apiKey) {
            final key? => <String, Object?>{'apiKey': key},
            _ => null,
          },
          if (_isApiKeyRouteProvider(providerId))
            'apiKeyConnectionId': apiKeyConnectionId,
          if (providerId == 'chatgpt' || providerId == 'chatgpt_api')
            'fastMode': chatGptFastMode,
          ...?switch (responseToReplace) {
            final replacement? => <String, Object?>{
              'excludedAssistantMessageId': replacement.id,
            },
            _ => null,
          },
          ...?switch (_selectedReasoningEffort) {
            final effort? => <String, Object?>{'reasoningEffort': effort},
            _ => null,
          },
          'toolPermissionMode': toolPermissionMode.serviceValue,
          if (projectToolRules.isNotEmpty)
            'toolPermissionRules': projectToolRules,
        },
      );
      operation = activeOperation;
      _activeChatOperation = activeOperation;
      subscription = activeOperation.events.listen(
        (event) {
          if (event.name == 'chat.goal.updated') {
            goalStateMayHaveChanged = true;
            try {
              final goal = AgentGoal.fromJson(event.data);
              if (goal?.conversationId == _selectedConversationId && mounted) {
                setState(() {
                  _activeGoal = switch (goal?.status) {
                    AgentGoalStatus.completed ||
                    AgentGoalStatus.cancelled ||
                    AgentGoalStatus.failed => null,
                    _ => goal,
                  };
                });
              }
            } on FormatException catch (error, stackTrace) {
              FlutterError.reportError(
                FlutterErrorDetails(
                  exception: error,
                  stack: stackTrace,
                  library: 'chat_goal',
                  context: ErrorDescription('while reading goal progress'),
                ),
              );
            }
            return;
          }
          if (event.name == 'chat.question.requested') {
            final conversationId = event.data['conversationId'];
            if (conversationId is String) {
              _questionRunRefreshConversations.add(conversationId);
            }
            unawaited(_handleQuestionRequested(event.data));
            return;
          }
          if (event.name == 'chat.tool.permission.requested') {
            _handleToolPermissionRequest(event.data);
            return;
          }
          if (event.name == 'chat.citations.updated') {
            updateCitationSources(event.data['sources']);
          }
          if (event.name != 'chat.started' &&
              event.name != 'chat.delta' &&
              event.name != 'chat.reasoning.delta' &&
              event.name != 'chat.tool.updated' &&
              event.name != 'chat.citations.updated') {
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
          if (event.name == 'chat.delta' ||
              event.name == 'chat.reasoning.delta') {
            streamChunkCount++;
          }

          final hasStartedGenerating =
              content.isNotEmpty || assistantReasoningSummaries.isNotEmpty;
          if (hasStartedGenerating && streamStopwatch == null) {
            streamStopwatch = Stopwatch()..start();
          }

          final serverTokens = (event.data['outputTokens'] as num?)?.toInt();
          final serverTps = (event.data['tokensPerSecond'] as num?)?.toDouble();

          if (serverTokens != null) {
            liveOutputTokens = serverTokens;
          } else if (hasStartedGenerating) {
            var totalChars = content.length;
            for (final summary in assistantReasoningSummaries) {
              totalChars += summary.content.length;
            }
            if (totalChars > 0) {
              liveOutputTokens = math.max(
                streamChunkCount,
                (totalChars / 3.7).ceil(),
              );
            }
          }

          if (serverTps != null) {
            liveTokensPerSecond = serverTps;
          } else if (streamStopwatch != null &&
              liveOutputTokens != null &&
              liveOutputTokens! > 0) {
            final elapsedSeconds =
                streamStopwatch!.elapsedMicroseconds / 1000000.0;
            if (elapsedSeconds >= 0.05) {
              liveTokensPerSecond = liveOutputTokens! / elapsedSeconds;
            }
          }

          final snapshot = chat.ChatMessage(
            id: messageId,
            role: chat.ChatMessageRole.assistant,
            content: content,
            createdAt: assistantCreatedAt,
            outputTokens: liveOutputTokens,
            tokensPerSecond: liveTokensPerSecond,
            providerId: providerId,
            modelId: modelId,
            reasoningSummaries: assistantReasoningSummaries,
            toolActivities: assistantToolActivities,
            citationSources: assistantCitationSources,
            status: chat.ChatMessageStatus.streaming,
          );
          streamMessagePersister.add(snapshot);
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
      final result = await activeOperation.result;
      await activeOperation.eventsDone;
      await streamMessagePersister.flush();
      final resultStatus = result['status'];
      final status = switch (resultStatus) {
        'completed' || 'paused' => chat.ChatMessageStatus.completed,
        'stopped' => chat.ChatMessageStatus.stopped,
        _ => throw const FormatException(
          'The ChatGPT response status was invalid.',
        ),
      };
      final outputTokens = switch (result['outputTokens']) {
        null => liveOutputTokens,
        int value => value,
        _ => throw const FormatException(
          'The ChatGPT response metrics were invalid.',
        ),
      };
      final elapsedMicroseconds = switch (result['elapsedMicroseconds']) {
        null => streamStopwatch?.elapsedMicroseconds,
        int value => value,
        _ => throw const FormatException(
          'The ChatGPT response metrics were invalid.',
        ),
      };
      final tokensPerSecond = switch (result['tokensPerSecond']) {
        null =>
          (outputTokens != null &&
                  elapsedMicroseconds != null &&
                  elapsedMicroseconds > 0)
              ? (outputTokens / (elapsedMicroseconds / 1000000.0))
              : liveTokensPerSecond,
        num value => value.toDouble(),
        _ => throw const FormatException(
          'The ChatGPT response metrics were invalid.',
        ),
      };
      updateReasoningSummaries(result['reasoningGroups']);
      updateCitationSources(result['citationSources']);
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
            providerId: providerId,
            modelId: modelId,
            citationSources: assistantCitationSources,
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
        if (status == chat.ChatMessageStatus.completed) {
          await _showAssistantResponseNotification(
            conversationId: conversationId,
            assistantMessageId: savedMessageId,
            providerId: _providerFamily(providerId),
            content: assistantContent,
          );
        }
      }
    } on OpenChatServiceException catch (error) {
      if (resumeRunId != null ||
          error.code == 'question_delivery_failed' ||
          error.code == 'question_wait_interrupted' ||
          error.code == 'question_storage_unavailable' ||
          error.code == 'service_exited' ||
          error.code == 'service_pipe_failed' ||
          error.code == 'service_unavailable') {
        _questionRunRefreshConversations.add(conversationId);
      }
      if (operation case final activeOperation?) {
        await activeOperation.eventsDone;
      }
      await flushFailedStreamPersistence();
      final wasCancelled = error.code == 'operation_cancelled';
      final retryCleanupSucceeded = wasCancelled
          ? await preserveStoppedResponse()
          : await preserveFailedResponse(failureCode: error.code);
      if (mounted) {
        if (!retryCleanupSucceeded) {
          _showMessage(context.openchatL10n.responseRetryCleanupFailed);
        } else if (!wasCancelled && responseToReplace != null) {
          _showServiceFailure(error);
        } else if (!wasCancelled &&
            isGoalSend &&
            error.code != 'rate_limited' &&
            error.code != 'quota_exceeded' &&
            error.code != 'insufficient_quota' &&
            error.code != 'resource_exhausted') {
          _showServiceFailure(error);
        }
      }
    } on Object catch (error, stackTrace) {
      if (resumeRunId != null) {
        _questionRunRefreshConversations.add(conversationId);
      }
      if (operation case final activeOperation?) {
        await activeOperation.eventsDone;
      }
      await flushFailedStreamPersistence();
      final retryCleanupSucceeded = await preserveFailedResponse();
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'chat',
          context: ErrorDescription('while sending a chat message'),
        ),
      );
      if (mounted) {
        if (!retryCleanupSucceeded) {
          _showMessage(context.openchatL10n.responseRetryCleanupFailed);
        } else if (responseToReplace != null) {
          _showMessage(context.openchatL10n.chatRequestFailed);
        }
      }
    } finally {
      await subscription?.cancel();
      _activeChatOperation = null;
      if (mounted) {
        setState(() {
          _isSending = false;
          _pendingToolPermissionRequest = null;
          _isRespondingToToolPermission = false;
          _toolPermissionError = null;
          if (_activeChatConversationId == conversationId) {
            _activeChatConversationId = null;
            _replacingAssistantMessageId = null;
          }
        });
      }
      if (mounted && _selectedConversationId != conversationId) {
        showOpenChatToast(
          context,
          l10n.agentRunEndedInAnotherChat,
          type: OpenChatToastType.info,
        );
      }
      if (_questionRunRefreshConversations.remove(conversationId)) {
        await _loadPendingQuestionGroups(conversationId);
      }
      if (isGoalSend || resumeRunId != null || goalStateMayHaveChanged) {
        await _loadActiveGoal(conversationId);
      }
    }
  }

  Future<void> _branchFromUserMessage(
    ChatConversation sourceConversation,
    chat.ChatMessage sourceMessage,
    String editedContent,
  ) async {
    final repository = widget.chatRepository;
    final service = widget.serviceClient;
    if (repository == null ||
        service == null ||
        _isSending ||
        _isCreatingMessageBranch) {
      return;
    }
    final conversationId = _newLocalId();
    final l10n = context.openchatL10n;
    setState(() => _isCreatingMessageBranch = true);
    try {
      final branch = await repository.createBranchFromUserMessage(
        sourceConversationId: sourceConversation.id,
        throughUserMessageId: sourceMessage.id,
        branchId: conversationId,
        branchTitle: l10n.conversationBranchTitle(sourceConversation.title),
        createdAt: DateTime.now().toUtc(),
        editedUserMessage: editedContent,
      );
      if (!mounted) return;
      _selectConversation(
        branch.id,
        loadModels: false,
        loadPendingQuestions: false,
      );
      await Future.wait<void>(<Future<void>>[
        _loadConversationModels(branch.id),
        _loadPendingQuestionGroups(branch.id),
      ]);
      if (!mounted || _selectedConversationId != branch.id) return;
      final selectedBranch = await repository.getConversation(branch.id);
      if (selectedBranch == null) {
        throw ConversationNotFoundException(branch.id);
      }
      setState(() => _isCreatingMessageBranch = false);
      await _sendMessage(
        selectedBranch,
        messageTextOverride: editedContent,
        userMessageAlreadySaved: true,
      );
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'chat_history',
          context: ErrorDescription('while creating a conversation branch'),
        ),
      );
      if (mounted) _showMessage(l10n.conversationBranchCreateFailed);
    } finally {
      if (mounted && _isCreatingMessageBranch) {
        setState(() => _isCreatingMessageBranch = false);
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

  void _clearActiveSendState() {
    if (!mounted) return;
    setState(() {
      _isSending = false;
      _activeChatConversationId = null;
      _replacingAssistantMessageId = null;
    });
  }

  void _stopMessage() {
    final goal = _activeGoal;
    if (goal != null &&
        goal.status == AgentGoalStatus.running &&
        goal.conversationId == _activeChatConversationId) {
      unawaited(_stopGoal(goal));
      return;
    }
    final operation = _activeChatOperation;
    if (operation != null) unawaited(operation.cancel());
  }

  Future<void> _toggleGoal(
    AgentGoal goal,
    ChatConversation? selectedConversation,
  ) async {
    if (_isChangingGoal) return;
    if (goal.status == AgentGoalStatus.paused ||
        goal.status == AgentGoalStatus.interrupted) {
      if (_isSending || selectedConversation?.id != goal.conversationId) return;
      await _sendMessage(selectedConversation, resumeRunId: goal.runId);
      return;
    }
    if (goal.status != AgentGoalStatus.running) return;
    final service = widget.serviceClient;
    if (service == null) return;
    final operation = _activeChatConversationId == goal.conversationId
        ? _activeChatOperation
        : null;
    setState(() => _isChangingGoal = true);
    try {
      await service.call(
        'chat.goal.pause',
        params: <String, Object?>{
          'conversationId': goal.conversationId,
          'runId': goal.runId,
        },
      );
      if (mounted && _selectedConversationId == goal.conversationId) {
        setState(() {
          _activeGoal = goal.copyWith(
            status: AgentGoalStatus.paused,
            pauseReason: 'user_paused',
          );
        });
      }
      await operation?.cancel();
    } on OpenChatServiceException catch (error) {
      if (mounted) _showServiceFailure(error);
    } finally {
      if (mounted) setState(() => _isChangingGoal = false);
      if (_selectedConversationId == goal.conversationId) {
        await _loadActiveGoal(goal.conversationId);
      }
    }
  }

  Future<void> _stopGoal(AgentGoal goal) async {
    if (_isChangingGoal) return;
    final service = widget.serviceClient;
    if (service == null) return;
    final operation = _activeChatConversationId == goal.conversationId
        ? _activeChatOperation
        : null;
    setState(() => _isChangingGoal = true);
    var stopped = false;
    try {
      await service.call(
        'chat.goal.stop',
        params: <String, Object?>{
          'conversationId': goal.conversationId,
          'runId': goal.runId,
        },
      );
      stopped = true;
      if (mounted && _selectedConversationId == goal.conversationId) {
        setState(() => _activeGoal = null);
      }
      await operation?.cancel();
    } on OpenChatServiceException catch (error) {
      if (mounted) _showServiceFailure(error);
    } finally {
      if (mounted) setState(() => _isChangingGoal = false);
      if (!stopped && _selectedConversationId == goal.conversationId) {
        await _loadActiveGoal(goal.conversationId);
      }
    }
  }

  void _handleToolPermissionRequest(Map<String, Object?> data) {
    final request = ToolPermissionRequest.fromEvent(data);
    final existingRequest = _pendingToolPermissionRequest;
    if (request == null) {
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
    if (existingRequest != null) {
      if (existingRequest.id == request.id) return;
      if (!_isRespondingToToolPermission) {
        FlutterError.reportError(
          FlutterErrorDetails(
            exception: const FormatException(
              'A second tool permission request arrived before the first was answered.',
            ),
            library: 'local_service',
          ),
        );
        unawaited(_activeChatOperation?.cancel());
        return;
      }
    }
    if (!mounted) return;
    setState(() {
      _pendingToolPermissionRequest = request;
      _isRespondingToToolPermission = false;
      _toolPermissionError = null;
    });
  }

  Future<void> _respondToToolPermission({required bool approved}) async {
    final request = _pendingToolPermissionRequest;
    final service = widget.serviceClient;
    if (request == null || service == null || _isRespondingToToolPermission) {
      return;
    }
    setState(() {
      _isRespondingToToolPermission = true;
      _toolPermissionError = null;
    });
    try {
      await service.call(
        'chat.tool.permission.respond',
        params: <String, Object?>{
          'approvalRequestId': request.id,
          'approved': approved,
        },
      );
    } on Object catch (error, stackTrace) {
      if (error is OpenChatServiceException &&
          error.code == 'tool_permission_request_unavailable') {
        if (mounted && _pendingToolPermissionRequest?.id == request.id) {
          setState(() {
            _pendingToolPermissionRequest = null;
            _isRespondingToToolPermission = false;
            _toolPermissionError = null;
          });
          _showMessage(context.openchatL10n.toolPermissionRequestExpired);
        }
        return;
      }
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'local_service',
          context: ErrorDescription('while sending a tool permission choice'),
        ),
      );
      if (mounted && _pendingToolPermissionRequest?.id == request.id) {
        setState(() {
          _isRespondingToToolPermission = false;
          _toolPermissionError =
              context.openchatL10n.toolPermissionResponseFailed;
        });
      }
      return;
    }

    if (mounted && _pendingToolPermissionRequest?.id == request.id) {
      setState(() {
        _pendingToolPermissionRequest = null;
        _isRespondingToToolPermission = false;
        _toolPermissionError = null;
      });
    }
  }

  void _showServiceFailure(OpenChatServiceException error) {
    final l10n = context.openchatL10n;
    final message = switch (error.code) {
      'goal_already_active' => l10n.goalAlreadyActive,
      'rate_limited' => l10n.providerRateLimited,
      'opencode_free_tier_restricted' => l10n.openCodeFreeTierRestricted,
      'authentication_required' ||
      'refresh_rejected' => l10n.providerAuthenticationRequired,
      'provider_tool_request_rejected' => l10n.providerToolRequestRejected,
      'network_unavailable' => l10n.providerNetworkUnavailable,
      'provider_endpoint_unavailable' ||
      'invalid_provider_response' => l10n.providerRequestFailed,
      'model_unavailable' => l10n.selectedModelUnavailable,
      'local_engine_not_installed' ||
      'local_engine_unavailable' => l10n.localModelEngineNotReady,
      'local_model_unavailable' => l10n.localModelInvalid,
      'local_engine_start_failed' => l10n.localModelStartError,
      'local_engine_start_timeout' => l10n.localModelStartTimeout,
      'local_engine_runtime_unavailable' => l10n.localModelRuntimeUnavailable,
      'local_engine_capability_unavailable' =>
        l10n.localModelContextUnavailable,
      'local_model_inference_failed' => l10n.localModelInferenceFailed,
      'context_window_exceeded' ||
      'context_compaction_input_too_large' => l10n.contextWindowExceeded,
      'conversation_not_routed' => l10n.modelRequired,
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

  Future<void> _loadChatGptFastMode() async {
    try {
      final enabled = await _settingsPreferences.readChatGptFastMode();
      if (!mounted) return;
      setState(() {
        _chatGptFastModeEnabled = enabled;
        _isLoadingChatGptFastMode = false;
      });
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while loading ChatGPT Fast mode settings'),
        ),
      );
      if (!mounted) return;
      setState(() => _isLoadingChatGptFastMode = false);
      _showMessage(context.openchatL10n.chatGptFastModeSettingsLoadFailed);
    }
  }

  Future<void> _setChatGptFastMode(bool enabled) async {
    if (_isLoadingChatGptFastMode ||
        _isSavingChatGptFastMode ||
        enabled == _chatGptFastModeEnabled) {
      return;
    }
    setState(() => _isSavingChatGptFastMode = true);
    try {
      await _settingsPreferences.writeChatGptFastMode(enabled);
      if (!mounted) return;
      setState(() {
        _chatGptFastModeEnabled = enabled;
        _isSavingChatGptFastMode = false;
      });
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while saving ChatGPT Fast mode settings'),
        ),
      );
      if (!mounted) return;
      setState(() => _isSavingChatGptFastMode = false);
      _showMessage(context.openchatL10n.chatGptFastModeSettingsSaveFailed);
    }
  }

  Future<void> _loadCollapsedSidebarSections() async {
    try {
      final sections = await _settingsPreferences
          .readCollapsedSidebarSections();
      if (!mounted || _didToggleSidebarSection) return;
      setState(() => _collapsedSidebarSections = sections);
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'settings',
          context: ErrorDescription('while loading sidebar section settings'),
        ),
      );
    }
  }

  bool _didToggleSidebarSection = false;
  Future<void> _sidebarPreferenceWrite = Future<void>.value();

  void _toggleSidebarSection(String section) {
    _didToggleSidebarSection = true;
    final updated = Set<String>.of(_collapsedSidebarSections);
    if (!updated.add(section)) updated.remove(section);
    setState(() => _collapsedSidebarSections = updated);
    _sidebarPreferenceWrite = _sidebarPreferenceWrite.then((_) async {
      try {
        await _settingsPreferences.writeCollapsedSidebarSections(
          Set<String>.of(_collapsedSidebarSections),
        );
      } on Object catch (error, stackTrace) {
        FlutterError.reportError(
          FlutterErrorDetails(
            exception: error,
            stack: stackTrace,
            library: 'settings',
            context: ErrorDescription('while saving sidebar section settings'),
          ),
        );
      }
    });
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

  Future<void> _openProjectOptions(
    String projectId,
    String projectRoot,
    String projectName,
  ) async {
    final action = await showDialog<_ProjectOptionsAction>(
      context: context,
      builder: (dialogContext) {
        final l10n = dialogContext.openchatL10n;
        return AlertDialog(
          title: Text(l10n.projectOptionsTitle),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              ListTile(
                leading: const Icon(LucideIcons.hand),
                title: Text(l10n.projectOptionsToolPermissions),
                onTap: () =>
                    Navigator.of(dialogContext)
                        .pop(_ProjectOptionsAction.toolPermissions),
              ),
              ListTile(
                leading: const Icon(LucideIcons.gitBranch),
                title: Text(l10n.projectOptionsWorktrees),
                onTap: () =>
                    Navigator.of(dialogContext)
                        .pop(_ProjectOptionsAction.worktrees),
              ),
            ],
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(dialogContext).pop(),
              child: Text(l10n.cancel),
            ),
          ],
        );
      },
    );
    if (!mounted || action == null) return;
    if (action == _ProjectOptionsAction.toolPermissions) {
      await _openProjectToolPermissions(projectId);
      return;
    }
    final service = widget.serviceClient;
    if (service == null) return;
    final ToolPermissionMode permissionMode;
    final Map<String, ToolPermissionRule> projectPermissionRules;
    try {
      permissionMode = await _settingsPreferences.readToolPermissionMode();
      projectPermissionRules = await _settingsPreferences
          .readProjectToolPermissionRules(projectId);
    } on Exception {
      if (mounted) {
        _showMessage(context.openchatL10n.projectToolRulesLoadFailed);
      }
      return;
    }
    if (!mounted) return;
    await showDialog<void>(
      context: context,
      builder: (context) => ProjectWorktreesDialog(
        serviceClient: service,
        projectId: projectId,
        projectRoot: projectRoot,
        permissionMode: permissionMode,
        projectPermissionRules: projectPermissionRules,
        onUseWorktree: (path, branch) =>
            _createWorktreeProject(projectName, path, branch),
      ),
    );
  }

  Future<void> _createWorktreeProject(
    String projectName,
    String folderPath,
    String branch,
  ) async {
    final repository = widget.chatRepository;
    if (repository == null ||
        widget.historyStorageStatus != HistoryStorageStatus.available) {
      throw StateError('Project history is unavailable.');
    }
    final projectId = _newLocalId();
    await repository.createProject(
      id: projectId,
      name: '$projectName ($branch)',
      folderPath: folderPath,
      createdAt: DateTime.now().toUtc(),
    );
    if (!mounted) return;
    _startProjectConversation(projectId);
    showOpenChatToast(
      context,
      context.openchatL10n.projectWorktreeUse,
      type: OpenChatToastType.success,
    );
  }

  Future<void> _openProjectToolPermissions(String projectId) async {
    try {
      final rules = await _settingsPreferences.readProjectToolPermissionRules(
        projectId,
      );
      if (!mounted) return;
      await showDialog<void>(
        context: context,
        builder: (context) => ProjectToolPermissionsDialog(
          initialRules: rules,
          onSave: (updatedRules) => _settingsPreferences
              .writeProjectToolPermissionRules(projectId, updatedRules),
        ),
      );
    } on Exception catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'tool_permissions',
          context: ErrorDescription('while opening project tool permissions'),
        ),
      );
      if (mounted) {
        _showMessage(context.openchatL10n.projectToolRulesLoadFailed);
      }
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
        providerId: 'opencode',
        selectedModelId: _selectedModelId,
        selectedModelRouteKey: _selectedModelId == null
            ? null
            : _routeKey('opencode', modelId: _selectedModelId),
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
      final fileChangesRemoved = await _deleteFileChangesWithReporting(
        () =>
            _chatFileChangesRepository?.deleteConversation(conversationId) ??
            Future<void>.value(),
        contextDescription: 'while deleting conversation file changes',
      );
      if (!mounted) return;
      if (_selectedConversationId == conversationId) _startNewConversation();
      showOpenChatToast(
        context,
        fileChangesRemoved
            ? context.openchatL10n.conversationDeleted
            : context.openchatL10n.conversationDeletedFileChangesCleanupFailed,
        type: fileChangesRemoved
            ? OpenChatToastType.success
            : OpenChatToastType.warning,
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

  Future<void> _setConversationArchived(
    String conversationId, {
    required bool isArchived,
  }) async {
    final repository = widget.chatRepository;
    if (repository == null) return;
    if (isArchived && _activeChatConversationId == conversationId) {
      _showMessage(context.openchatL10n.stopResponseBeforeArchive);
      return;
    }

    try {
      await repository.setConversationArchived(
        conversationId: conversationId,
        isArchived: isArchived,
      );
      if (!mounted) return;
      if (isArchived && _selectedConversationId == conversationId) {
        _startNewConversation();
      }
      showOpenChatToast(
        context,
        isArchived
            ? context.openchatL10n.conversationArchived
            : context.openchatL10n.conversationRestored,
        type: OpenChatToastType.success,
      );
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'chat_history',
          context: ErrorDescription(
            'while changing conversation archive state',
          ),
        ),
      );
      if (mounted) {
        _showMessage(
          isArchived
              ? context.openchatL10n.conversationArchiveFailed
              : context.openchatL10n.conversationRestoreFailed,
        );
      }
    }
  }

  Future<bool> _deleteFileChangesWithReporting(
    Future<void> Function() delete, {
    required String contextDescription,
  }) async {
    try {
      await delete();
      return true;
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'chat_file_changes',
          context: ErrorDescription(contextDescription),
        ),
      );
      return false;
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

  Future<void> _pickAttachments({required bool supportsImages}) async {
    final repository = widget.chatRepository;
    if (repository?.supportsAttachments != true || _isSending) return;
    final l10n = context.openchatL10n;
    final allowedExtensions = ChatAttachmentTypes.allowedExtensions
        .where(
          (extension) =>
              supportsImages ||
              !ChatAttachmentTypes.imageExtensions.contains(extension),
        )
        .toList(growable: false);

    try {
      final selectedFiles = await FilePicker.pickFiles(
        type: FileType.custom,
        allowedExtensions: allowedExtensions,
      );
      if (!mounted || selectedFiles.isEmpty) return;
      final existing = List<ChatAttachment>.of(_pendingAttachments);
      if (existing.length + selectedFiles.length >
              ChatAttachmentStore.maximumFileCount ||
          existing.where((attachment) => attachment.isImage).length +
                  selectedFiles.where((file) {
                    final kind = ChatAttachmentTypes.kindForExtension(
                      file.extension ?? '',
                    );
                    return kind == ChatAttachmentKind.image;
                  }).length >
              ChatAttachmentStore.maximumImageCount) {
        _showMessage(l10n.attachmentCountExceeded);
        return;
      }

      var totalBytes = existing.fold<int>(
        0,
        (total, attachment) => total + attachment.sizeBytes,
      );
      final added = <ChatAttachment>[];
      for (final file in selectedFiles) {
        final extension = file.extension?.toLowerCase();
        final kind = extension == null
            ? null
            : ChatAttachmentTypes.kindForExtension(extension);
        final mimeType = extension == null
            ? null
            : ChatAttachmentTypes.mimeTypeForExtension(extension);
        if (kind == null || mimeType == null) {
          _showMessage(l10n.unsupportedAttachmentFile);
          return;
        }
        if (kind == ChatAttachmentKind.image && !supportsImages) {
          _showMessage(l10n.modelDoesNotSupportImages);
          return;
        }
        final length = await file.length();
        final maximumBytes = kind == ChatAttachmentKind.image
            ? ChatAttachmentStore.maximumImageBytes
            : ChatAttachmentStore.maximumTextBytes;
        if (length == null || length <= 0 || length > maximumBytes) {
          _showMessage(l10n.attachmentFileTooLarge);
          return;
        }
        final fileSizeBytes = length.toInt();
        totalBytes += fileSizeBytes;
        if (totalBytes > ChatAttachmentStore.maximumTotalBytes) {
          _showMessage(l10n.attachmentTotalTooLarge);
          return;
        }
        final bytes = await file.readAsBytes();
        if (bytes.length != fileSizeBytes) {
          _showMessage(l10n.attachmentReadFailed);
          return;
        }
        if (kind == ChatAttachmentKind.image &&
            !ChatAttachmentTypes.hasValidImageSignature(mimeType, bytes)) {
          _showMessage(l10n.attachmentInvalidImage);
          return;
        }
        if (kind == ChatAttachmentKind.text) {
          try {
            const Utf8Decoder(allowMalformed: false).convert(bytes);
          } on FormatException {
            _showMessage(l10n.attachmentMustBeUtf8);
            return;
          }
        }
        added.add(
          ChatAttachment(
            id: _newLocalId(),
            name: file.name,
            mimeType: mimeType,
            sizeBytes: fileSizeBytes,
            kind: kind,
            bytes: bytes,
          ),
        );
      }
      if (added.isNotEmpty) {
        setState(() => _pendingAttachments.addAll(added));
      }
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: StateError('Attachment selection or reading failed.'),
          stack: stackTrace,
          library: 'attachments',
          context: ErrorDescription('while selecting chat attachments'),
        ),
      );
      if (mounted) _showMessage(l10n.attachmentReadFailed);
    }
  }

  void _removePendingAttachment(String attachmentId) {
    setState(() {
      _pendingAttachments.removeWhere(
        (attachment) => attachment.id == attachmentId,
      );
    });
  }

  @override
  void dispose() {
    _historySearchDebounceTimer?.cancel();
    _historySearchGeneration++;
    _fileChangesLoadGeneration++;
    unawaited(_questionServiceEvents?.cancel());
    _pendingAttachments.clear();
    _searchController.dispose();
    _messageController.dispose();
    _messageScrollController.dispose();
    final downloadController = _modelDownloadController;
    downloadController?.removeListener(_handleModelDownloadChanged);
    downloadController?.dispose();
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
        final historyError = switch (widget.historyStorageStatus) {
          HistoryStorageStatus.corrupt => l10n.historyStorageCorruptDescription,
          HistoryStorageStatus.backupUnavailable =>
            l10n.historyStorageBackupFailedDescription,
          HistoryStorageStatus.unavailable =>
            l10n.historyStorageUnavailableDescription,
          HistoryStorageStatus.loading || HistoryStorageStatus.available =>
            conversationSnapshot.hasError ? l10n.historyLoadFailed : null,
        };
        final resolvedStorageStatus = conversationSnapshot.hasError
            ? HistoryStorageStatus.unavailable
            : widget.historyStorageStatus;

        return LayoutBuilder(
          builder: (context, constraints) {
            final showSidebar =
                !_settingsOpen &&
                !_modelsPageOpen &&
                !_localModelsPageOpen &&
                !_sidebarsCompact &&
                constraints.maxWidth >= OpenChatSpacing.sidebarBreakpoint;
            final sidebarWidth = _sidebarsCompact
                ? OpenChatSpacing.collapsedSidebarWidth
                : constraints.maxWidth >= OpenChatSpacing.fullSidebarBreakpoint
                ? OpenChatSpacing.sidebarWidth
                : OpenChatSpacing.compactSidebarWidth;
            final drawerWidth = constraints.maxWidth < sidebarWidth
                ? constraints.maxWidth
                : sidebarWidth;

            return Scaffold(
              key: _scaffoldKey,
              drawer:
                  _settingsOpen ||
                      _modelsPageOpen ||
                      _localModelsPageOpen ||
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
                          onRetryStorage:
                              resolvedStorageStatus ==
                                      HistoryStorageStatus.unavailable ||
                                  resolvedStorageStatus ==
                                      HistoryStorageStatus.backupUnavailable
                              ? widget.onRetryStorage
                              : null,
                        ),
                      ),
                    ),
              body: Column(
                children: [
                  const OpenChatWindowTitleBar(),
                  Expanded(
                    child: Stack(
                      children: [
                        Row(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: [
                            ChatNavigationRail(
                              expanded: false,
                              showBrand: !OpenChatWindowControls.isSupportedOn(
                                Theme.of(context).platform,
                              ),
                              settingsSelected: _settingsOpen,
                              modelsSelected: _modelsPageOpen,
                              localModelsSelected: _localModelsPageOpen,
                              onOpenChat: _startNewConversation,
                              onOpenSettings: () => setState(() {
                                _settingsOpen = true;
                                _modelsPageOpen = false;
                                _localModelsPageOpen = false;
                              }),
                              onOpenModels: () => setState(() {
                                _modelsPageOpen = true;
                                _settingsOpen = false;
                                _localModelsPageOpen = false;
                              }),
                              onOpenLocalModels: () => setState(() {
                                _localModelsPageOpen = true;
                                _settingsOpen = false;
                                _modelsPageOpen = false;
                              }),
                              onToggleTheme: _handleThemeToggle,
                              sidebarsCompact: _sidebarsCompact,
                              onToggleSidebars: () => setState(
                                () => _sidebarsCompact = !_sidebarsCompact,
                              ),
                            ),
                            if (_settingsOpen)
                              Expanded(
                                child: _buildMainSurface(
                                  surfaceKey: 'settings',
                                  child: SettingsScreen(
                                    themeMode: widget.themeMode,
                                    locale: widget.locale,
                                    conversationWidth: widget.conversationWidth,
                                    conversationTextSize:
                                        widget.conversationTextSize,
                                    appFont: widget.appFont,
                                    settingsPreferences: _settingsPreferences,
                                    onThemeModeChanged:
                                        widget.onThemeModeChanged,
                                    onLocaleChanged: widget.onLocaleChanged,
                                    onConversationWidthChanged:
                                        widget.onConversationWidthChanged,
                                    onConversationTextSizeChanged:
                                        widget.onConversationTextSizeChanged,
                                    onAppFontChanged: widget.onAppFontChanged,
                                    historyStorageStatus: resolvedStorageStatus,
                                    hasConversationHistory:
                                        conversations.isNotEmpty,
                                    activeConversationId:
                                        selectedConversation?.id,
                                    activeConversationTitle:
                                        selectedConversation?.title,
                                    isActiveConversationSending:
                                        _isSending &&
                                        _activeChatConversationId ==
                                            selectedConversation?.id,
                                    onClearConversationHistory:
                                        widget.chatRepository == null
                                        ? null
                                        : _clearConversationHistory,
                                    chatGptApiKeyStore:
                                        widget.chatGptApiKeyStore,
                                    apiCompatibleProviderKeyStore:
                                        widget.apiCompatibleProviderKeyStore,
                                    openCodeApiKeyStore:
                                        widget.openCodeApiKeyStore,
                                    serviceClient: widget.serviceClient,
                                    chatRepository: widget.chatRepository,
                                    onProviderStateChanged:
                                        _refreshProviderState,
                                    onConnectionRemoved:
                                        _handleConnectionRemoved,
                                    onOpenConversation: _selectConversation,
                                  ),
                                ),
                              )
                            else if (_modelsPageOpen)
                              Expanded(
                                child: _buildMainSurface(
                                  surfaceKey: 'models',
                                  child: ModelsPage(
                                    serviceClient: widget.serviceClient,
                                    downloadController:
                                        _modelDownloadController,
                                    settingsPreferences: _settingsPreferences,
                                  ),
                                ),
                              )
                            else if (_localModelsPageOpen)
                              Expanded(
                                child: _buildMainSurface(
                                  surfaceKey: 'local-models',
                                  child: LocalModelsPage(
                                    serviceClient: widget.serviceClient,
                                    onOpenModelCatalog: () => setState(() {
                                      _modelsPageOpen = true;
                                      _localModelsPageOpen = false;
                                    }),
                                  ),
                                ),
                              )
                            else ...[
                              Expanded(
                                child: _buildMainSurface(
                                  surfaceKey: 'chat',
                                  child: Row(
                                    crossAxisAlignment:
                                        CrossAxisAlignment.stretch,
                                    children: [
                                      AnimatedSize(
                                        key: const ValueKey<String>(
                                          'conversation-sidebar-transition',
                                        ),
                                        alignment: Alignment.centerLeft,
                                        duration:
                                            MediaQuery.disableAnimationsOf(
                                              context,
                                            )
                                            ? Duration.zero
                                            : const Duration(milliseconds: 220),
                                        curve: Curves.easeInOutCubic,
                                        child: SizedBox(
                                          width: showSidebar ? sidebarWidth : 0,
                                          child: showSidebar
                                              ? _buildSidebar(
                                                  width: sidebarWidth,
                                                  conversations: conversations,
                                                  selectedConversationId:
                                                      _selectedConversationId,
                                                  isLoading: isLoading,
                                                  errorMessage: historyError,
                                                  onRetryStorage:
                                                      resolvedStorageStatus ==
                                                          HistoryStorageStatus
                                                              .unavailable
                                                      ? widget.onRetryStorage
                                                      : null,
                                                )
                                              : const SizedBox.shrink(),
                                        ),
                                      ),
                                      Expanded(
                                        child: _buildConversationPane(
                                          selectedConversation:
                                              selectedConversation,
                                        ),
                                      ),
                                    ],
                                  ),
                                ),
                              ),
                            ],
                          ],
                        ),
                      ],
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

  Widget _buildMainSurface({
    required String surfaceKey,
    required Widget child,
  }) {
    final reducedMotion = MediaQuery.disableAnimationsOf(context);
    return Padding(
      padding: const EdgeInsets.only(
        right: OpenChatSpacing.mainSurfaceInset,
        bottom: OpenChatSpacing.mainSurfaceInset,
      ),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(OpenChatRadii.control),
        clipBehavior: Clip.antiAlias,
        child: AnimatedSwitcher(
          duration: reducedMotion
              ? Duration.zero
              : const Duration(milliseconds: 190),
          switchInCurve: Curves.easeOutCubic,
          switchOutCurve: Curves.easeInCubic,
          transitionBuilder: (child, animation) {
            final position = animation.drive(
              Tween<Offset>(
                begin: const Offset(0, 0.012),
                end: Offset.zero,
              ).chain(CurveTween(curve: Curves.easeOutCubic)),
            );
            return FadeTransition(
              opacity: animation,
              child: SlideTransition(position: position, child: child),
            );
          },
          child: KeyedSubtree(key: ValueKey<String>(surfaceKey), child: child),
        ),
      ),
    );
  }

  Widget _buildSidebar({
    required double width,
    required List<ChatConversation> conversations,
    required String? selectedConversationId,
    required bool isLoading,
    required String? errorMessage,
    required VoidCallback? onRetryStorage,
    bool showDivider = true,
  }) {
    return StreamBuilder<List<ChatProject>>(
      stream: _projectStream,
      builder: (context, projectSnapshot) {
        final projectRows = projectSnapshot.data ?? const <ChatProject>[];
        final activeConversations = conversations
            .where((conversation) => !conversation.isArchived)
            .toList(growable: false);
        final archivedConversations = conversations
            .where((conversation) => conversation.isArchived)
            .toList(growable: false);
        final sidebarProjects = projectRows
            .map(
              (project) => ConversationSidebarProject(
                id: project.id,
                title: project.name,
                folderPath: project.folderPath,
                conversations: activeConversations
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
        final pinnedConversations = activeConversations
            .where((conversation) => conversation.isPinned)
            .toList(growable: false);
        final unassignedConversations = activeConversations
            .where(
              (conversation) =>
                  conversation.projectId == null && !conversation.isPinned,
            )
            .toList(growable: false);

        return ConversationSidebar(
          width: width,
          showDivider: showDivider,
          searchController: _searchController,
          isHistorySearchOpen: _isHistorySearchOpen,
          onOpenHistorySearch: _openHistorySearch,
          onOpenRunManager:
              widget.serviceClient == null ||
                  widget.historyStorageStatus != HistoryStorageStatus.available
              ? null
              : () => unawaited(_openRunManager()),
          onCloseHistorySearch: _closeHistorySearch,
          onSearchChanged: _handleHistorySearchChanged,
          onSearchSubmitted: _submitHistorySearch,
          historySearchQuery: _historySearchQuery,
          historySearchResults: _historySearchResults,
          isHistorySearchLoading: _isSearchingHistory,
          historySearchErrorCode: _historySearchErrorCode,
          onSelectHistoryResult: _selectHistorySearchResult,
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
          archivedConversations: archivedConversations
              .map(_sidebarConversationFromModel)
              .toList(growable: false),
          selectedConversationId: selectedConversationId,
          onSelectConversation: _selectConversation,
          onToggleConversationPinned: (conversationId) =>
              unawaited(_toggleConversationPinned(conversationId)),
          onPinConversation: (conversationId) =>
              unawaited(_pinConversation(conversationId)),
          collapsedSections: _collapsedSidebarSections,
          onToggleSection: _toggleSidebarSection,
          onRenameConversation: _requestConversationRename,
          onDeleteConversation: (conversationId) =>
              unawaited(_deleteConversation(conversationId)),
          onExportConversation: (conversationId) =>
              unawaited(_exportConversation(conversationId)),
          onArchiveConversation: (conversationId) => unawaited(
            _setConversationArchived(
              conversationId,
              isArchived: !archivedConversations.any(
                (conversation) => conversation.id == conversationId,
              ),
            ),
          ),
          onMoveConversationToProject: (conversationId, projectId) =>
              unawaited(_moveConversationToProject(conversationId, projectId)),
          onOpenProjectOptions: (projectId, projectRoot, projectName) =>
              unawaited(
                _openProjectOptions(projectId, projectRoot, projectName),
              ),
          onCreateProjectConversation: _startProjectConversation,
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
          onRetryStorage: onRetryStorage,
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
      isArchived: conversation.isArchived,
    );
  }

  Widget _buildConversationPane({
    required ChatConversation? selectedConversation,
  }) {
    final selectedModelId = _selectedModelId ?? selectedConversation?.modelId;
    final routeProviderId = _isUpdatingConversationModel
        ? _selectedProviderId
        : selectedConversation?.providerId ??
              (selectedConversation?.connectionId != null
                  ? 'chatgpt'
                  : _selectedProviderId);
    final routeConnectionId = routeProviderId == 'chatgpt'
        ? selectedConversation?.connectionId ?? _selectedConnectionId
        : null;
    final routeWorkspaceId = routeProviderId == 'chatgpt'
        ? selectedConversation?.workspaceId ?? _selectedWorkspaceId
        : null;
    final routeApiKeyConnectionId = _isApiKeyRouteProvider(routeProviderId)
        ? selectedConversation?.apiKeyConnectionId ??
              _selectedApiKeyConnectionId ??
              (ApiCompatibleProviderKeyStore.providerIds.contains(
                    routeProviderId,
                  )
                  ? routeProviderId
                  : null)
        : null;
    final selectedModelRouteKey = _routeKey(
      routeProviderId,
      modelId: selectedModelId,
      connectionId: routeConnectionId,
      workspaceId: routeWorkspaceId,
      apiKeyConnectionId: routeApiKeyConnectionId,
    );
    final selectedModel = _firstOrNull(
      _models.where(
        (model) => model.routeKey == selectedModelRouteKey && model.isAvailable,
      ),
    );
    final modelLabel =
        selectedModel?.displayName ??
        selectedModelId ??
        widget.selectedModelLabel;
    final modelOptions = _models
        .where(
          (model) =>
              model.isAvailable ||
              _localEngineProviderIds.contains(model.providerId),
        )
        .toList();
    final reasoningOptions = selectedModel?.reasoningLevels ?? const <String>[];
    final selectedReasoning =
        reasoningOptions.contains(_selectedReasoningEffort)
        ? _selectedReasoningEffort
        : null;
    final routeReady =
        routeProviderId == 'opencode' ||
        _localEngineProviderIds.contains(routeProviderId) ||
        (routeProviderId == 'chatgpt' &&
            routeConnectionId != null &&
            routeWorkspaceId != null) ||
        (_isApiKeyRouteProvider(routeProviderId) &&
            routeApiKeyConnectionId != null);
    final modelRouteReady =
        _loadedProviderId == _providerFamily(routeProviderId) &&
        (routeProviderId == 'opencode' ||
            _localEngineProviderIds.contains(routeProviderId) ||
            (_isApiKeyRouteProvider(routeProviderId) &&
                _loadedApiKeyConnectionIds.contains(routeApiKeyConnectionId)) ||
            (ApiCompatibleProviderKeyStore.providerIds.contains(
                  routeProviderId,
                ) &&
                _availableCompatibleProviderIds.contains(routeProviderId)) ||
            (routeProviderId == 'chatgpt' &&
                (routeConnectionId == _loadedConnectionId &&
                    routeWorkspaceId == _loadedWorkspaceId)));
    final canSend =
        widget.historyStorageStatus == HistoryStorageStatus.available &&
        _toolPermissionModeReady &&
        !_isLoadingToolPermissionMode &&
        !_isSavingToolPermissionMode &&
        !_isLoadingChatGptFastMode &&
        routeReady &&
        modelRouteReady &&
        selectedModel != null &&
        !_isUpdatingConversationModel &&
        !_isLoadingPendingQuestions &&
        !_questionLoadFailed &&
        _pendingQuestionGroups.isEmpty &&
        !_isCreatingMessageBranch &&
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
            final activeGoal =
                _activeGoal?.conversationId == selectedConversation?.id
                ? _activeGoal
                : null;
            return ConversationPane(
              messageController: _messageController,
              messageScrollController: _messageScrollController,
              historySearchTargetMessageId:
                  _historyTargetConversationId == _selectedConversationId
                  ? _historyTargetMessageId
                  : null,
              historySearchTargetRequestId: _historyTargetRequestId,
              onHistorySearchTargetHandled: _handleHistorySearchTarget,
              showHistoryButton:
                  MediaQuery.sizeOf(context).width <
                  OpenChatSpacing.sidebarBreakpoint,
              onOpenHistory: () => _scaffoldKey.currentState?.openDrawer(),
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
              onBranchMessage:
                  selectedConversation == null ||
                      !canSend ||
                      widget.chatRepository == null ||
                      widget.serviceClient == null
                  ? null
                  : (message, editedContent) => _branchFromUserMessage(
                      selectedConversation,
                      message,
                      editedContent,
                    ),
              onStopMessage: _stopMessage,
              activeGoal: activeGoal,
              isChangingGoal:
                  _isChangingGoal ||
                  (_isSending && activeGoal?.status != AgentGoalStatus.running),
              onPauseOrResumeGoal: activeGoal == null
                  ? null
                  : () => unawaited(
                      _toggleGoal(activeGoal, selectedConversation),
                    ),
              onStopGoal: activeGoal == null
                  ? null
                  : () => unawaited(_stopGoal(activeGoal)),
              canSendMessage: canSend,
              pendingAttachments: _pendingAttachments,
              onAddAttachments: () => _pickAttachments(
                supportsImages: selectedModel?.supportsImages ?? false,
              ),
              onRemoveAttachment: _removePendingAttachment,
              attachmentsEnabled:
                  widget.chatRepository?.supportsAttachments == true &&
                  !_isSending,
              isSending: _isSending,
              isLoadingModels: _isLoadingModels,
              models: modelOptions,
              favoriteModels: favoriteSnapshot.data ?? const <FavoriteModel>[],
              providerId: _providerFamily(routeProviderId),
              isChatGptConnected: _isChatGptConnected,
              chatGptFastModeEnabled: _chatGptFastModeEnabled,
              chatGptFastModeAvailable:
                  routeProviderId == 'chatgpt_api' ||
                  (routeProviderId == 'chatgpt' &&
                      selectedModel?.supportsFastMode == true),
              chatGptFastModeLoading: _isLoadingChatGptFastMode,
              chatGptFastModeSaving: _isSavingChatGptFastMode,
              onChatGptFastModeChanged: (enabled) =>
                  unawaited(_setChatGptFastMode(enabled)),
              availableProviderIds: {
                'opencode',
                ..._localEngineProviderIds,
                if (_isChatGptConnected) 'chatgpt',
                ..._availableCompatibleProviderIds,
              },
              onProviderSelected: selectedConversation == null
                  ? _selectProvider
                  : null,
              hiddenModelKeys: _hiddenModelKeys,
              selectedModelId: selectedModelId,
              selectedModelRouteKey: selectedModelRouteKey,
              onModelSelected: _isUpdatingConversationModel
                  ? null
                  : _selectModel,
              onModelFavoriteChanged:
                  (
                    providerId,
                    modelId,
                    displayName,
                    sourceConnectionId,
                    isFavorite,
                  ) => unawaited(
                    _setModelFavorite(
                      providerId: providerId,
                      modelId: modelId,
                      displayName: displayName,
                      sourceConnectionId: sourceConnectionId,
                      isFavorite: isFavorite,
                    ),
                  ),
              onFavoriteModelSelected: (favorite) =>
                  unawaited(_selectFavoriteModel(favorite)),
              reasoningOptions: reasoningOptions,
              showReasoningSelector: selectedModel?.supportsReasoning ?? false,
              onReasoningSelected: _selectReasoning,
              selectedModelLabel: resolvedModelLabel,
              modelsEmptyLabel: modelsEmptyLabel,
              reasoningLevel: selectedReasoning,
              toolPermissionMode: _toolPermissionMode,
              onToolPermissionModeChanged:
                  _isLoadingToolPermissionMode || _isSavingToolPermissionMode
                  ? null
                  : _selectToolPermissionMode,
              toolPermissionRequest: _pendingToolPermissionRequest,
              isRespondingToToolPermission: _isRespondingToToolPermission,
              toolPermissionError: _toolPermissionError,
              onApproveToolPermission: () =>
                  unawaited(_respondToToolPermission(approved: true)),
              onDenyToolPermission: () =>
                  unawaited(_respondToToolPermission(approved: false)),
              pendingQuestionGroups: _pendingQuestionGroups,
              focusedQuestionGroupId: _focusedQuestionGroupId,
              isResumingQuestion: _isResumingQuestion,
              pendingQuestionError: _questionLoadFailed
                  ? context.openchatL10n.userQuestionLoadFailed
                  : null,
              onRetryPendingQuestions: selectedConversation == null
                  ? null
                  : () => unawaited(
                      _loadPendingQuestionGroups(selectedConversation.id),
                    ),
              onSubmitQuestionAnswers: selectedConversation == null
                  ? null
                  : (group, answers) => _submitQuestionAnswers(
                      selectedConversation,
                      group,
                      answers,
                    ),
              conversationTitle: selectedConversation?.title,
              conversationId: selectedConversation?.id,
              contextProviderId: routeProviderId,
              contextModelId: selectedModelId,
              contextSupportsTools: selectedModel?.supportsTools,
              contextWindow: selectedModel?.contextWindow,
              contextConnectionId: routeConnectionId ?? routeApiKeyConnectionId,
              contextWorkspaceId: routeWorkspaceId,
              conversationMemoryRepository: _conversationMemoryRepository,
              fileChangesRepository: _chatFileChangesRepository,
              conversationFileChanges:
                  selectedConversation?.id == _fileChangesConversationId
                  ? _conversationFileChanges
                  : const <ChatFileChange>[],
              fileChangesRevision: _fileChangesRevision,
              isFileChangesPanelOpen: _isFileChangesPanelOpen,
              onOpenFileChanges: _openConversationFileChanges,
              onCloseFileChanges: _closeConversationFileChanges,
              onFileChangesUpdated: selectedConversation == null
                  ? null
                  : (changes) => _updateConversationFileChanges(
                      selectedConversation.id,
                      changes,
                    ),
              settingsPreferences: _settingsPreferences,
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

bool _isApiKeyRouteProvider(String providerId) =>
    providerId == 'chatgpt_api' ||
    ApiCompatibleProviderKeyStore.providerIds.contains(providerId);

T? _firstOrNull<T>(Iterable<T> values) {
  final iterator = values.iterator;
  return iterator.moveNext() ? iterator.current : null;
}
