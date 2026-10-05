import 'package:openchat/features/chat/domain/history_search_result.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

class HistorySearchRepository {
  const HistorySearchRepository(this._serviceClient);

  final OpenChatServiceClient _serviceClient;

  Future<List<HistorySearchResult>> search(String query) async {
    final normalizedQuery = query.trim();
    final queryLength = normalizedQuery.runes.length;
    if (queryLength < 2 || queryLength > 512) {
      throw ArgumentError.value(
        query,
        'query',
        'Invalid history search query.',
      );
    }
    final response = await _serviceClient.call(
      'chat.history.search',
      params: <String, Object?>{'query': normalizedQuery},
      timeout: const Duration(seconds: 60),
    );
    final rawResults = response['results'];
    if (rawResults is! List || rawResults.length > 30) {
      throw const FormatException('Invalid conversation history results.');
    }
    return rawResults
        .map(HistorySearchResult.fromServiceResponse)
        .toList(growable: false);
  }
}
