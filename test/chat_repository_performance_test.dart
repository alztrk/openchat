import 'package:drift/drift.dart';
import 'package:drift/native.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/features/chat/data/chat_repository.dart';
import 'package:openchat/features/chat/data/openchat_database.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';

const _messageCount = 256;
const _warmupCount = 12;
const _sampleCount = 100;

void main() {
  test('measures conversation history mapping in debug mode', () async {
    final database = OpenChatDatabase(NativeDatabase.memory());
    addTearDown(database.close);

    const conversationId = 'performance-conversation';
    final repository = ChatRepository(database);
    final createdAt = DateTime.utc(2026, 10, 9, 12);
    await repository.createConversation(
      id: conversationId,
      title: 'Performance fixture',
      createdAt: createdAt,
    );

    final content = List<String>.filled(
      8,
      'A deterministic assistant history entry with representative text. ',
    ).join();
    final messages = List<MessagesCompanion>.generate(
      _messageCount,
      (index) => MessagesCompanion.insert(
        id: 'message-${index.toString().padLeft(4, '0')}',
        conversationId: conversationId,
        role: index.isEven ? 'user' : 'assistant',
        content: content,
        createdAt: Value(createdAt.millisecondsSinceEpoch + index),
        status: 'completed',
      ),
      growable: false,
    );
    await database.batch((batch) {
      batch.insertAll(database.messages, messages);
    });

    final getMessages = () => repository.getMessages(conversationId);
    final watchMessages = () => repository.watchMessages(conversationId).first;
    final queryRows = () {
      final query = database.select(database.messages)
        ..where((message) => message.conversationId.equals(conversationId))
        ..orderBy([
          (message) => OrderingTerm.asc(message.createdAt),
          (message) => OrderingTerm.asc(message.id),
        ]);
      return query.get();
    };

    for (var index = 0; index < _warmupCount; index++) {
      if (index.isEven) {
        expect(await queryRows(), hasLength(_messageCount));
        _expectCompleteHistory(await getMessages());
        _expectCompleteHistory(await watchMessages());
      } else {
        _expectCompleteHistory(await watchMessages());
        _expectCompleteHistory(await getMessages());
        expect(await queryRows(), hasLength(_messageCount));
      }
    }

    final rowSamples = <int>[];
    final getSamples = <int>[];
    final watchSamples = <int>[];
    List<Message> latestRows = const <Message>[];
    List<ChatMessage> latestGetMessages = const <ChatMessage>[];
    List<ChatMessage> latestWatchMessages = const <ChatMessage>[];

    for (var index = 0; index < _sampleCount; index++) {
      if (index.isEven) {
        final rowResult = await _measure(queryRows);
        latestRows = rowResult.value;
        rowSamples.add(rowResult.microseconds);
        expect(latestRows, hasLength(_messageCount));

        final getResult = await _measure(getMessages);
        latestGetMessages = getResult.value;
        getSamples.add(getResult.microseconds);
        _expectCompleteHistory(latestGetMessages);

        final watchResult = await _measure(watchMessages);
        latestWatchMessages = watchResult.value;
        watchSamples.add(watchResult.microseconds);
        _expectCompleteHistory(latestWatchMessages);
      } else {
        final watchResult = await _measure(watchMessages);
        latestWatchMessages = watchResult.value;
        watchSamples.add(watchResult.microseconds);
        _expectCompleteHistory(latestWatchMessages);

        final getResult = await _measure(getMessages);
        latestGetMessages = getResult.value;
        getSamples.add(getResult.microseconds);
        _expectCompleteHistory(latestGetMessages);

        final rowResult = await _measure(queryRows);
        latestRows = rowResult.value;
        rowSamples.add(rowResult.microseconds);
        expect(latestRows, hasLength(_messageCount));
      }
    }

    expect(
      latestWatchMessages.map((message) => message.id),
      latestGetMessages.map((message) => message.id),
    );
    print(
      'history-mapping debug; messages=$_messageCount; '
      'select.get ${_formatPercentiles(rowSamples)}; '
      'getMessages ${_formatPercentiles(getSamples)}; '
      'watchMessages.first ${_formatPercentiles(watchSamples)}',
    );
  }, skip: !const bool.fromEnvironment('OPENCHAT_RUN_PERF_BENCHMARK'));
}

Future<_TimedResult<T>> _measure<T>(Future<T> Function() operation) async {
  final stopwatch = Stopwatch()..start();
  final value = await operation();
  stopwatch.stop();
  return _TimedResult(value, stopwatch.elapsedMicroseconds);
}

void _expectCompleteHistory(List<ChatMessage> messages) {
  expect(messages, hasLength(_messageCount));
  expect(messages.first.id, 'message-0000');
  expect(messages.last.id, 'message-0255');
}

String _formatPercentiles(List<int> samples) {
  return 'p50=${_percentile(samples, 0.50).toStringAsFixed(1)}us '
      'p95=${_percentile(samples, 0.95).toStringAsFixed(1)}us '
      'p99=${_percentile(samples, 0.99).toStringAsFixed(1)}us';
}

double _percentile(List<int> samples, double probability) {
  final sorted = List<int>.of(samples)..sort();
  final position = probability * (sorted.length - 1);
  final lowerIndex = position.floor();
  final upperIndex = position.ceil();
  final fraction = position - lowerIndex;
  return sorted[lowerIndex] +
      (sorted[upperIndex] - sorted[lowerIndex]) * fraction;
}

class _TimedResult<T> {
  const _TimedResult(this.value, this.microseconds);

  final T value;
  final int microseconds;
}
