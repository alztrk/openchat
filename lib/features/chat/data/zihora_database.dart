import 'package:drift/drift.dart';
import 'package:drift_flutter/drift_flutter.dart';

part 'zihora_database.g.dart';

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
  TextColumn get modelId => text().nullable()();
  TextColumn get projectId => text().nullable().references(
    Projects,
    #id,
    onDelete: KeyAction.setNull,
  )();
  BoolColumn get isPinned => boolean().withDefault(const Constant(false))();
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
  TextColumn get reasoningSummaries =>
      text().withDefault(const Constant('[]'))();
  TextColumn get status => text()();

  @override
  Set<Column<Object>> get primaryKey => {conversationId, id};
}

@DriftDatabase(tables: [Projects, Conversations, Messages])
class ZihoraDatabase extends _$ZihoraDatabase {
  ZihoraDatabase([QueryExecutor? executor])
    : super(executor ?? driftDatabase(name: 'zihora_chat'));

  ZihoraDatabase.atPath(String databasePath)
    : super(_databaseAtPath(databasePath));

  @override
  int get schemaVersion => 4;

  @override
  MigrationStrategy get migration => MigrationStrategy(
    onCreate: (migrator) => migrator.createAll(),
    onUpgrade: (migrator, from, to) async {
      if (from < 2) {
        await migrator.addColumn(conversations, conversations.titleSource);
        await migrator.addColumn(conversations, conversations.connectionId);
        await migrator.addColumn(conversations, conversations.workspaceId);
      }
      if (from < 3) {
        await migrator.createTable(projects);
        await migrator.addColumn(conversations, conversations.projectId);
      }
      if (from < 4) {
        await migrator.addColumn(messages, messages.reasoningSummaries);
      }
    },
    beforeOpen: (_) async => customStatement('PRAGMA foreign_keys = ON'),
  );
}

QueryExecutor _databaseAtPath(String path) {
  return driftDatabase(
    name: 'zihora_local',
    native: DriftNativeOptions(
      databasePath: () async => path,
      setup: (database) => database.execute('PRAGMA busy_timeout = 5000;'),
    ),
  );
}
