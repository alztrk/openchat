import 'dart:convert';

import 'package:drift/drift.dart';

import 'package:openchat/features/chat/data/chat_attachment_store.dart';
import 'package:openchat/features/chat/domain/chat_attachment.dart';
import 'package:openchat/features/chat/domain/chat_library_attachment.dart';
import 'package:openchat/features/chat/domain/chat_conversation.dart';
import 'package:openchat/features/chat/domain/chat_message.dart' as domain;
import 'package:openchat/features/chat/domain/model_favorite.dart';
import 'package:openchat/features/chat/domain/chat_project.dart';
import 'package:openchat/features/chat/domain/chat_saved_output.dart';
import 'package:openchat/features/chat/domain/chat_workspace.dart';
import 'package:openchat/features/chat/domain/local_library_search_result.dart';
import 'package:openchat/features/chat/domain/temporary_conversation_id.dart';

import 'package:openchat/features/chat/data/openchat_database.dart';
import 'package:openchat/features/settings/data/api_compatible_provider_key_store.dart';

const _localEngineProviderIds = <String>{'llama_cpp'};

typedef SavedOutputMessageKey = ({String conversationId, String messageId});

const maxSavedOutputVersionCharacters = 1000000;
const _attachmentLibraryMessagePageSize = 100;
const _maximumIndexedAttachmentBytes = 100 * 1024 * 1024;

bool _isLocalEngineProvider(String? providerId) =>
    providerId != null && _localEngineProviderIds.contains(providerId);

bool _isApiKeyRouteProvider(String? providerId) =>
    providerId == 'chatgpt_api' ||
    (providerId != null &&
        ApiCompatibleProviderKeyStore.isApiKeyProviderId(providerId));

T _readStoredEnum<T extends Enum>(
  List<T> values,
  String encoded, {
  required String fieldDescription,
}) {
  for (final value in values) {
    if (value.name == encoded) return value;
  }
  throw FormatException('Saved $fieldDescription value is invalid.');
}

List<String> _validatedPersonalMemoryFactIds(List<String> ids) {
  if (ids.length > 100 ||
      ids.any((id) => !RegExp(r'^[a-f0-9]{32}$').hasMatch(id)) ||
      ids.toSet().length != ids.length) {
    throw ArgumentError.value(
      ids,
      'ids',
      'Personal memory fact IDs are invalid.',
    );
  }
  return List<String>.unmodifiable(ids);
}

List<String> _personalMemoryFactIdsFromJson(Object? value) {
  if (value is! List<Object?> ||
      value.length > 100 ||
      value.any((item) => item is! String)) {
    throw const FormatException(
      'Saved personal memory references are invalid.',
    );
  }
  return _validatedPersonalMemoryFactIds(value.cast<String>());
}

class ChatRepository {
  const ChatRepository(this._database, {this.attachmentStore});

  final OpenChatDatabase _database;
  final ChatAttachmentStore? attachmentStore;

  bool get supportsAttachments => attachmentStore != null;

  Stream<List<ChatConversation>> watchConversations() {
    final query = _database.select(_database.conversations)
      ..orderBy([
        (conversation) => OrderingTerm.desc(conversation.isBookmarked),
        (conversation) => OrderingTerm.desc(conversation.isPinned),
        (conversation) => OrderingTerm.desc(conversation.updatedAt),
        (conversation) => OrderingTerm.asc(conversation.id),
      ]);

    return query.watch().map(
      (rows) => rows.map(_conversationFromRow).toList(growable: false),
    );
  }

  Stream<List<ChatConversation>> watchArchivedConversations() {
    final query = _database.select(_database.conversations)
      ..where((conversation) => conversation.isArchived.equals(true))
      ..orderBy([
        (conversation) => OrderingTerm.desc(conversation.updatedAt),
        (conversation) => OrderingTerm.asc(conversation.id),
      ]);

    return query.watch().map(
      (rows) => rows.map(_conversationFromRow).toList(growable: false),
    );
  }

  Stream<List<ChatProject>> watchProjects() {
    final query = _database.select(_database.projects)
      ..orderBy([
        (project) => OrderingTerm.asc(project.name),
        (project) => OrderingTerm.asc(project.id),
      ]);

    return query.watch().map(
      (rows) => rows
          .map(
            (row) => ChatProject(
              id: row.id,
              name: row.name,
              folderPath: row.folderPath,
              createdAt: DateTime.fromMillisecondsSinceEpoch(
                row.createdAt,
                isUtc: true,
              ),
              updatedAt: DateTime.fromMillisecondsSinceEpoch(
                row.updatedAt,
                isUtc: true,
              ),
            ),
          )
          .toList(growable: false),
    );
  }

  Stream<List<ChatWorkspace>> watchWorkspaces() {
    final query = _database.select(_database.workspaces)
      ..orderBy([
        (workspace) => OrderingTerm.asc(workspace.name),
        (workspace) => OrderingTerm.asc(workspace.id),
      ]);
    return query.watch().map(
      (rows) => rows
          .map(
            (row) => ChatWorkspace(
              id: row.id,
              name: row.name,
              createdAt: DateTime.fromMillisecondsSinceEpoch(
                row.createdAt,
                isUtc: true,
              ),
              updatedAt: DateTime.fromMillisecondsSinceEpoch(
                row.updatedAt,
                isUtc: true,
              ),
            ),
          )
          .toList(growable: false),
    );
  }

  Stream<List<ChatSavedOutput>> watchSavedOutputs() {
    final query = _database.select(_database.savedOutputs)
      ..orderBy([
        (output) => OrderingTerm.desc(output.savedAt),
        (output) => OrderingTerm.asc(output.conversationId),
        (output) => OrderingTerm.asc(output.messageId),
      ]);
    return query.watch().map(
      (rows) => rows
          .map(
            (row) => ChatSavedOutput(
              conversationId: row.conversationId,
              messageId: row.messageId,
              conversationTitle: row.conversationTitle,
              content: row.content,
              providerId: row.providerId,
              modelId: row.modelId,
              selectedVersion: row.selectedVersion,
              savedAt: DateTime.fromMillisecondsSinceEpoch(
                row.savedAt,
                isUtc: true,
              ),
            ),
          )
          .toList(growable: false),
    );
  }

