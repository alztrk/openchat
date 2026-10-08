import 'dart:io';
import 'dart:isolate';

import 'package:drift/drift.dart';
import 'package:drift_flutter/drift_flutter.dart';
import 'package:sqlite3/sqlite3.dart' as sqlite;

import 'tool_index_redaction.dart';

part 'openchat_database.g.dart';

class Projects extends Table {
  TextColumn get id => text()();
  TextColumn get name => text()();
  TextColumn get folderPath => text()();
  IntColumn get createdAt => integer()();
  IntColumn get updatedAt => integer()();

  @override
  Set<Column<Object>> get primaryKey => {id};
}

class Conversations extends Table {
  TextColumn get id => text()();
  TextColumn get title => text()();
  TextColumn get titleSource =>
      text().withDefault(const Constant('automatic'))();
  TextColumn get connectionId => text().nullable()();
  TextColumn get workspaceId => text().nullable()();
  TextColumn get apiKeyConnectionId => text().nullable()();
  TextColumn get providerId => text().nullable()();
  TextColumn get modelId => text().nullable()();
  TextColumn get projectId => text().nullable().references(
    Projects,
    #id,
    onDelete: KeyAction.setNull,
  )();
  BoolColumn get isPinned => boolean().withDefault(const Constant(false))();
  BoolColumn get isArchived => boolean().withDefault(const Constant(false))();
  BoolColumn get isBookmarked => boolean().withDefault(const Constant(false))();
  TextColumn get tags => text().withDefault(const Constant('[]'))();
  IntColumn get createdAt => integer()();
  IntColumn get updatedAt => integer()();

  @override
  Set<Column<Object>> get primaryKey => {id};
}

class Messages extends Table {
  TextColumn get id => text()();
  TextColumn get conversationId =>
      text().references(Conversations, #id, onDelete: KeyAction.cascade)();
  TextColumn get role => text()();
  TextColumn get content => text()();
  IntColumn get createdAt => integer().nullable()();
  IntColumn get outputTokens => integer().nullable()();
  RealColumn get tokensPerSecond => real().nullable()();
  IntColumn get elapsedMicroseconds => integer().nullable()();
  TextColumn get providerId => text().nullable()();
  TextColumn get modelId => text().nullable()();
  TextColumn get citationSources => text().withDefault(const Constant('[]'))();
  TextColumn get reasoningSummaries =>
      text().withDefault(const Constant('[]'))();
  TextColumn get toolActivities => text().withDefault(const Constant('[]'))();
  TextColumn get status => text()();
  TextColumn get failureCode => text().nullable()();

  @override
  Set<Column<Object>> get primaryKey => {conversationId, id};
}

class ModelFavorites extends Table {
  TextColumn get providerId => text()();
  TextColumn get modelId => text()();
  TextColumn get sourceConnectionId => text().nullable()();
  TextColumn get displayName => text()();
  IntColumn get favoritedAt => integer()();

  @override
  Set<Column<Object>> get primaryKey => {providerId, modelId};
}

@DriftDatabase(tables: [Projects, Conversations, Messages, ModelFavorites])
class OpenChatDatabase extends _$OpenChatDatabase {
  OpenChatDatabase([QueryExecutor? executor])
    : super(executor ?? _defaultDatabase());

  OpenChatDatabase.atPath(String databasePath)
    : super(_databaseAtPath(databasePath));

  static const currentSchemaVersion = 14;
  static const minimumSqliteVersionNumber = 3051003;

  static bool supportsSqliteRuntime(int versionNumber) =>
      versionNumber >= minimumSqliteVersionNumber;

  static void validateSqliteRuntime({
    required int versionNumber,
    required String version,
  }) {
    if (supportsSqliteRuntime(versionNumber)) return;

    throw DatabaseIntegrityFailure(
      code: 'sqlite_runtime_unsupported',
      isCorrupt: false,
      retryable: false,
      message: 'OpenChat requires SQLite 3.51.3 or newer; loaded $version.',
    );
  }

  @override
  int get schemaVersion => currentSchemaVersion;

  static Future<void> verifyExistingFile(String databasePath) =>
      Isolate.run(() => _verifyExistingFile(databasePath));

  static void _verifyExistingFile(String databasePath) {
    _ensureSupportedSqliteRuntime();
    final file = File(databasePath);
    if (!file.existsSync() || file.lengthSync() == 0) return;

    sqlite.Database? database;
    try {
      database = sqlite.sqlite3.open(
        databasePath,
        mode: sqlite.OpenMode.readOnly,
      );
      final result = database.select('PRAGMA quick_check(1)');
      if (result.length != 1 || result.first.values.single != 'ok') {
        throw const DatabaseIntegrityFailure(
          code: 'database_corrupt',
          isCorrupt: true,
          retryable: false,
          message: 'SQLite quick_check rejected the existing database.',
        );
      }
    } on sqlite.SqliteException catch (error) {
      final isCorrupt = error.resultCode == 11 || error.resultCode == 26;
      final isRetryable =
          error.resultCode == 5 ||
          error.resultCode == 6 ||
          error.resultCode == 10 ||
          error.resultCode == 14;
      throw DatabaseIntegrityFailure(
        code: isCorrupt
            ? 'database_corrupt'
            : 'database_integrity_check_failed',
        isCorrupt: isCorrupt,
        retryable: isRetryable,
        message:
            'SQLite quick_check failed with result code ${error.resultCode}.',
      );
    } finally {
      database?.close();
    }
  }

