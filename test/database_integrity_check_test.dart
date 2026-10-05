import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/features/chat/data/openchat_database.dart';
import 'package:sqlite3/sqlite3.dart' as sqlite;

void main() {
  test('SQLite runtime guard requires the WAL-reset fix', () {
    expect(OpenChatDatabase.supportsSqliteRuntime(3051002), isFalse);
    expect(OpenChatDatabase.supportsSqliteRuntime(3051003), isTrue);
    expect(
      () => OpenChatDatabase.validateSqliteRuntime(
        versionNumber: 3051002,
        version: '3.51.2',
      ),
      throwsA(
        isA<DatabaseIntegrityFailure>()
            .having(
              (failure) => failure.code,
              'code',
              'sqlite_runtime_unsupported',
            )
            .having((failure) => failure.retryable, 'retryable', isFalse),
      ),
    );
    expect(
      () => OpenChatDatabase.validateSqliteRuntime(
        versionNumber: 3051003,
        version: '3.51.3',
      ),
      returnsNormally,
    );
    expect(
      OpenChatDatabase.supportsSqliteRuntime(
        sqlite.sqlite3.version.versionNumber,
      ),
      isTrue,
    );
  });

  test('read-only integrity preflight accepts a valid database', () async {
    final directory = await Directory.systemTemp.createTemp(
      'openchat-integrity-valid-',
    );
    addTearDown(() => directory.delete(recursive: true));
    final path = '${directory.path}${Platform.pathSeparator}openchat.sqlite3';
    final database = sqlite.sqlite3.open(path);
    database.execute('PRAGMA journal_mode = WAL');
    database.execute('CREATE TABLE probe (value TEXT NOT NULL)');
    database.execute("INSERT INTO probe VALUES ('preserved')");
    database.close();

    await OpenChatDatabase.verifyExistingFile(path);
  });

  test(
    'read-only integrity preflight rejects corrupt bytes without changing them',
    () async {
      final directory = await Directory.systemTemp.createTemp(
        'openchat-integrity-corrupt-',
      );
      addTearDown(() => directory.delete(recursive: true));
      final file = File(
        '${directory.path}${Platform.pathSeparator}openchat.sqlite3',
      );
      const original = <int>[0x4f, 0x70, 0x65, 0x6e, 0x43, 0x68, 0x61, 0x74];
      await file.writeAsBytes(original, flush: true);

      await expectLater(
        OpenChatDatabase.verifyExistingFile(file.path),
        throwsA(
          isA<DatabaseIntegrityFailure>()
              .having((failure) => failure.isCorrupt, 'isCorrupt', isTrue)
              .having((failure) => failure.retryable, 'retryable', isFalse),
        ),
      );

      expect(await file.readAsBytes(), original);
    },
  );

  test(
    'read-only integrity preflight allows a new empty database file',
    () async {
      final directory = await Directory.systemTemp.createTemp(
        'openchat-integrity-empty-',
      );
      addTearDown(() => directory.delete(recursive: true));
      final file = File(
        '${directory.path}${Platform.pathSeparator}openchat.sqlite3',
      );
      await file.create();

      await OpenChatDatabase.verifyExistingFile(file.path);

      expect(await file.length(), 0);
    },
  );
}
