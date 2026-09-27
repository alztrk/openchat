import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:file_picker/file_picker.dart';

import '../../../app/zihora_theme.dart';
import '../../../app/zihora_toast.dart';
import '../../../l10n/zihora_localizations.dart';
import '../../../platform/windows/zihora_service_client.dart';
import '../../settings/data/chat_gpt_api_key_store.dart';
import '../../settings/presentation/settings_screen.dart';
import '../data/chat_repository.dart';
import '../domain/chat_conversation.dart';
import '../domain/chat_message.dart' as chat;
import '../domain/chatgpt_connection.dart';
import '../domain/chat_project.dart';
import '../domain/conversation_sidebar_data.dart';
import '../domain/history_storage_status.dart';
import 'widgets/chat_navigation_rail.dart';
import 'widgets/conversation_pane.dart';
import 'widgets/conversation_sidebar.dart';

class ChatScreen extends StatefulWidget {
  const ChatScreen({
    required this.themeMode,
    required this.onThemeModeChanged,
    required this.onToggleTheme,
    required this.historyStorageStatus,
    this.chatRepository,
    this.chatGptApiKeyStore,
    this.serviceClient,
    this.selectedModelLabel,
    super.key,
  });

  final ThemeMode themeMode;
  final Future<void> Function(ThemeMode) onThemeModeChanged;
  final Future<void> Function() onToggleTheme;
  final ChatRepository? chatRepository;
  final ChatGptApiKeyStore? chatGptApiKeyStore;
  final ZihoraServiceClient? serviceClient;
  final HistoryStorageStatus historyStorageStatus;
  final String? selectedModelLabel;

  @override
  State<ChatScreen> createState() => _ChatScreenState();
}

class _ChatScreenState extends State<ChatScreen> {
  final _scaffoldKey = GlobalKey<ScaffoldState>();
  final _searchController = TextEditingController();
  final _messageController = TextEditingController();
  bool _settingsOpen = false;
  bool _sidebarCollapsed = false;
  bool _isSending = false;
  bool _isLoadingConnections = false;
  bool _isLoadingModels = false;
  bool _isUpdatingConversationModel = false;
  String? _selectedConversationId;
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
  String? _loadedConnectionId;
  String? _loadedWorkspaceId;
  ZihoraServiceOperation? _activeChatOperation;
  Stream<List<ChatConversation>>? _conversationStream;
  Stream<List<ChatProject>>? _projectStream;
  Stream<List<chat.ChatMessage>>? _messageStream;

