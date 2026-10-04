import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/features/chat/data/tool_index_redaction.dart';
import 'package:sqlite3/sqlite3.dart' as sqlite;

void main() {
  test('redacts common named credentials and provider token formats', () {
    const input =
        'api_key=short-secret openai_api_key=prefixed-secret '
        '"client_secret":"json-secret" '
        'Authorization: Bearer header-secret\n'
        'sk-proj-abcdefghijklmnopqrstuvwxyz1234567890 '
        'sk-ant-api03-abcdefghijklmnopqrstuvwxyz '
        'sk-or-v1-abcdefghijklmnopqrstuvwxyz0123456789 '
        'gsk_abcdefghijklmnopqrstuvwxyz123456 '
        'AIzaabcdefghijklmnopqrstuvwxyz1234567890 '
        'hf_abcdefghijklmnopqrstuvwxyz123456 '
        'csk-abcdefghijklmnopqrstuvwxyz123456 '
        'npm_abcdefghijklmnopqrstuvwxyz123456 '
        'ya29.abcdefghijklmnopqrstuvwxyz1234567890 '
        'github_pat_abcdefghijklmnopqrstuvwxyz0123456789';

    final redacted = redactToolIndexText(input);

    for (final secret in <String>[
      'short-secret',
      'prefixed-secret',
      'json-secret',
      'header-secret',
      'sk-proj-abcdefghijklmnopqrstuvwxyz1234567890',
      'sk-ant-api03-abcdefghijklmnopqrstuvwxyz',
      'sk-or-v1-abcdefghijklmnopqrstuvwxyz0123456789',
      'gsk_abcdefghijklmnopqrstuvwxyz123456',
      'AIzaabcdefghijklmnopqrstuvwxyz1234567890',
      'hf_abcdefghijklmnopqrstuvwxyz123456',
      'csk-abcdefghijklmnopqrstuvwxyz123456',
      'npm_abcdefghijklmnopqrstuvwxyz123456',
      'ya29.abcdefghijklmnopqrstuvwxyz1234567890',
      'github_pat_abcdefghijklmnopqrstuvwxyz0123456789',
    ]) {
      expect(redacted, isNot(contains(secret)));
    }
    expect(RegExp(r'\[REDACTED\]').allMatches(redacted), hasLength(14));
  });

  test('preserves ordinary text, Unicode, and short token-like values', () {
    const input = 'Ordinary text, café and sk-demo';
    expect(redactToolIndexText(input), input);
  });

  test('replaces oversized strings before scanning them', () {
    final input = 'x' * (8 * 1024 * 1024 + 1);
    expect(
      redactToolIndexText(input),
      '[tool activity omitted: searchable archive limit exceeded]',
    );
  });

  test('registers a trigger-safe SQLite function for Drift connections', () {
    final database = sqlite.sqlite3.openInMemory();
    addTearDown(database.close);
    registerToolIndexRedaction(database);
    database.execute('CREATE TABLE indexed (content TEXT NOT NULL)');
    database.execute('CREATE TABLE source (content TEXT NOT NULL)');
    database.execute('''
      CREATE TRIGGER index_source
      AFTER INSERT ON source
      BEGIN
        INSERT INTO indexed VALUES (
          openchat_redact_credentials(NEW.content)
        );
      END
    ''');
    database.execute("INSERT INTO source VALUES ('api_key=trigger-secret')");

    final indexed = database.select('SELECT content FROM indexed').single;
    final source = database.select('SELECT content FROM source').single;
    expect(indexed['content'], 'api_key=[REDACTED]');
    expect(source['content'], 'api_key=trigger-secret');
  });
}