  @override
  // Keep schema changes and their published version in one write transaction.
  MigrationStrategy get migration => MigrationStrategy(
    onCreate: (migrator) => transaction(() async {
      final currentVersion = await _readSchemaVersion();
      if (currentVersion > schemaVersion) {
        throw StateError(
          'Database schema $currentVersion is newer than supported schema $schemaVersion.',
        );
      }
      if (currentVersion == 0) {
        await migrator.createAll();
      } else if (currentVersion < schemaVersion) {
        await _upgradeSchema(migrator, currentVersion, schemaVersion);
      }
      await _writeSchemaVersion(schemaVersion);
    }),
    onUpgrade: (migrator, _, to) => transaction(() async {
      final currentVersion = await _readSchemaVersion();
      if (currentVersion > to) {
        throw StateError(
          'Database schema $currentVersion is newer than supported schema $to.',
        );
      }
      if (currentVersion < to) {
        await _upgradeSchema(migrator, currentVersion, to);
        await _writeSchemaVersion(to);
      }
    }),
    beforeOpen: (_) async => customStatement('PRAGMA foreign_keys = ON'),
  );

  Future<int> _readSchemaVersion() async {
    final row = await customSelect('PRAGMA user_version').getSingle();
    return row.read<int>('user_version');
  }

  Future<void> _writeSchemaVersion(int version) =>
      customStatement('PRAGMA user_version = $version');

  Future<void> _upgradeSchema(Migrator migrator, int from, int to) async {
    if (from < 2) {
      await _addColumnIfMissing(
        migrator,
        conversations,
        conversations.titleSource,
      );
      await _addColumnIfMissing(
        migrator,
        conversations,
        conversations.connectionId,
      );
      await _addColumnIfMissing(
        migrator,
        conversations,
        conversations.workspaceId,
      );
    }
    if (from < 3) {
      await migrator.createTable(projects);
      await _addColumnIfMissing(
        migrator,
        conversations,
        conversations.projectId,
      );
    }
    if (from < 4) {
      await _addColumnIfMissing(
        migrator,
        messages,
        messages.reasoningSummaries,
      );
    }
    if (from < 5) {
      await _addColumnIfMissing(
        migrator,
        conversations,
        conversations.providerId,
      );
    }
    if (from < 6) {
      await migrator.createTable(modelFavorites);
    }
    if (from < 7) {
      await _addColumnIfMissing(migrator, messages, messages.toolActivities);
    }
    if (from < 8) {
      await _addColumnIfMissing(
        migrator,
        conversations,
        conversations.apiKeyConnectionId,
      );
      await _addColumnIfMissing(
        migrator,
        modelFavorites,
        modelFavorites.sourceConnectionId,
      );
    }
    if (from < 9) {
      // Normalize persisted enum names before strict domain parsing.
      await customStatement(
        "UPDATE conversations SET title_source = 'manual' WHERE title_source = 'user'",
      );
      await customStatement(
        "UPDATE messages SET status = 'completed' WHERE status = 'complete'",
      );
    }
    if (from < 10 && to >= 10) {
      await _addColumnIfMissing(migrator, messages, messages.failureCode);
    }
    if (from < 11 && to >= 11) {
      await _addColumnIfMissing(
        migrator,
        conversations,
        conversations.isArchived,
      );
    }
    if (from < 12 && to >= 12) {
      await _addColumnIfMissing(migrator, messages, messages.providerId);
      await _addColumnIfMissing(migrator, messages, messages.modelId);
      await _addColumnIfMissing(migrator, messages, messages.citationSources);
    }
    if (from < 13 && to >= 13) {
      await _addColumnIfMissing(migrator, conversations, conversations.tags);
    }
    if (from < 14 && to >= 14) {
      await _addColumnIfMissing(
        migrator,
        conversations,
        conversations.isBookmarked,
      );
    }
  }

  Future<void> _addColumnIfMissing(
    Migrator migrator,
    TableInfo table,
    GeneratedColumn<Object> column,
  ) async {
    final tableName = table.actualTableName.replaceAll('"', '""');
    final columns = await customSelect('PRAGMA table_info("$tableName")').get();
    if (columns.any((row) => row.read<String>('name') == column.name)) return;
    await migrator.addColumn(table, column);
  }

  static void _ensureSupportedSqliteRuntime() {
    final version = sqlite.sqlite3.version;
    validateSqliteRuntime(
      versionNumber: version.versionNumber,
      version: version.libVersion,
    );
  }
}

final class DatabaseIntegrityFailure implements Exception {
  const DatabaseIntegrityFailure({
    required this.code,
    required this.isCorrupt,
    required this.retryable,
    required this.message,
  });

  final String code;
  final bool isCorrupt;
  final bool retryable;
  final String message;

  @override
  String toString() => message;
}

QueryExecutor _databaseAtPath(String path) {
  OpenChatDatabase._ensureSupportedSqliteRuntime();
  return driftDatabase(
    name: 'openchat_local',
    native: _nativeDatabaseOptions(databasePath: () async => path),
  );
}

QueryExecutor _defaultDatabase() {
  OpenChatDatabase._ensureSupportedSqliteRuntime();
  return driftDatabase(name: 'openchat_chat', native: _nativeDatabaseOptions());
}

DriftNativeOptions _nativeDatabaseOptions({
  Future<String> Function()? databasePath,
}) {
  return DriftNativeOptions(
    databasePath: databasePath,
    setup: (database) {
      database.execute('PRAGMA busy_timeout = 5000;');
      database.createFunction(
        functionName: 'openchat_redact_credentials',
        argumentCount: const sqlite.AllowedArgumentCount(1),
        deterministic: true,
        directOnly: false,
        function: redactToolIndexSqlFunction,
      );
    },
  );
}