  @override
  void initState() {
    super.initState();
    _bindRepositoryStreams();
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
      return;
    }
    _conversationStream = repository.watchConversations();
    _projectStream = repository.watchProjects();
    final selectedId = _selectedConversationId;
    _messageStream = selectedId == null
        ? null
        : repository.watchMessages(selectedId);
  }

  void _selectConversation(String conversationId) {
    _modelSelectionGeneration++;
    setState(() {
      _selectedConversationId = conversationId;
      _messageStream = widget.chatRepository?.watchMessages(conversationId);
      _selectedModelId = null;
      _selectedReasoningEffort = null;
      _isUpdatingConversationModel = false;
      _titleEditRequestId = null;
    });
    unawaited(_loadConversationModels(conversationId));
  }

  Future<void> _loadProviderState() async {
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
        await _loadConversationModels(currentId);
      } else if (selectedConnection != null && selectedWorkspace != null) {
        await _loadModels(
          selectedConnection.id,
          selectedWorkspace.id,
          selectedModelId: null,
        );
      } else {
        _clearModels();
      }
    } on ZihoraServiceException catch (error) {
      _showServiceFailure(error);
    } on FormatException {
      if (mounted) _showMessage(context.zihoraL10n.chatGptDataUnavailable);
    } finally {
      if (mounted) setState(() => _isLoadingConnections = false);
    }
  }

  Future<void> _loadConversationModels(String conversationId) async {
    final repository = widget.chatRepository;
    if (repository == null) return;
    try {
      final conversation = await repository.getConversation(conversationId);
      if (!mounted || _selectedConversationId != conversationId) return;
      if (conversation?.connectionId case final String connectionId) {
        final workspaceId = conversation?.workspaceId;
        final modelId = conversation?.modelId;
        if (workspaceId == null || modelId == null) {
          throw StateError(
            'The conversation provider selection was incomplete.',
          );
        }
        if (_loadedConnectionId == connectionId &&
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
          );
        }
      } else if (_selectedConnectionId != null &&
          _selectedWorkspaceId != null) {
        await _loadModels(
          _selectedConnectionId!,
          _selectedWorkspaceId!,
          selectedModelId: conversation?.modelId,
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
      if (mounted) _showMessage(context.zihoraL10n.chatGptDataUnavailable);
    }
  }

  Future<void> _loadModels(
    String connectionId,
    String workspaceId, {
    required String? selectedModelId,
  }) async {
    final service = widget.serviceClient;
    if (service == null) return;
    final generation = ++_modelLoadGeneration;
    setState(() {
      _isLoadingModels = true;
      _modelLoadError = null;
      _modelFreshness = 'unavailable';
      _models = const <ChatGptModel>[];
      _selectedModelId = selectedModelId;
      _selectedReasoningEffort = null;
      _loadedConnectionId = connectionId;
      _loadedWorkspaceId = workspaceId;
    });
    try {
      final response = await service.call(
        'chatgpt.models.list',
        params: <String, Object?>{
          'connectionId': connectionId,
          'workspaceId': workspaceId,
        },
      );
      final rawModels = response['models'];
      if (rawModels is! List<Object?>) {
        throw const FormatException('The ChatGPT model list was invalid.');
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
    } on ZihoraServiceException catch (error) {
      if (!mounted || generation != _modelLoadGeneration) return;
      setState(() {
        _modelLoadError = error.code;
        _models = const <ChatGptModel>[];
      });
      _showServiceFailure(error);
    } on FormatException {
      if (!mounted || generation != _modelLoadGeneration) return;
      setState(() {
        _modelLoadError = 'invalid_response';
        _models = const <ChatGptModel>[];
      });
      if (mounted) _showMessage(context.zihoraL10n.chatGptDataUnavailable);
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
      _selectedModelId = null;
      _selectedReasoningEffort = null;
      _loadedConnectionId = null;
      _loadedWorkspaceId = null;
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
      if (conversation.connectionId == null) {
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
        showZihoraToast(
          context,
          context.zihoraL10n.conversationModelSaveFailed,
          type: ZihoraToastType.error,
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

  void _startNewConversation() {
    _modelSelectionGeneration++;
    setState(() {
      _selectedConversationId = null;
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
      _showMessage(context.zihoraL10n.themeSaveFailed);
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
    _messageController.clear();
    setState(() {
      _selectedConversationId = null;
      _messageStream = null;
      _isUpdatingConversationModel = false;
      _titleEditRequestId = null;
    });
  }

  Future<void> _sendMessage(ChatConversation? selectedConversation) async {
    if (_isSending || _isUpdatingConversationModel) return;
    final repository = widget.chatRepository;
    final service = widget.serviceClient;
    final text = _messageController.text.trim();
    if (repository == null || service == null || text.isEmpty) return;

    final connectionId =
        selectedConversation?.connectionId ?? _selectedConnectionId;
    final workspaceId =
        selectedConversation?.workspaceId ?? _selectedWorkspaceId;
    final modelId = _selectedModelId ?? selectedConversation?.modelId;
    if (connectionId == null || workspaceId == null || modelId == null) {
      _showMessage(context.zihoraL10n.modelRequired);
      return;
    }
    if (_loadedConnectionId != connectionId ||
        _loadedWorkspaceId != workspaceId ||
        !_models.any((model) => model.id == modelId && model.isAvailable)) {
      _showMessage(context.zihoraL10n.modelCatalogUnavailable);
      return;
    }

    final conversationId = selectedConversation?.id ?? _newLocalId();
    try {
      if (selectedConversation == null) {
        await repository.createConversation(
          id: conversationId,
          title: context.zihoraL10n.conversationTitle,
          createdAt: DateTime.now().toUtc(),
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
      } else if (selectedConversation.connectionId == null) {
        await repository.bindConversationProvider(
          conversationId: conversationId,
          connectionId: connectionId,
          workspaceId: workspaceId,
          modelId: modelId,
        );
      }
      await repository.saveMessage(
        conversationId: conversationId,
        message: chat.ChatMessage(
          id: _newLocalId(),
          role: chat.ChatMessageRole.user,
          content: text,
          createdAt: DateTime.now().toUtc(),
        ),
      );
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'chat_history',
          context: ErrorDescription('while saving a new chat message'),
        ),
      );
      if (mounted) _showMessage(context.zihoraL10n.messageSaveFailed);
      return;
    }

    _messageController.clear();
    if (mounted) setState(() => _isSending = true);
    String? assistantMessageId;
    String assistantContent = '';
    DateTime? assistantCreatedAt;
    var assistantReasoningSummaries = const <chat.ChatReasoningSummary>[];
    var persistence = Future<void>.value();
    StreamSubscription<ZihoraServiceEvent>? subscription;

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

    try {
      final operation = await service.startOperation(
        'chat.send',
        params: <String, Object?>{
          'conversationId': conversationId,
          if (_selectedReasoningEffort != null)
            'reasoningEffort': _selectedReasoningEffort,
        },
      );
      _activeChatOperation = operation;
      subscription = operation.events.listen(
        (event) {
          if (event.name != 'chat.started' &&
              event.name != 'chat.delta' &&
              event.name != 'chat.reasoning.delta') {
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
          final snapshot = chat.ChatMessage(
            id: messageId,
            role: chat.ChatMessageRole.assistant,
            content: content,
            createdAt: assistantCreatedAt,
            reasoningSummaries: assistantReasoningSummaries,
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
            status: status,
          ),
        );
      }
    } on ZihoraServiceException catch (error) {
      await persistence;
      final failedMessageId = assistantMessageId;
      if (failedMessageId != null) {
        await repository.saveMessage(
          conversationId: conversationId,
          message: chat.ChatMessage(
            id: failedMessageId,
            role: chat.ChatMessageRole.assistant,
            content: assistantContent,
            createdAt: assistantCreatedAt,
            reasoningSummaries: assistantReasoningSummaries,
            status: chat.ChatMessageStatus.failed,
          ),
        );
      }
      if (mounted) _showServiceFailure(error);
    } on Object catch (error, stackTrace) {
      await persistence;
      final failedMessageId = assistantMessageId;
      if (failedMessageId != null) {
        await repository.saveMessage(
          conversationId: conversationId,
          message: chat.ChatMessage(
            id: failedMessageId,
            role: chat.ChatMessageRole.assistant,
            content: assistantContent,
            createdAt: assistantCreatedAt,
            reasoningSummaries: assistantReasoningSummaries,
            status: chat.ChatMessageStatus.failed,
          ),
        );
      }
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'chat',
          context: ErrorDescription('while sending a ChatGPT message'),
        ),
      );
      if (mounted) _showMessage(context.zihoraL10n.chatRequestFailed);
    } finally {
      await subscription?.cancel();
      _activeChatOperation = null;
      if (mounted) setState(() => _isSending = false);
    }
  }

  void _stopMessage() {
    final operation = _activeChatOperation;
    if (operation != null) unawaited(operation.cancel());
  }

  void _showServiceFailure(ZihoraServiceException error) {
    final l10n = context.zihoraL10n;
    final message = switch (error.code) {
      'rate_limited' => l10n.chatGptRateLimited,
      'authentication_required' ||
      'refresh_rejected' => l10n.chatGptReauthenticationRequired,
      'provider_endpoint_unavailable' ||
      'invalid_provider_response' => l10n.chatGptProviderChanged,
      'model_unavailable' || 'conversation_not_routed' => l10n.modelRequired,
      _ => l10n.chatRequestFailed,
    };
    showZihoraToast(context, message, type: ZihoraToastType.error);
  }

  void _showMessage(String message) {
    if (!mounted) return;
    showZihoraToast(context, message, type: ZihoraToastType.error);
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
      if (mounted) _showMessage(context.zihoraL10n.chatHistoryUnavailable);
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
      if (mounted) _showMessage(context.zihoraL10n.projectMoveFailed);
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
      builder: (context) => const _CreateProjectDialog(),
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
        showZihoraToast(
          context,
          context.zihoraL10n.projectCreated,
          type: ZihoraToastType.success,
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
      if (mounted) _showMessage(context.zihoraL10n.projectCreateFailed);
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
      if (mounted) _showMessage(context.zihoraL10n.projectMoveFailed);
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
        showZihoraToast(
          context,
          context.zihoraL10n.conversationTitleSaveFailed,
          type: ZihoraToastType.error,
        );
      }
      rethrow;
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
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final palette = ZihoraPalette.of(context);
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
        final l10n = context.zihoraL10n;
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
                !_sidebarCollapsed &&
                constraints.maxWidth >= ZihoraSpacing.sidebarBreakpoint;
            final expandedRail =
                constraints.maxWidth >= ZihoraSpacing.expandedRailBreakpoint;
            final sidebarWidth =
                constraints.maxWidth >= ZihoraSpacing.fullSidebarBreakpoint
                ? ZihoraSpacing.sidebarWidth
                : ZihoraSpacing.compactSidebarWidth;
            final drawerWidth = constraints.maxWidth < sidebarWidth
                ? constraints.maxWidth
                : sidebarWidth;

            return Scaffold(
              key: _scaffoldKey,
              drawer:
                  _settingsOpen ||
                      constraints.maxWidth >= ZihoraSpacing.sidebarBreakpoint
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
                          onCollapse: () =>
                              setState(() => _sidebarCollapsed = true),
                        ),
                      ),
                    ),
              body: Row(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  ChatNavigationRail(
                    expanded: expandedRail,
                    settingsSelected: _settingsOpen,
                    onOpenChat: _startNewConversation,
                    onOpenSettings: () => setState(() => _settingsOpen = true),
                    onToggleTheme: _handleThemeToggle,
                  ),
                  if (_settingsOpen)
                    Expanded(
                      child: SettingsScreen(
                        themeMode: widget.themeMode,
                        onThemeModeChanged: widget.onThemeModeChanged,
                        historyStorageStatus: resolvedStorageStatus,
                        hasConversationHistory: conversations.isNotEmpty,
                        onClearConversationHistory:
                            widget.chatRepository == null
                            ? null
                            : _clearConversationHistory,
                        chatGptApiKeyStore: widget.chatGptApiKeyStore,
                        serviceClient: widget.serviceClient,
                        onProviderStateChanged: _loadProviderState,
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
                        onCollapse: () =>
                            setState(() => _sidebarCollapsed = true),
                      ),
                    Expanded(
                      child: _buildConversationPane(
                        selectedConversation: selectedConversation,
                      ),
                    ),
                  ],
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
    VoidCallback? onCollapse,
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
          onCollapse: onCollapse,
          searchController: _searchController,
          projects: sidebarProjects,
          projectsLoading:
              _projectStream != null &&
              !projectSnapshot.hasData &&
              !projectSnapshot.hasError,
          projectLoadError: projectSnapshot.hasError
              ? context.zihoraL10n.projectLoadFailed
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
    final routeConnectionId =
        selectedConversation?.connectionId ?? _selectedConnectionId;
    final routeWorkspaceId =
        selectedConversation?.workspaceId ?? _selectedWorkspaceId;
    final routeReady = routeConnectionId != null && routeWorkspaceId != null;
    final modelRouteReady =
        routeConnectionId == _loadedConnectionId &&
        routeWorkspaceId == _loadedWorkspaceId;
    final canSend =
        widget.historyStorageStatus == HistoryStorageStatus.available &&
        routeReady &&
        modelRouteReady &&
        selectedModel != null &&
        !_isUpdatingConversationModel &&
        !_isSending;
    final l10n = context.zihoraL10n;
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
        return ConversationPane(
          messageController: _messageController,
          showHistoryButton:
              _sidebarCollapsed ||
              MediaQuery.sizeOf(context).width <
                  ZihoraSpacing.sidebarBreakpoint,
          onOpenHistory: () {
            if (_sidebarCollapsed) {
              setState(() => _sidebarCollapsed = false);
              if (MediaQuery.sizeOf(context).width <
                  ZihoraSpacing.sidebarBreakpoint) {
                _scaffoldKey.currentState?.openDrawer();
              }
            } else {
              _scaffoldKey.currentState?.openDrawer();
            }
          },
          onSendMessage: () => unawaited(_sendMessage(selectedConversation)),
          onStopMessage: _stopMessage,
          canSendMessage: canSend,
          isSending: _isSending,
          models: modelOptions,
          selectedModelId: selectedModelId,
          onModelSelected: _isUpdatingConversationModel ? null : _selectModel,
          reasoningOptions: reasoningOptions,
          onReasoningSelected: _selectReasoning,
          selectedModelLabel: resolvedModelLabel,
          reasoningLevel: selectedReasoning,
          conversationTitle: selectedConversation?.title,
          conversationId: selectedConversation?.id,
          titleEditRequestId: _titleEditRequestId,
          onRenameConversation: _renameConversation,
          onConversationTitleEditFinished: _finishConversationTitleEdit,
          assistantModelLabel: modelLabel,
          messages: selectedConversation == null
              ? const <chat.ChatMessage>[]
              : messageSnapshot.data ?? const <chat.ChatMessage>[],
          messagesLoading:
              selectedConversation != null &&
              !messageSnapshot.hasData &&
              !messageSnapshot.hasError,
          messagesErrorDescription: messageSnapshot.hasError
              ? context.zihoraL10n.messageHistoryLoadFailed
              : null,
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

class _CreateProjectDialog extends StatefulWidget {
  const _CreateProjectDialog();

  @override
  State<_CreateProjectDialog> createState() => _CreateProjectDialogState();
}

class _CreateProjectDialogState extends State<_CreateProjectDialog> {
  final _nameController = TextEditingController();
  String? _folderPath;
  String? _errorMessage;
  bool _isSelectingFolder = false;

  @override
  void dispose() {
    _nameController.dispose();
    super.dispose();
  }

  Future<void> _selectFolder() async {
    if (_isSelectingFolder) return;
    setState(() {
      _isSelectingFolder = true;
      _errorMessage = null;
    });
    try {
      final folderPath = await FilePicker.getDirectoryPath(
        dialogTitle: context.zihoraL10n.chooseProjectFolder,
        windowsOptions: const WindowsOptions(lockParentWindow: true),
        linuxOptions: const LinuxOptions(lockParentWindow: true),
      );
      if (!mounted) return;
      if (folderPath != null && folderPath.trim().isNotEmpty) {
        setState(() => _folderPath = folderPath.trim());
      }
    } on Exception {
      if (mounted) {
        setState(
          () => _errorMessage = context.zihoraL10n.projectFolderSelectionFailed,
        );
      }
    } finally {
      if (mounted) setState(() => _isSelectingFolder = false);
    }
  }

  void _createProject() {
    final name = _nameController.text.trim();
    final folderPath = _folderPath;
    if (name.isEmpty) {
      setState(() => _errorMessage = context.zihoraL10n.projectNameRequired);
      return;
    }
    if (folderPath == null) {
      setState(
        () => _errorMessage = context.zihoraL10n.projectFolderNotSelected,
      );
      return;
    }
    Navigator.of(context).pop((name: name, folderPath: folderPath));
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.zihoraL10n;
    final palette = ZihoraPalette.of(context);

    return AlertDialog(
      title: Text(l10n.createProject),
      content: SizedBox(
        width: 420,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            TextField(
              controller: _nameController,
              autofocus: true,
              maxLines: 1,
              textInputAction: TextInputAction.next,
              onChanged: (_) {
                if (_errorMessage != null) {
                  setState(() => _errorMessage = null);
                }
              },
              decoration: InputDecoration(labelText: l10n.projectName),
            ),
            const SizedBox(height: 18),
            Text(
              l10n.projectFolder,
              style: Theme.of(context).textTheme.labelLarge,
            ),
            const SizedBox(height: 8),
            Container(
              constraints: const BoxConstraints(minHeight: 52),
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
              decoration: BoxDecoration(
                color: palette.composer,
                border: Border.all(color: palette.border),
                borderRadius: BorderRadius.circular(8),
              ),
              child: Row(
                children: [
                  Icon(
                    Icons.folder_outlined,
                    size: 18,
                    color: palette.secondaryIcon,
                  ),
                  const SizedBox(width: 9),
                  Expanded(
                    child: Text(
                      _folderPath ?? l10n.projectFolderNotSelected,
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: _folderPath == null
                            ? palette.secondaryText
                            : palette.text,
                        fontSize: 13,
                      ),
                    ),
                  ),
                ],
              ),
            ),
            const SizedBox(height: 10),
            OutlinedButton.icon(
              onPressed: _isSelectingFolder ? null : _selectFolder,
              icon: _isSelectingFolder
                  ? const SizedBox.square(
                      dimension: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.folder_open_outlined, size: 18),
              label: Text(l10n.chooseProjectFolder),
            ),
            if (_errorMessage case final String error) ...[
              const SizedBox(height: 8),
              Text(
                error,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ],
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: _isSelectingFolder
              ? null
              : () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          onPressed: _isSelectingFolder ? null : _createProject,
          child: Text(l10n.createProject),
        ),
      ],
    );
  }
}