  Future<List<LocalLibrarySearchResult>> searchLocalLibrary(
    String query, {
    int limit = 30,
  }) async {
    final normalizedQuery = query.trim();
    if (normalizedQuery.runes.length < 2 ||
        normalizedQuery.runes.length > 512 ||
        limit < 1 ||
        limit > 100) {
      throw ArgumentError.value(query, 'query', 'Invalid library search.');
    }

    final escapedQuery = normalizedQuery
        .replaceAll(r'\', r'\\')
        .replaceAll('%', r'\%')
        .replaceAll('_', r'\_');
    final pattern = '%$escapedQuery%';
    final results = <LocalLibrarySearchResult>[];
    final projects =
        await (_database.select(_database.projects)
              ..where((project) => project.name.like(pattern, escapeChar: r'\'))
              ..orderBy([(project) => OrderingTerm.desc(project.updatedAt)])
              ..limit(limit))
            .get();
    final latestConversationByProject = <String, String>{};
    if (projects.isNotEmpty) {
      final matchingConversations =
          await (_database.select(_database.conversations)
                ..where(
                  (conversation) => conversation.projectId.isIn(
                    projects.map((project) => project.id),
                  ),
                )
                ..orderBy([
                  (conversation) => OrderingTerm.desc(conversation.updatedAt),
                ])
                ..limit(limit * 10))
              .get();
      for (final conversation in matchingConversations) {
        final projectId = conversation.projectId;
        if (projectId != null) {
          latestConversationByProject.putIfAbsent(
            projectId,
            () => conversation.id,
          );
        }
      }
    }
    results.addAll(
      projects.map(
        (project) => LocalLibrarySearchResult(
          kind: LocalLibrarySearchResultKind.project,
          id: project.id,
          title: project.name,
          updatedAt: DateTime.fromMillisecondsSinceEpoch(
            project.updatedAt,
            isUtc: true,
          ),
          conversationId: latestConversationByProject[project.id],
        ),
      ),
    );
    final savedOutputs =
        await (_database.select(_database.savedOutputs)
              ..where(
                (output) =>
                    output.conversationTitle.like(pattern, escapeChar: r'\') |
                    output.content.like(pattern, escapeChar: r'\'),
              )
              ..orderBy([(output) => OrderingTerm.desc(output.savedAt)])
              ..limit(limit))
            .get();
    results.addAll(
      savedOutputs.map(
        (output) => LocalLibrarySearchResult(
          kind: LocalLibrarySearchResultKind.savedResponse,
          id: '${output.conversationId}:${output.messageId}',
          title: output.conversationTitle,
          subtitle: output.content,
          updatedAt: DateTime.fromMillisecondsSinceEpoch(
            output.savedAt,
            isUtc: true,
          ),
          conversationId: output.conversationId,
          messageId: output.messageId,
        ),
      ),
    );

    results.sort((left, right) => right.updatedAt.compareTo(left.updatedAt));
    return List<LocalLibrarySearchResult>.unmodifiable(results.take(limit));
  }

  Stream<List<ChatSavedOutputVersion>> watchSavedOutputVersions({
    required String conversationId,
    required String messageId,
  }) {
    final normalizedConversationId = _requireValue(
      conversationId,
      'conversationId',
    );
    final normalizedMessageId = _requireValue(messageId, 'messageId');
    final query = _database.select(_database.savedOutputVersions)
      ..where(
        (version) =>
            version.conversationId.equals(normalizedConversationId) &
            version.messageId.equals(normalizedMessageId),
      )
      ..orderBy([(version) => OrderingTerm.asc(version.versionNumber)]);
    return query.watch().map(
      (rows) => rows
          .map(
            (row) => ChatSavedOutputVersion(
              conversationId: row.conversationId,
              messageId: row.messageId,
              versionNumber: row.versionNumber,
              content: row.content,
              createdAt: DateTime.fromMillisecondsSinceEpoch(
                row.createdAt,
                isUtc: true,
              ),
            ),
          )
          .toList(growable: false),
    );
  }

  Stream<Set<SavedOutputMessageKey>> watchSavedOutputMessageKeys() {
    final conversationId = _database.savedOutputs.conversationId;
    final messageId = _database.savedOutputs.messageId;
    final query = _database.selectOnly(_database.savedOutputs)
      ..addColumns([conversationId, messageId]);

    return query.watch().map((rows) {
      return rows.map((row) {
        final savedConversationId = row.read(conversationId);
        final savedMessageId = row.read(messageId);
        if (savedConversationId == null || savedMessageId == null) {
          throw StateError('A saved output is missing its message key.');
        }
        return (conversationId: savedConversationId, messageId: savedMessageId);
      }).toSet();
    });
  }

  Future<void> saveOutput({
    required String conversationId,
    required String messageId,
  }) async {
    final normalizedConversationId = _requireValue(
      conversationId,
      'conversationId',
    );
    final normalizedMessageId = _requireValue(messageId, 'messageId');
    await _database.transaction(() async {
      final conversation =
          await (_database.select(_database.conversations)
                ..where((row) => row.id.equals(normalizedConversationId)))
              .getSingleOrNull();
      if (conversation == null) {
        throw ConversationNotFoundException(normalizedConversationId);
      }
      final message =
          await (_database.select(_database.messages)..where(
                (row) =>
                    row.conversationId.equals(normalizedConversationId) &
                    row.id.equals(normalizedMessageId),
              ))
              .getSingleOrNull();
      if (message == null) {
        throw MessageNotFoundException(
          normalizedConversationId,
          normalizedMessageId,
        );
      }
      if (message.role != domain.ChatMessageRole.assistant.name ||
          message.status != domain.ChatMessageStatus.completed.name ||
          message.content.trim().isEmpty) {
        throw const InvalidOutputSaveException();
      }

      final existingOutput =
          await (_database.select(_database.savedOutputs)..where(
                (row) =>
                    row.conversationId.equals(normalizedConversationId) &
                    row.messageId.equals(normalizedMessageId),
              ))
              .getSingleOrNull();
      if (existingOutput != null) return;

      final savedAt = DateTime.now().toUtc().millisecondsSinceEpoch;
      await _database
          .into(_database.savedOutputs)
          .insert(
            SavedOutputsCompanion.insert(
              conversationId: normalizedConversationId,
              messageId: normalizedMessageId,
              conversationTitle: conversation.title,
              content: message.content,
              providerId: Value(message.providerId),
              modelId: Value(message.modelId),
              savedAt: savedAt,
            ),
          );
      await _database
          .into(_database.savedOutputVersions)
          .insert(
            SavedOutputVersionsCompanion.insert(
              conversationId: normalizedConversationId,
              messageId: normalizedMessageId,
              versionNumber: 1,
              content: message.content,
              createdAt: savedAt,
            ),
          );
    });
  }

  Future<int> createSavedOutputVersion({
    required String conversationId,
    required String messageId,
    required String content,
  }) async {
    final normalizedConversationId = _requireValue(
      conversationId,
      'conversationId',
    );
    final normalizedMessageId = _requireValue(messageId, 'messageId');
    if (content.trim().isEmpty ||
        content.length > maxSavedOutputVersionCharacters) {
      throw const InvalidSavedOutputVersionException();
    }
    final normalizedContent = content;

    return _database.transaction(() async {
      final savedOutput =
          await (_database.select(_database.savedOutputs)..where(
                (row) =>
                    row.conversationId.equals(normalizedConversationId) &
                    row.messageId.equals(normalizedMessageId),
              ))
              .getSingleOrNull();
      if (savedOutput == null) {
        throw SavedOutputNotFoundException(
          normalizedConversationId,
          normalizedMessageId,
        );
      }

      final latestVersion =
          await (_database.select(_database.savedOutputVersions)
                ..where(
                  (row) =>
                      row.conversationId.equals(normalizedConversationId) &
                      row.messageId.equals(normalizedMessageId),
                )
                ..orderBy([(row) => OrderingTerm.desc(row.versionNumber)])
                ..limit(1))
              .getSingleOrNull();
      final versionNumber = (latestVersion?.versionNumber ?? 0) + 1;
      final savedAt = DateTime.now().toUtc().millisecondsSinceEpoch;
      await _database
          .into(_database.savedOutputVersions)
          .insert(
            SavedOutputVersionsCompanion.insert(
              conversationId: normalizedConversationId,
              messageId: normalizedMessageId,
              versionNumber: versionNumber,
              content: normalizedContent,
              createdAt: savedAt,
            ),
          );
      final updatedRows =
          await (_database.update(_database.savedOutputs)..where(
                (row) =>
                    row.conversationId.equals(normalizedConversationId) &
                    row.messageId.equals(normalizedMessageId),
              ))
              .write(
                SavedOutputsCompanion(
                  content: Value(normalizedContent),
                  selectedVersion: Value(versionNumber),
                  savedAt: Value(savedAt),
                ),
              );
      if (updatedRows != 1) {
        throw SavedOutputNotFoundException(
          normalizedConversationId,
          normalizedMessageId,
        );
      }
      return versionNumber;
    });
  }

  Future<void> selectSavedOutputVersion({
    required String conversationId,
    required String messageId,
    required int versionNumber,
  }) async {
    final normalizedConversationId = _requireValue(
      conversationId,
      'conversationId',
    );
    final normalizedMessageId = _requireValue(messageId, 'messageId');
    if (versionNumber < 1) {
      throw const InvalidSavedOutputVersionException();
    }
    await _database.transaction(() async {
      final version =
          await (_database.select(_database.savedOutputVersions)..where(
                (row) =>
                    row.conversationId.equals(normalizedConversationId) &
                    row.messageId.equals(normalizedMessageId) &
                    row.versionNumber.equals(versionNumber),
              ))
              .getSingleOrNull();
      if (version == null) {
        throw const InvalidSavedOutputVersionException();
      }
      final updatedRows =
          await (_database.update(_database.savedOutputs)..where(
                (row) =>
                    row.conversationId.equals(normalizedConversationId) &
                    row.messageId.equals(normalizedMessageId),
              ))
              .write(
                SavedOutputsCompanion(
                  content: Value(version.content),
                  selectedVersion: Value(versionNumber),
                  savedAt: Value(DateTime.now().toUtc().millisecondsSinceEpoch),
                ),
              );
      if (updatedRows != 1) {
        throw SavedOutputNotFoundException(
          normalizedConversationId,
          normalizedMessageId,
        );
      }
    });
  }

  Future<void> removeSavedOutput({
    required String conversationId,
    required String messageId,
  }) async {
    final normalizedConversationId = _requireValue(
      conversationId,
      'conversationId',
    );
    final normalizedMessageId = _requireValue(messageId, 'messageId');
    await _database.transaction(() async {
      await (_database.delete(_database.savedOutputVersions)..where(
            (row) =>
                row.conversationId.equals(normalizedConversationId) &
                row.messageId.equals(normalizedMessageId),
          ))
          .go();
      await (_database.delete(_database.savedOutputs)..where(
            (row) =>
                row.conversationId.equals(normalizedConversationId) &
                row.messageId.equals(normalizedMessageId),
          ))
          .go();
    });
  }

  Future<void> createWorkspace({
    required String id,
    required String name,
    required DateTime createdAt,
  }) async {
    final normalizedId = _requireValue(id, 'id');
    final normalizedName = _requireWorkspaceName(name);
    final timestamp = createdAt.toUtc().millisecondsSinceEpoch;
    await _database
        .into(_database.workspaces)
        .insert(
          WorkspacesCompanion.insert(
            id: normalizedId,
            name: normalizedName,
            createdAt: timestamp,
            updatedAt: timestamp,
          ),
        );
  }

  Future<void> renameWorkspace({
    required String workspaceId,
    required String name,
  }) async {
    final normalizedId = _requireValue(workspaceId, 'workspaceId');
    final normalizedName = _requireWorkspaceName(name);
    await _database.transaction(() async {
      final workspace = await (_database.select(
        _database.workspaces,
      )..where((row) => row.id.equals(normalizedId))).getSingleOrNull();
      if (workspace == null) throw WorkspaceNotFoundException(normalizedId);
      await (_database.update(
        _database.workspaces,
      )..where((row) => row.id.equals(normalizedId))).write(
        WorkspacesCompanion(
          name: Value(normalizedName),
          updatedAt: Value(DateTime.now().toUtc().millisecondsSinceEpoch),
        ),
      );
    });
  }

  Future<void> deleteWorkspace(String workspaceId) async {
    final normalizedId = _requireValue(workspaceId, 'workspaceId');
    final deletedRows = await (_database.delete(
      _database.workspaces,
    )..where((row) => row.id.equals(normalizedId))).go();
    if (deletedRows == 0) throw WorkspaceNotFoundException(normalizedId);
  }

  Future<void> setConversationWorkspace({
    required String conversationId,
    required String? workspaceId,
  }) async {
    final normalizedConversationId = _requireValue(
      conversationId,
      'conversationId',
    );
    final normalizedWorkspaceId = workspaceId == null
        ? null
        : _requireValue(workspaceId, 'workspaceId');
    await _database.transaction(() async {
      final conversation =
          await (_database.select(_database.conversations)
                ..where((row) => row.id.equals(normalizedConversationId)))
              .getSingleOrNull();
      if (conversation == null) {
        throw ConversationNotFoundException(normalizedConversationId);
      }
      if (normalizedWorkspaceId != null) {
        final workspace =
            await (_database.select(_database.workspaces)
                  ..where((row) => row.id.equals(normalizedWorkspaceId)))
                .getSingleOrNull();
        if (workspace == null) {
          throw WorkspaceNotFoundException(normalizedWorkspaceId);
        }
      }
      if (conversation.productWorkspaceId == normalizedWorkspaceId) return;
      await (_database.update(
        _database.conversations,
      )..where((row) => row.id.equals(normalizedConversationId))).write(
        ConversationsCompanion(
          productWorkspaceId: Value(normalizedWorkspaceId),
          updatedAt: Value(DateTime.now().toUtc().millisecondsSinceEpoch),
        ),
      );
    });
  }

  Stream<List<FavoriteModel>> watchModelFavorites() {
    final query = _database.select(_database.modelFavorites)
      ..orderBy([
        (favorite) => OrderingTerm.asc(favorite.providerId),
        (favorite) => OrderingTerm.asc(favorite.displayName),
        (favorite) => OrderingTerm.asc(favorite.modelId),
      ]);

    return query.watch().map(
      (rows) => rows
          .map(
            (row) => FavoriteModel(
              providerId: row.providerId,
              modelId: row.modelId,
              sourceConnectionId: row.sourceConnectionId,
              displayName: row.displayName,
            ),
          )
          .toList(growable: false),
    );
  }

  Future<void> setModelFavorite({
    required String providerId,
    required String modelId,
    required String displayName,
    required bool isFavorite,
    String? sourceConnectionId,
  }) async {
    if (providerId != 'chatgpt' &&
        providerId != 'chatgpt_api' &&
        providerId != 'opencode' &&
        !ApiCompatibleProviderKeyStore.isApiKeyProviderId(providerId) &&
        !_isLocalEngineProvider(providerId)) {
      throw ArgumentError.value(providerId, 'providerId', 'Unknown provider.');
    }
    final normalizedModelId = _requireValue(modelId, 'modelId');
    final normalizedDisplayName = _requireValue(displayName, 'displayName');
    final normalizedSourceConnectionId = _isApiKeyRouteProvider(providerId)
        ? _requireValue(sourceConnectionId ?? '', 'sourceConnectionId')
        : null;
    final query = _database.delete(_database.modelFavorites)
      ..where(
        (favorite) =>
            favorite.providerId.equals(providerId) &
            favorite.modelId.equals(normalizedModelId),
      );

    if (!isFavorite) {
      await query.go();
      return;
    }

    await _database
        .into(_database.modelFavorites)
        .insertOnConflictUpdate(
          ModelFavoritesCompanion.insert(
            providerId: providerId,
            modelId: normalizedModelId,
            sourceConnectionId: Value(normalizedSourceConnectionId),
            displayName: normalizedDisplayName,
            favoritedAt: DateTime.now().toUtc().millisecondsSinceEpoch,
          ),
        );
  }

  Future<void> createProject({
    required String id,
    required String name,
    required String folderPath,
    required DateTime createdAt,
  }) async {
    final normalizedId = _requireValue(id, 'id');
    final normalizedName = _requireValue(name, 'name');
    final normalizedFolderPath = _requireValue(folderPath, 'folderPath');
    final timestamp = createdAt.toUtc().millisecondsSinceEpoch;

    await _database
        .into(_database.projects)
        .insert(
          ProjectsCompanion.insert(
            id: normalizedId,
            name: normalizedName,
            folderPath: normalizedFolderPath,
            createdAt: timestamp,
            updatedAt: timestamp,
          ),
        );
  }

  Future<void> moveConversationToProject({
    required String conversationId,
    required String projectId,
  }) async {
    final normalizedProjectId = _requireValue(projectId, 'projectId');

    await _database.transaction(() async {
      final conversation = await (_database.select(
        _database.conversations,
      )..where((row) => row.id.equals(conversationId))).getSingleOrNull();
      if (conversation == null) {
        throw ConversationNotFoundException(conversationId);
      }

      final project = await (_database.select(
        _database.projects,
      )..where((row) => row.id.equals(normalizedProjectId))).getSingleOrNull();
      if (project == null) {
        throw ProjectNotFoundException(normalizedProjectId);
      }
      if (conversation.projectId == normalizedProjectId) return;

      await (_database.update(
        _database.conversations,
      )..where((row) => row.id.equals(conversationId))).write(
        ConversationsCompanion(
          projectId: Value(normalizedProjectId),
          isPinned: const Value(false),
        ),
      );
    });
  }

  Future<void> moveConversations({
    required List<String> conversationIds,
    required String? projectId,
  }) async {
    final ids = conversationIds.map((id) => id.trim()).toSet();
    if (ids.isEmpty || ids.any((id) => id.isEmpty)) {
      throw ArgumentError.value(
        conversationIds,
        'conversationIds',
        'At least one conversation ID is required.',
      );
    }
    final normalizedProjectId = projectId == null
        ? null
        : _requireValue(projectId, 'projectId');
    await _database.transaction(() async {
      if (normalizedProjectId != null) {
        final project =
            await (_database.select(_database.projects)
                  ..where((row) => row.id.equals(normalizedProjectId)))
                .getSingleOrNull();
        if (project == null) {
          throw ProjectNotFoundException(normalizedProjectId);
        }
      }
      final orderedIds = ids.toList(growable: false);
      final existingIds = <String>{};
      for (var offset = 0; offset < orderedIds.length; offset += 500) {
        final chunk = orderedIds.skip(offset).take(500).toSet();
        existingIds.addAll(
          await (_database.select(
            _database.conversations,
          )..where((row) => row.id.isIn(chunk))).map((row) => row.id).get(),
        );
      }
      final missingIds = ids.difference(existingIds);
      if (missingIds.isNotEmpty) {
        throw ConversationNotFoundException(missingIds.first);
      }
      for (var offset = 0; offset < orderedIds.length; offset += 500) {
        final chunk = orderedIds.skip(offset).take(500).toSet();
        await (_database.update(
          _database.conversations,
        )..where((row) => row.id.isIn(chunk))).write(
          ConversationsCompanion(
            projectId: Value(normalizedProjectId),
            isPinned: const Value(false),
          ),
        );
      }
    });
  }

  Future<void> moveConversationToChats(String conversationId) async {
    final updatedRows =
        await (_database.update(_database.conversations)
              ..where((conversation) => conversation.id.equals(conversationId)))
            .write(
              const ConversationsCompanion(
                projectId: Value(null),
                isPinned: Value(false),
              ),
            );
    if (updatedRows == 0) {
      throw ConversationNotFoundException(conversationId);
    }
  }

  Future<void> clearConversationProviderForConnection(
    String connectionId,
  ) async {
    final normalizedConnectionId = _requireValue(connectionId, 'connectionId');
    await (_database.update(
      _database.conversations,
    )..where((row) => row.connectionId.equals(normalizedConnectionId))).write(
      const ConversationsCompanion(
        connectionId: Value(null),
        workspaceId: Value(null),
      ),
    );
  }

  Future<ChatConversation?> getConversation(String conversationId) async {
    final row =
        await (_database.select(_database.conversations)
              ..where((conversation) => conversation.id.equals(conversationId)))
            .getSingleOrNull();
    return row == null ? null : _conversationFromRow(row);
  }

  Future<bool> hasCompletedAssistantResponse() async {
    final query = _database.selectOnly(_database.messages)
      ..addColumns([_database.messages.id])
      ..where(
        _database.messages.role.equals('assistant') &
            _database.messages.status.equals('completed'),
      )
      ..limit(1);
    return await query.getSingleOrNull() != null;
  }

  Future<ChatConversation> createBranchFromUserMessage({
    required String sourceConversationId,
    required String throughUserMessageId,
    required String branchId,
    required String branchTitle,
    required DateTime createdAt,
    required String editedUserMessage,
  }) async {
    final sourceRow =
        await (_database.select(_database.conversations)..where(
              (conversation) => conversation.id.equals(sourceConversationId),
            ))
            .getSingleOrNull();
    if (sourceRow == null) {
      throw ConversationNotFoundException(sourceConversationId);
    }
    final sourceMessages = await getMessages(sourceConversationId);
    final targetIndex = sourceMessages.indexWhere(
      (message) => message.id == throughUserMessageId,
    );
    if (targetIndex < 0 ||
        sourceMessages[targetIndex].role != domain.ChatMessageRole.user) {
      throw MessageNotFoundException(
        sourceConversationId,
        throughUserMessageId,
      );
    }
    if (editedUserMessage.trim().isEmpty &&
        sourceMessages[targetIndex].attachments.isEmpty) {
      throw ArgumentError.value(
        editedUserMessage,
        'editedUserMessage',
        'A branched user message must contain text or an attachment.',
      );
    }

    final normalizedBranchId = _requireValue(branchId, 'branchId');
    if (normalizedBranchId.length > 96 ||
        !_isValidBranchIdentifier(normalizedBranchId)) {
      throw ArgumentError.value(branchId, 'branchId', 'Invalid branch ID.');
    }
    final branchMessages = sourceMessages.take(targetIndex + 1).toList();
    await createConversation(
      id: normalizedBranchId,
      title: branchTitle,
      createdAt: createdAt,
      connectionId: sourceRow.connectionId,
      workspaceId: sourceRow.workspaceId,
      productWorkspaceId: sourceRow.productWorkspaceId,
      apiKeyConnectionId: sourceRow.apiKeyConnectionId,
      providerId: sourceRow.providerId,
      modelId: sourceRow.modelId,
      projectId: sourceRow.projectId,
    );

    try {
      for (var index = 0; index < branchMessages.length; index++) {
        final sourceMessage = branchMessages[index];
        final targetMessageId = '${normalizedBranchId}_m$index';
        final attachments = sourceMessage.attachments;
        if (attachments.isNotEmpty) {
          final store = attachmentStore;
          if (store == null) {
            throw const ChatAttachmentStorageException(
              'Attachments are unavailable in this storage location.',
            );
          }
          await store.copyMessageAttachments(
            sourceConversationId: sourceConversationId,
            sourceMessageId: sourceMessage.id,
            targetConversationId: normalizedBranchId,
            targetMessageId: targetMessageId,
            expectedAttachments: attachments,
          );
        }
        await saveMessage(
          conversationId: normalizedBranchId,
          attachmentsAlreadyCopied: attachments.isNotEmpty,
          message: domain.ChatMessage(
            id: targetMessageId,
            role: sourceMessage.role,
            content: index == targetIndex
                ? editedUserMessage
                : sourceMessage.content,
            attachments: attachments,
            createdAt: sourceMessage.createdAt,
            outputTokens: sourceMessage.outputTokens,
            tokensPerSecond: sourceMessage.tokensPerSecond,
            elapsed: sourceMessage.elapsed,
            providerId: sourceMessage.providerId,
            modelId: sourceMessage.modelId,
            citationSources: sourceMessage.citationSources,
            personalMemoryFactIds: sourceMessage.personalMemoryFactIds,
            reasoningSummaries: sourceMessage.reasoningSummaries,
            toolActivities: sourceMessage.toolActivities,
            status: sourceMessage.status,
            failureCode: sourceMessage.failureCode,
          ),
        );
      }
    } on Object catch (error, stackTrace) {
      try {
        await deleteConversation(normalizedBranchId);
      } on Object catch (cleanupError) {
        Error.throwWithStackTrace(
          ConversationBranchCleanupException(error, cleanupError),
          stackTrace,
        );
      }
      Error.throwWithStackTrace(error, stackTrace);
    }

    final branch = await getConversation(normalizedBranchId);
    if (branch == null) {
      throw ConversationNotFoundException(normalizedBranchId);
    }
    return branch;
  }

  Stream<List<domain.ChatMessage>> watchMessages(String conversationId) {
    final query = _database.select(_database.messages)
      ..where((message) => message.conversationId.equals(conversationId))
      ..orderBy([
        (message) => OrderingTerm.asc(message.createdAt),
        (message) => OrderingTerm.asc(message.id),
      ]);

    return query.watch().asyncMap(_messagesFromRows);
  }

  Future<List<ChatLibraryAttachment>> loadAttachmentLibrary({
    int limit = 100,
  }) async {
    if (limit < 1 || limit > 200) {
      throw ArgumentError.value(limit, 'limit', 'Invalid attachment limit.');
    }
    final page = await loadAttachmentLibraryPage(attachmentLimit: limit);
    return page.attachments;
  }

  Future<ChatLibraryAttachmentPage> loadAttachmentLibraryPage({
    int messageOffset = 0,
    int attachmentLimit = 100,
  }) async {
    if (messageOffset < 0 ||
        messageOffset > 100000 ||
        attachmentLimit < 1 ||
        attachmentLimit > 1000) {
      throw ArgumentError('Invalid attachment library page.');
    }
    final attachmentStore = this.attachmentStore;
    if (attachmentStore == null) {
      return const ChatLibraryAttachmentPage(
        attachments: <ChatLibraryAttachment>[],
        nextMessageOffset: null,
      );
    }

    final messageRows =
        await (_database.select(_database.messages)
              ..where(
                (message) => message.content.like(
                  '${ChatMessageContentCodec.metadataPrefix}%',
                ),
              )
              ..orderBy([
                (message) => OrderingTerm.desc(message.createdAt),
                (message) => OrderingTerm.desc(message.id),
              ])
              ..limit(_attachmentLibraryMessagePageSize, offset: messageOffset))
            .get();
    final conversationIds = messageRows
        .map((message) => message.conversationId)
        .toSet();
    if (conversationIds.isEmpty) {
      return const ChatLibraryAttachmentPage(
        attachments: <ChatLibraryAttachment>[],
        nextMessageOffset: null,
      );
    }

    final conversationRows = await (_database.select(
      _database.conversations,
    )..where((conversation) => conversation.id.isIn(conversationIds))).get();
    final conversationTitles = <String, String>{
      for (final conversation in conversationRows)
        conversation.id: conversation.title,
    };
    final results = <ChatLibraryAttachment>[];
    for (
      var messageIndex = 0;
      messageIndex < messageRows.length;
      messageIndex++
    ) {
      final message = messageRows[messageIndex];
      final conversationTitle = conversationTitles[message.conversationId];
      if (conversationTitle == null) continue;
      final metadata = ChatMessageContentCodec.decode(message.content);
      if (metadata.attachments.isEmpty) continue;
      final availableAttachments = await attachmentStore.readMessageAttachments(
        conversationId: message.conversationId,
        messageId: message.id,
        expectedAttachments: metadata.attachments,
      );
      final createdAt = DateTime.fromMillisecondsSinceEpoch(
        message.createdAt ?? 0,
        isUtc: true,
      );
      for (final attachment in availableAttachments) {
        results.add(
          ChatLibraryAttachment(
            conversationId: message.conversationId,
            conversationTitle: conversationTitle,
            messageId: message.id,
            attachment: ChatAttachment(
              id: attachment.id,
              name: attachment.name,
              mimeType: attachment.mimeType,
              sizeBytes: attachment.sizeBytes,
              kind: attachment.kind,
              isAvailable: attachment.isAvailable,
            ),
            createdAt: createdAt,
          ),
        );
      }
      if (results.length >= attachmentLimit) {
        final hasBufferedMessages = messageIndex + 1 < messageRows.length;
        final hasNextPage =
            hasBufferedMessages ||
            messageRows.length == _attachmentLibraryMessagePageSize;
        return ChatLibraryAttachmentPage(
          attachments: List<ChatLibraryAttachment>.unmodifiable(results),
          nextMessageOffset: hasNextPage
              ? messageOffset + messageIndex + 1
              : null,
        );
      }
    }
    return ChatLibraryAttachmentPage(
      attachments: List<ChatLibraryAttachment>.unmodifiable(results),
      nextMessageOffset: messageRows.length == _attachmentLibraryMessagePageSize
          ? messageOffset + messageRows.length
          : null,
    );
  }

  Future<int> rebuildAttachmentTextIndex({
    int maximumAttachments = 5000,
  }) async {
    if (maximumAttachments < 1 || maximumAttachments > 5000) {
      throw ArgumentError.value(
        maximumAttachments,
        'maximumAttachments',
        'Invalid attachment index limit.',
      );
    }
    final store = attachmentStore;
    if (store == null) return 0;
    await _database.customStatement(
      'DROP TABLE IF EXISTS attachment_text_index_staging',
    );
    await _database.customStatement('''
      CREATE TABLE attachment_text_index_staging (
        conversation_id TEXT NOT NULL,
        message_id TEXT NOT NULL,
        attachment_id TEXT NOT NULL,
        attachment_name TEXT NOT NULL,
        content TEXT NOT NULL,
        indexed_at INTEGER NOT NULL,
        PRIMARY KEY (conversation_id, message_id, attachment_id)
      )
    ''');
    var indexedCount = 0;
    var indexedBytes = 0;
    var messageOffset = 0;
    try {
      while (indexedCount < maximumAttachments) {
        final page = await loadAttachmentLibraryPage(
          messageOffset: messageOffset,
          attachmentLimit: 1000,
        );
        for (final item in page.attachments) {
          if (indexedCount >= maximumAttachments) break;
          final attachment = item.attachment;
          if (!attachment.isAvailable ||
              attachment.kind != ChatAttachmentKind.text ||
              !_isIndexableTextAttachment(attachment.name)) {
            continue;
          }
          final bytes = await store.readReusableAttachmentBytes(
            conversationId: item.conversationId,
            messageId: item.messageId,
            attachment: attachment,
          );
          if (indexedBytes + bytes.length > _maximumIndexedAttachmentBytes) {
            continue;
          }
          final content = const Utf8Decoder(allowMalformed: false)
              .convert(bytes);
          await _database.customStatement(
            'INSERT OR REPLACE INTO attachment_text_index_staging '
            '(conversation_id, message_id, attachment_id, attachment_name, content, indexed_at) '
            'VALUES (?, ?, ?, ?, ?, ?)',
            [
              item.conversationId,
              item.messageId,
              attachment.id,
              attachment.name,
              content,
              item.createdAt.millisecondsSinceEpoch,
            ],
          );
          indexedCount++;
          indexedBytes += bytes.length;
        }
        final nextOffset = page.nextMessageOffset;
        if (nextOffset == null || nextOffset <= messageOffset) break;
        messageOffset = nextOffset;
      }
      await _database.transaction(() async {
        await _database.customStatement('DELETE FROM attachment_text_index');
        await _database.customStatement(
          'INSERT INTO attachment_text_index SELECT * FROM attachment_text_index_staging',
        );
        await _database.customStatement(
          'DROP TABLE attachment_text_index_staging',
        );
      });
      return indexedCount;
    } on Object {
      await _database.customStatement(
        'DROP TABLE IF EXISTS attachment_text_index_staging',
      );
      rethrow;
    }
  }

  Future<int> attachmentTextIndexCount() async {
    final row = await _database
        .customSelect(
          'SELECT COUNT(*) AS item_count FROM attachment_text_index',
        )
        .getSingle();
    return row.read<int>('item_count');
  }

  Future<void> clearAttachmentTextIndex() =>
      _database.customStatement('DELETE FROM attachment_text_index');

  bool _isIndexableTextAttachment(String name) {
    final separator = name.lastIndexOf('.');
    if (separator < 0 || separator == name.length - 1) return false;
    return ChatAttachmentTypes.textExtensions.contains(
      name.substring(separator + 1).toLowerCase(),
    );
  }

  Future<ChatAttachment> loadAttachmentFromLibrary(
    ChatLibraryAttachment libraryAttachment,
  ) async {
    final attachmentStore = this.attachmentStore;
    if (attachmentStore == null) {
      throw const ChatAttachmentStorageException(
        'The local attachment library is unavailable.',
      );
    }
    final row =
        await (_database.select(_database.messages)..where(
              (message) =>
                  message.conversationId.equals(
                    libraryAttachment.conversationId,
                  ) &
                  message.id.equals(libraryAttachment.messageId),
            ))
            .getSingleOrNull();
    if (row == null) {
      throw const ChatAttachmentStorageException(
        'The saved attachment is no longer available.',
      );
    }
    final metadata = ChatMessageContentCodec.decode(row.content);
    ChatAttachment? expectedAttachment;
    for (final attachment in metadata.attachments) {
      if (attachment.id == libraryAttachment.attachment.id) {
        expectedAttachment = attachment;
        break;
      }
    }
    if (expectedAttachment == null) {
      throw const ChatAttachmentStorageException(
        'The saved attachment is no longer available.',
      );
    }
    final bytes = await attachmentStore.readReusableAttachmentBytes(
      conversationId: libraryAttachment.conversationId,
      messageId: libraryAttachment.messageId,
      attachment: expectedAttachment,
    );
    return ChatAttachment(
      id: expectedAttachment.id,
      name: expectedAttachment.name,
      mimeType: expectedAttachment.mimeType,
      sizeBytes: expectedAttachment.sizeBytes,
      kind: expectedAttachment.kind,
      bytes: bytes,
    );
  }

  Future<List<domain.ChatMessage>> getMessages(String conversationId) async {
    final query = _database.select(_database.messages)
      ..where((message) => message.conversationId.equals(conversationId))
      ..orderBy([
        (message) => OrderingTerm.asc(message.createdAt),
        (message) => OrderingTerm.asc(message.id),
      ]);
    final rows = await query.get();
    return _messagesFromRows(rows);
  }

  Future<void> createConversation({
    required String id,
    required String title,
    required DateTime createdAt,
    String? connectionId,
    String? workspaceId,
    String? productWorkspaceId,
    String? apiKeyConnectionId,
    String? providerId,
    String? modelId,
    String? projectId,
  }) async {
    requireDurableConversationId(id);
    final normalizedId = id.trim();
    final normalizedTitle = title.trim();
    if (normalizedId.isEmpty) {
      throw ArgumentError.value(id, 'id', 'Conversation ID cannot be empty.');
    }
    if (normalizedTitle.isEmpty) {
      throw ArgumentError.value(
        title,
        'title',
        'Conversation title cannot be empty.',
      );
    }
    final selectedProvider = providerId?.trim();
    final isOpenCode = selectedProvider == 'opencode';
    final isLocalEngine = _isLocalEngineProvider(selectedProvider);
    final hasApiKeyRoute = _isApiKeyRouteProvider(selectedProvider);
    if (selectedProvider != null &&
        selectedProvider != 'chatgpt' &&
        !isOpenCode &&
        !isLocalEngine &&
        !hasApiKeyRoute) {
      throw ArgumentError.value(providerId, 'providerId', 'Unknown provider.');
    }
    final hasOAuthRoute = connectionId != null || workspaceId != null;
    final hasApiRoute = apiKeyConnectionId != null;
    if (((isOpenCode || isLocalEngine) &&
            (hasOAuthRoute || hasApiRoute || modelId == null)) ||
        (hasApiKeyRoute &&
            (!hasApiRoute ||
                hasOAuthRoute ||
                modelId == null ||
                (selectedProvider != null &&
                    ApiCompatibleProviderKeyStore.isApiKeyProviderId(
                      selectedProvider,
                    ) &&
                    apiKeyConnectionId != selectedProvider))) ||
        (!isOpenCode &&
            !isLocalEngine &&
            !hasApiKeyRoute &&
            (hasApiRoute ||
                ((connectionId == null) != (workspaceId == null)) ||
                (modelId != null &&
                    (connectionId == null || workspaceId == null))))) {
      throw ArgumentError('The provider route is incomplete or incompatible.');
    }

    final timestamp = createdAt.toUtc().millisecondsSinceEpoch;
    final normalizedProductWorkspaceId = productWorkspaceId == null
        ? null
        : _requireValue(productWorkspaceId, 'productWorkspaceId');
    if (normalizedProductWorkspaceId != null) {
      final productWorkspace =
          await (_database.select(_database.workspaces)
                ..where((row) => row.id.equals(normalizedProductWorkspaceId)))
              .getSingleOrNull();
      if (productWorkspace == null) {
        throw WorkspaceNotFoundException(normalizedProductWorkspaceId);
      }
    }
    await _database
        .into(_database.conversations)
        .insert(
          ConversationsCompanion.insert(
            id: normalizedId,
            title: normalizedTitle,
            connectionId: Value(connectionId?.trim()),
            workspaceId: Value(workspaceId?.trim()),
            apiKeyConnectionId: Value(apiKeyConnectionId?.trim()),
            providerId: Value(selectedProvider),
            modelId: Value(modelId?.trim()),
            projectId: Value(projectId),
            productWorkspaceId: Value(normalizedProductWorkspaceId),
            createdAt: timestamp,
            updatedAt: timestamp,
          ),
        );
  }

  Future<void> saveTemporaryConversation({
    required String id,
    required String title,
    required DateTime createdAt,
    required List<domain.ChatMessage> messages,
    required String providerId,
    required String modelId,
    String? connectionId,
    String? workspaceId,
    String? apiKeyConnectionId,
  }) async {
    requireDurableConversationId(id);
    if (messages.isEmpty || messages.length > 100) {
      throw ArgumentError.value(
        messages.length,
        'messages',
        'A temporary transcript must contain between 1 and 100 messages.',
      );
    }
    final messageIds = <String>{};
    domain.ChatMessageRole? previousRole;
    for (final message in messages) {
      if (message.id.trim().isEmpty || !messageIds.add(message.id)) {
        throw ArgumentError.value(
          message.id,
          'message.id',
          'Temporary transcript message IDs must be non-empty and unique.',
        );
      }
      if (message.toolActivities.isNotEmpty ||
          message.role == domain.ChatMessageRole.user &&
              message.status != domain.ChatMessageStatus.completed ||
          previousRole == message.role) {
        throw ArgumentError.value(
          message,
          'messages',
          'The temporary transcript contains unsupported content or ordering.',
        );
      }
      if (message.attachments.isNotEmpty) {
        if (attachmentStore == null) {
          throw const ChatAttachmentStorageException(
            'Attachments cannot be saved in this storage location.',
          );
        }
        final hasInvalidTemporaryAttachment = message.attachments.any((
          attachment,
        ) {
          final bytes = attachment.bytes;
          return bytes == null || bytes.length != attachment.sizeBytes;
        });
        if (hasInvalidTemporaryAttachment) {
          throw const ChatAttachmentStorageException(
            'A temporary attachment is no longer available in memory.',
          );
        }
      }
      previousRole = message.role;
    }
    if (messages.first.role != domain.ChatMessageRole.user) {
      throw ArgumentError.value(
        messages,
        'messages',
        'A temporary transcript must start with a user message.',
      );
    }

    var conversationCreated = false;
    try {
      await _database.transaction(() async {
        await createConversation(
          id: id,
          title: title,
          createdAt: createdAt,
          providerId: providerId,
          modelId: modelId,
          connectionId: connectionId,
          workspaceId: workspaceId,
          apiKeyConnectionId: apiKeyConnectionId,
        );
        conversationCreated = true;
        for (final message in messages) {
          await saveMessage(
            conversationId: id,
            message: message,
            updateConversationTimestamp: false,
          );
        }
      });
    } on Object catch (error, stackTrace) {
      if (conversationCreated) {
        try {
          await attachmentStore?.deleteConversationAttachments(id);
        } on Object catch (cleanupError, cleanupStackTrace) {
          Error.throwWithStackTrace(
            ChatAttachmentStorageException(
              'The temporary conversation could not be saved and its attachments could not be cleaned up.',
              cleanupError,
            ),
            cleanupStackTrace,
          );
        }
      }
      Error.throwWithStackTrace(error, stackTrace);
    }
  }

  Future<void> bindConversationProvider({
    required String conversationId,
    String? providerId,
    required String connectionId,
    required String workspaceId,
    required String modelId,
  }) async {
    final normalizedConnectionId = _requireValue(connectionId, 'connectionId');
    final normalizedWorkspaceId = _requireValue(workspaceId, 'workspaceId');
    final normalizedModelId = _requireValue(modelId, 'modelId');
    await _database.transaction(() async {
      final conversation = await (_database.select(
        _database.conversations,
      )..where((row) => row.id.equals(conversationId))).getSingleOrNull();
      if (conversation == null) {
        throw ConversationNotFoundException(conversationId);
      }

      final currentSelection = [
        conversation.connectionId,
        conversation.workspaceId,
        conversation.modelId,
      ];
      if (currentSelection.every((value) => value == null)) {
        await (_database.update(
          _database.conversations,
        )..where((row) => row.id.equals(conversationId))).write(
          ConversationsCompanion(
            providerId: Value(providerId?.trim()),
            connectionId: Value(normalizedConnectionId),
            workspaceId: Value(normalizedWorkspaceId),
            apiKeyConnectionId: const Value(null),
            modelId: Value(normalizedModelId),
          ),
        );
        return;
      }

      if (conversation.connectionId == normalizedConnectionId &&
          conversation.workspaceId == normalizedWorkspaceId &&
          conversation.modelId == normalizedModelId) {
        return;
      }
      throw ConversationProviderAlreadyBoundException(conversationId);
    });
  }

  Future<void> setConversationRoute({
    required String conversationId,
    required String providerId,
    required String modelId,
    String? connectionId,
    String? workspaceId,
    String? apiKeyConnectionId,
  }) async {
    final normalizedProviderId = _requireValue(providerId, 'providerId');
    final normalizedModelId = _requireValue(modelId, 'modelId');
    final isOpenCode = normalizedProviderId == 'opencode';
    final isLocalEngine = _isLocalEngineProvider(normalizedProviderId);
    final hasApiKeyRoute = _isApiKeyRouteProvider(normalizedProviderId);
    if (!isOpenCode &&
        !isLocalEngine &&
        !hasApiKeyRoute &&
        normalizedProviderId != 'chatgpt') {
      throw ArgumentError.value(providerId, 'providerId', 'Unknown provider.');
    }
    final hasOAuthRoute = connectionId != null || workspaceId != null;
    if (((isOpenCode || isLocalEngine) &&
            (hasOAuthRoute || apiKeyConnectionId != null)) ||
        (hasApiKeyRoute &&
            (apiKeyConnectionId == null ||
                hasOAuthRoute ||
                (ApiCompatibleProviderKeyStore.isApiKeyProviderId(
                      normalizedProviderId,
                    ) &&
                    apiKeyConnectionId != normalizedProviderId))) ||
        (!isOpenCode &&
            !isLocalEngine &&
            !hasApiKeyRoute &&
            (apiKeyConnectionId != null ||
                connectionId == null ||
                workspaceId == null))) {
      throw ArgumentError('The provider route is incomplete or incompatible.');
    }
    final normalizedConnectionId = isOpenCode || isLocalEngine || hasApiKeyRoute
        ? null
        : _requireValue(connectionId ?? '', 'connectionId');
    final normalizedWorkspaceId = isOpenCode || isLocalEngine || hasApiKeyRoute
        ? null
        : _requireValue(workspaceId ?? '', 'workspaceId');
    final normalizedApiKeyConnectionId = hasApiKeyRoute
        ? _requireValue(apiKeyConnectionId ?? '', 'apiKeyConnectionId')
        : null;

    final updatedRows =
        await (_database.update(
          _database.conversations,
        )..where((row) => row.id.equals(conversationId))).write(
          ConversationsCompanion(
            providerId: Value(normalizedProviderId),
            connectionId: Value(normalizedConnectionId),
            workspaceId: Value(normalizedWorkspaceId),
            apiKeyConnectionId: Value(normalizedApiKeyConnectionId),
            modelId: Value(normalizedModelId),
            updatedAt: Value(DateTime.now().toUtc().millisecondsSinceEpoch),
          ),
        );
    if (updatedRows == 0) {
      throw ConversationNotFoundException(conversationId);
    }
  }

  Future<void> renameConversation({
    required String conversationId,
    required String title,
  }) async {
    final normalizedTitle = _requireValue(title, 'title');
    final updatedRows =
        await (_database.update(
          _database.conversations,
        )..where((row) => row.id.equals(conversationId))).write(
          ConversationsCompanion(
            title: Value(normalizedTitle),
            titleSource: const Value('manual'),
            updatedAt: Value(DateTime.now().toUtc().millisecondsSinceEpoch),
          ),
        );
    if (updatedRows == 0) {
      throw ConversationNotFoundException(conversationId);
    }
  }

  Future<void> setConversationModel({
    required String conversationId,
    required String modelId,
  }) async {
    final normalizedModelId = _requireValue(modelId, 'modelId');
    await _database.transaction(() async {
      final conversation = await (_database.select(
        _database.conversations,
      )..where((row) => row.id.equals(conversationId))).getSingleOrNull();
      if (conversation == null) {
        throw ConversationNotFoundException(conversationId);
      }
      final isOpenCode = conversation.providerId == 'opencode';
      final hasApiKeyRoute = _isApiKeyRouteProvider(conversation.providerId);
      if (!isOpenCode &&
          !hasApiKeyRoute &&
          (conversation.connectionId == null ||
              conversation.workspaceId == null)) {
        throw StateError('The conversation has no provider route.');
      }
      if (hasApiKeyRoute && conversation.apiKeyConnectionId == null) {
        throw StateError('The conversation has no API key route.');
      }
      if (conversation.providerId != null &&
          ApiCompatibleProviderKeyStore.isApiKeyProviderId(
            conversation.providerId!,
          ) &&
          conversation.apiKeyConnectionId != conversation.providerId) {
        throw StateError('The conversation provider route is invalid.');
      }
      if (conversation.modelId == normalizedModelId) return;

      await (_database.update(
        _database.conversations,
      )..where((row) => row.id.equals(conversationId))).write(
        ConversationsCompanion(
          modelId: Value(normalizedModelId),
          updatedAt: Value(DateTime.now().toUtc().millisecondsSinceEpoch),
        ),
      );
    });
  }

  Future<bool> applyGeneratedTitle({
    required String conversationId,
    required String title,
  }) async {
    final normalizedTitle = _requireValue(title, 'title');
    final updatedRows =
        await (_database.update(_database.conversations)..where(
              (row) =>
                  row.id.equals(conversationId) &
                  row.titleSource.equals('automatic'),
            ))
            .write(ConversationsCompanion(title: Value(normalizedTitle)));
    return updatedRows == 1;
  }

  Future<void> saveMessage({
    required String conversationId,
    required domain.ChatMessage message,
    bool updateConversationTimestamp = true,
    bool attachmentsAlreadyCopied = false,
  }) async {
    requireDurableConversationId(conversationId);
    if (message.id.trim().isEmpty) {
      throw ArgumentError.value(
        message.id,
        'message.id',
        'Message ID cannot be empty.',
      );
    }

    final attachmentStore = this.attachmentStore;
    var attachmentsSaved = false;
    if (message.attachments.isNotEmpty) {
      if (attachmentStore == null) {
        throw const ChatAttachmentStorageException(
          'File attachments are unavailable in this storage location.',
        );
      }
      if (attachmentsAlreadyCopied) {
        final copiedAttachments = await attachmentStore.readMessageAttachments(
          conversationId: conversationId,
          messageId: message.id,
          expectedAttachments: message.attachments,
        );
        if (copiedAttachments.any((attachment) => !attachment.isAvailable)) {
          throw const ChatAttachmentStorageException(
            'Copied message attachments were not available in the new conversation.',
          );
        }
      } else {
        await attachmentStore.saveMessageAttachments(
          conversationId: conversationId,
          messageId: message.id,
          attachments: message.attachments,
        );
        attachmentsSaved = true;
      }
    }

    try {
      await _database.transaction(() async {
        final conversation = await (_database.select(
          _database.conversations,
        )..where((row) => row.id.equals(conversationId))).getSingleOrNull();
        if (conversation == null) {
          throw ConversationNotFoundException(conversationId);
        }

        final existingMessage = message.attachments.isEmpty
            ? await (_database.select(_database.messages)..where(
                    (row) =>
                        row.conversationId.equals(conversationId) &
                        row.id.equals(message.id),
                  ))
                  .getSingleOrNull()
            : null;
        final storedMessageAttachments = message.attachments.isNotEmpty
            ? message.attachments
            : existingMessage == null
            ? const <ChatAttachment>[]
            : ChatMessageContentCodec.decode(existingMessage.content)
                  .attachments;

        final createdAt = message.createdAt ?? DateTime.now().toUtc();
        final createdAtMilliseconds = createdAt.toUtc().millisecondsSinceEpoch;
        await _database
            .into(_database.messages)
            .insertOnConflictUpdate(
              MessagesCompanion.insert(
                id: message.id,
                conversationId: conversationId,
                role: message.role.name,
                content: ChatMessageContentCodec.encode(
                  message.content,
                  storedMessageAttachments,
                ),
                createdAt: Value(createdAtMilliseconds),
                outputTokens: Value(message.outputTokens),
                tokensPerSecond: Value(message.tokensPerSecond),
                elapsedMicroseconds: Value(message.elapsed?.inMicroseconds),
                providerId: Value(message.providerId),
                modelId: Value(message.modelId),
                citationSources: Value(
                  jsonEncode(
                    message.citationSources
                        .map((source) => source.toJson())
                        .toList(growable: false),
                  ),
                ),
                personalMemoryFactIds: Value(
                  jsonEncode(
                    _validatedPersonalMemoryFactIds(
                      message.personalMemoryFactIds,
                    ),
                  ),
                ),
                reasoningSummaries: Value(
                  jsonEncode(
                    message.reasoningSummaries
                        .map((summary) => summary.toJson())
                        .toList(growable: false),
                  ),
                ),
                toolActivities: Value(
                  jsonEncode(
                    message.toolActivities
                        .map((activity) => activity.toJson())
                        .toList(growable: false),
                  ),
                ),
                status: message.status.name,
                failureCode: Value(message.failureCode),
              ),
            );

        if (updateConversationTimestamp) {
          await (_database.update(
            _database.conversations,
          )..where((row) => row.id.equals(conversationId))).write(
            ConversationsCompanion(
              updatedAt: Value(
                createdAtMilliseconds > conversation.updatedAt
                    ? createdAtMilliseconds
                    : conversation.updatedAt,
              ),
            ),
          );
        }
      });
    } on Object catch (error, stackTrace) {
      if (attachmentsSaved && attachmentStore != null) {
        try {
          await attachmentStore.deleteMessageAttachments(
            conversationId: conversationId,
            messageId: message.id,
          );
        } on Object catch (cleanupError, cleanupStackTrace) {
          Error.throwWithStackTrace(
            ChatAttachmentStorageException(
              'The message could not be saved and its attachments could not be cleaned up: $cleanupError',
            ),
            cleanupStackTrace,
          );
        }
      }
      Error.throwWithStackTrace(error, stackTrace);
    }
  }

  Future<void> setConversationPinned({
    required String conversationId,
    required bool isPinned,
  }) async {
    final updatedRows =
        await (_database.update(_database.conversations)
              ..where((conversation) => conversation.id.equals(conversationId)))
            .write(ConversationsCompanion(isPinned: Value(isPinned)));
    if (updatedRows == 0) {
      throw ConversationNotFoundException(conversationId);
    }
  }

  Future<void> setConversationBookmarked({
    required String conversationId,
    required bool isBookmarked,
  }) async {
    final updatedRows =
        await (_database.update(_database.conversations)
              ..where((conversation) => conversation.id.equals(conversationId)))
            .write(ConversationsCompanion(isBookmarked: Value(isBookmarked)));
    if (updatedRows == 0) {
      throw ConversationNotFoundException(conversationId);
    }
  }

  Future<void> setConversationArchived({
    required String conversationId,
    required bool isArchived,
  }) async {
    final updatedRows =
        await (_database.update(_database.conversations)
              ..where((conversation) => conversation.id.equals(conversationId)))
            .write(
              ConversationsCompanion(
                isArchived: Value(isArchived),
                isPinned: isArchived
                    ? const Value(false)
                    : const Value.absent(),
              ),
            );
    if (updatedRows == 0) {
      throw ConversationNotFoundException(conversationId);
    }
  }

  Future<void> setConversationsArchived({
    required List<String> conversationIds,
    required bool isArchived,
  }) async {
    final ids = conversationIds.map((id) => id.trim()).toSet();
    if (ids.isEmpty || ids.any((id) => id.isEmpty)) {
      throw ArgumentError.value(
        conversationIds,
        'conversationIds',
        'At least one conversation ID is required.',
      );
    }
    await _database.transaction(() async {
      final orderedIds = ids.toList(growable: false);
      final existingIds = <String>{};
      for (var offset = 0; offset < orderedIds.length; offset += 500) {
        final chunk = orderedIds.skip(offset).take(500).toSet();
        existingIds.addAll(
          await (_database.select(_database.conversations)
                ..where((conversation) => conversation.id.isIn(chunk)))
              .map((row) => row.id)
              .get(),
        );
      }
      final missingIds = ids.difference(existingIds);
      if (missingIds.isNotEmpty) {
        throw ConversationNotFoundException(missingIds.first);
      }
      for (var offset = 0; offset < orderedIds.length; offset += 500) {
        final chunk = orderedIds.skip(offset).take(500).toSet();
        await (_database.update(
          _database.conversations,
        )..where((conversation) => conversation.id.isIn(chunk))).write(
          ConversationsCompanion(
            isArchived: Value(isArchived),
            isPinned: isArchived ? const Value(false) : const Value.absent(),
          ),
        );
      }
    });
  }

  Future<void> setConversationTags({
    required String conversationId,
    required List<String> tags,
  }) async {
    final normalizedTags = <String, String>{};
    for (final tag in tags) {
      final label = tag.trim();
      if (label.isEmpty) continue;
      if (label.length > 32) {
        throw ArgumentError.value(
          tags,
          'tags',
          'Tags must be 32 characters or fewer.',
        );
      }
      normalizedTags.putIfAbsent(label.toLowerCase(), () => label);
    }
    if (normalizedTags.length > 12) {
      throw ArgumentError.value(
        tags,
        'tags',
        'A conversation can have at most 12 tags.',
      );
    }
    final updatedRows =
        await (_database.update(_database.conversations)
              ..where((conversation) => conversation.id.equals(conversationId)))
            .write(
              ConversationsCompanion(
                tags: Value(
                  jsonEncode(normalizedTags.values.toList(growable: false)),
                ),
              ),
            );
    if (updatedRows == 0) throw ConversationNotFoundException(conversationId);
  }

  Future<void> deleteConversation(String conversationId) async {
    final deletedRows = await _database.transaction(
      () => (_database.delete(
        _database.conversations,
      )..where((conversation) => conversation.id.equals(conversationId))).go(),
    );
    if (deletedRows == 0) {
      throw ConversationNotFoundException(conversationId);
    }
    await attachmentStore?.deleteConversationAttachments(conversationId);
  }

  Future<void> deleteMessage({
    required String conversationId,
    required String messageId,
  }) async {
    final normalizedMessageId = messageId.trim();
    if (normalizedMessageId.isEmpty) {
      throw ArgumentError.value(
        messageId,
        'messageId',
        'Message ID cannot be empty.',
      );
    }
    final deletedRows =
        await (_database.delete(_database.messages)..where(
              (message) =>
                  message.conversationId.equals(conversationId) &
                  message.id.equals(normalizedMessageId),
            ))
            .go();
    if (deletedRows == 0) {
      throw MessageNotFoundException(conversationId, normalizedMessageId);
    }
    await attachmentStore?.deleteMessageAttachments(
      conversationId: conversationId,
      messageId: normalizedMessageId,
    );
  }

  Future<void> deleteAllConversations() async {
    await _database.transaction(() async {
      await _database.delete(_database.conversations).go();
    });
    await attachmentStore?.deleteAllAttachments();
  }

  ChatConversation _conversationFromRow(Conversation row) {
    return ChatConversation(
      id: row.id,
      title: row.title,
      titleSource: _readStoredEnum(
        ChatConversationTitleSource.values,
        row.titleSource,
        fieldDescription: 'conversation title source',
      ),
      connectionId: row.connectionId,
      workspaceId: row.workspaceId,
      productWorkspaceId: row.productWorkspaceId,
      apiKeyConnectionId: row.apiKeyConnectionId,
      providerId: row.providerId,
      modelId: row.modelId,
      projectId: row.projectId,
      isPinned: row.isPinned,
      isArchived: row.isArchived,
      isBookmarked: row.isBookmarked,
      tags: _decodeConversationTags(row.tags),
      createdAt: DateTime.fromMillisecondsSinceEpoch(
        row.createdAt,
        isUtc: true,
      ),
      updatedAt: DateTime.fromMillisecondsSinceEpoch(
        row.updatedAt,
        isUtc: true,
      ),
    );
  }

  List<String> _decodeConversationTags(String encoded) {
    final decoded = jsonDecode(encoded);
    if (decoded is! List<Object?> || decoded.length > 12) {
      throw const FormatException('Conversation tags are invalid.');
    }
    final tags = <String>[];
    final seen = <String>{};
    for (final value in decoded) {
      if (value is! String || value.trim().isEmpty || value.length > 32) {
        throw const FormatException('Conversation tags are invalid.');
      }
      final label = value.trim();
      if (seen.add(label.toLowerCase())) tags.add(label);
    }
    return List<String>.unmodifiable(tags);
  }

  String _requireValue(String value, String parameterName) {
    final normalizedValue = value.trim();
    if (normalizedValue.isEmpty) {
      throw ArgumentError.value(
        value,
        parameterName,
        '$parameterName cannot be empty.',
      );
    }
    return normalizedValue;
  }

  String _requireWorkspaceName(String value) {
    final normalizedName = value.trim();
    if (normalizedName.isEmpty || normalizedName.length > 80) {
      throw ArgumentError.value(
        value,
        'name',
        'Workspace names must contain between 1 and 80 characters.',
      );
    }
    return normalizedName;
  }

  bool _isValidBranchIdentifier(String value) => value.codeUnits.every(
    (unit) =>
        (unit >= 48 && unit <= 57) ||
        (unit >= 65 && unit <= 90) ||
        (unit >= 97 && unit <= 122) ||
        unit == 45 ||
        unit == 95,
  );

  Future<List<domain.ChatMessage>> _messagesFromRows(List<Message> rows) async {
    final messages = List<domain.ChatMessage>.generate(
      rows.length,
      (index) => _messageFromRow(rows[index]),
      growable: false,
    );
    final attachmentMessageIndices = <int>[];
    for (var index = 0; index < messages.length; index++) {
      if (messages[index].attachments.isNotEmpty) {
        attachmentMessageIndices.add(index);
      }
    }
    if (attachmentMessageIndices.isEmpty) return messages;

    final attachmentStore = this.attachmentStore;
    if (attachmentStore == null) {
      final resolvedMessages = List<domain.ChatMessage>.of(messages);
      for (final index in attachmentMessageIndices) {
        final message = resolvedMessages[index];
        resolvedMessages[index] = _copyMessageWithAttachments(
          message,
          message.attachments
              .map(
                (attachment) => ChatAttachment(
                  id: attachment.id,
                  name: attachment.name,
                  mimeType: attachment.mimeType,
                  sizeBytes: attachment.sizeBytes,
                  kind: attachment.kind,
                  isAvailable: false,
                ),
              )
              .toList(growable: false),
        );
      }
      return List<domain.ChatMessage>.of(resolvedMessages, growable: false);
    }

    final attachmentReads = <Future<List<ChatAttachment>>>[];
    for (final index in attachmentMessageIndices) {
      final message = messages[index];
      final row = rows[index];
      attachmentReads.add(
        attachmentStore.readMessageAttachments(
          conversationId: row.conversationId,
          messageId: row.id,
          expectedAttachments: message.attachments,
        ),
      );
    }
    final loadedAttachments = await Future.wait(attachmentReads);
    final resolvedMessages = List<domain.ChatMessage>.of(messages);
    for (var index = 0; index < attachmentMessageIndices.length; index++) {
      final messageIndex = attachmentMessageIndices[index];
      resolvedMessages[messageIndex] = _copyMessageWithAttachments(
        resolvedMessages[messageIndex],
        loadedAttachments[index],
      );
    }
    return List<domain.ChatMessage>.of(resolvedMessages, growable: false);
  }

  domain.ChatMessage _copyMessageWithAttachments(
    domain.ChatMessage message,
    List<ChatAttachment> attachments,
  ) {
    return domain.ChatMessage(
      id: message.id,
      role: message.role,
      content: message.content,
      attachments: attachments,
      createdAt: message.createdAt,
      outputTokens: message.outputTokens,
      tokensPerSecond: message.tokensPerSecond,
      elapsed: message.elapsed,
      providerId: message.providerId,
      modelId: message.modelId,
      citationSources: message.citationSources,
      personalMemoryFactIds: message.personalMemoryFactIds,
      reasoningSummaries: message.reasoningSummaries,
      toolActivities: message.toolActivities,
      status: message.status,
      failureCode: message.failureCode,
    );
  }

  domain.ChatMessage _messageFromRow(Message row) {
    final decodedContent = ChatMessageContentCodec.decode(row.content);
    return domain.ChatMessage(
      id: row.id,
      role: _readStoredEnum(
        domain.ChatMessageRole.values,
        row.role,
        fieldDescription: 'message role',
      ),
      content: decodedContent.content,
      attachments: decodedContent.attachments,
      createdAt: row.createdAt == null
          ? null
          : DateTime.fromMillisecondsSinceEpoch(row.createdAt!, isUtc: true),
      outputTokens: row.outputTokens,
      tokensPerSecond: row.tokensPerSecond,
      elapsed: row.elapsedMicroseconds == null
          ? null
          : Duration(microseconds: row.elapsedMicroseconds!),
      providerId: row.providerId,
      modelId: row.modelId,
      citationSources: domain.ChatCitationSource.listFromJson(
        jsonDecode(row.citationSources),
      ),
      personalMemoryFactIds: _personalMemoryFactIdsFromJson(
        jsonDecode(row.personalMemoryFactIds),
      ),
      reasoningSummaries: domain.ChatReasoningSummary.listFromJson(
        jsonDecode(row.reasoningSummaries),
      ),
      toolActivities: domain.ChatToolActivity.listFromJson(
        jsonDecode(row.toolActivities),
      ),
      status: _readStoredEnum(
        domain.ChatMessageStatus.values,
        row.status,
        fieldDescription: 'message status',
      ),
      failureCode: row.failureCode,
    );
  }
}

class ConversationNotFoundException implements Exception {
  const ConversationNotFoundException(this.conversationId);

  final String conversationId;

  @override
  String toString() => 'Conversation "$conversationId" was not found.';
}

class MessageNotFoundException implements Exception {
  const MessageNotFoundException(this.conversationId, this.messageId);

  final String conversationId;
  final String messageId;

  @override
  String toString() =>
      'Message "$messageId" was not found in conversation "$conversationId".';
}

class ConversationBranchCleanupException implements Exception {
  const ConversationBranchCleanupException(this.cause, this.cleanupFailure);

  final Object cause;
  final Object cleanupFailure;

  @override
  String toString() => 'The conversation branch failed and cleanup failed.';
}

class ConversationProviderAlreadyBoundException implements Exception {
  const ConversationProviderAlreadyBoundException(this.conversationId);

  final String conversationId;

  @override
  String toString() =>
      'Conversation "$conversationId" is already bound to a provider selection.';
}

class ProjectNotFoundException implements Exception {
  const ProjectNotFoundException(this.projectId);

  final String projectId;

  @override
  String toString() => 'Project "$projectId" was not found.';
}

class WorkspaceNotFoundException implements Exception {
  const WorkspaceNotFoundException(this.workspaceId);

  final String workspaceId;

  @override
  String toString() => 'Workspace "$workspaceId" was not found.';
}

class InvalidOutputSaveException implements Exception {
  const InvalidOutputSaveException();

  @override
  String toString() => 'Only completed assistant responses can be saved.';
}

class SavedOutputNotFoundException implements Exception {
  const SavedOutputNotFoundException(this.conversationId, this.messageId);

  final String conversationId;
  final String messageId;

  @override
  String toString() => 'The saved response was not found.';
}

class InvalidSavedOutputVersionException implements Exception {
  const InvalidSavedOutputVersionException();

  @override
  String toString() => 'The saved response version is invalid.';
}
