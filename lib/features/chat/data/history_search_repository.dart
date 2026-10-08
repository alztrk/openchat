import 'package:openchat/features/chat/domain/history_search_result.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

class HistorySearchRepository {
  const HistorySearchRepository(this._serviceClient);

  final OpenChatServiceClient _serviceClient;

  Future<HistorySearchFilterOptions> loadFilterOptions() async {
    final response = await _serviceClient.call(
      'chat.history.search_filters',
      timeout: const Duration(seconds: 15),
    );
    return HistorySearchFilterOptions.fromServiceResponse(response);
  }

  Future<List<HistorySearchResult>> search(
    String query, {
    DateTime? from,
    DateTime? through,
    HistorySearchFilters filters = const HistorySearchFilters(),
  }) async {
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
      params: <String, Object?>{
        'query': normalizedQuery,
        if (from != null) 'fromUnixMs': from.millisecondsSinceEpoch,
        if (through != null) 'throughUnixMs': through.millisecondsSinceEpoch,
        if (filters.providerId != null) 'providerId': filters.providerId,
        if (filters.modelId != null) 'modelId': filters.modelId,
        if (filters.projectId != null) 'projectId': filters.projectId,
        if (filters.isArchived != null) 'isArchived': filters.isArchived,
        if (filters.tag != null) 'tag': filters.tag,
      },
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
