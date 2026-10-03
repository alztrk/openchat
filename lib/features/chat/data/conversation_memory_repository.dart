import 'package:openchat/features/chat/domain/conversation_memory.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

class SemanticSearchPreparation {
  const SemanticSearchPreparation({
    required this.progress,
    required this.completed,
    required this.cancel,
  });

  final Stream<SemanticPreparationProgress> progress;
  final Future<bool> completed;
  final Future<bool> Function() cancel;
}

class ConversationMemoryRepository {
  const ConversationMemoryRepository(this._serviceClient);

  final OpenChatServiceClient _serviceClient;

  Future<ContextUsageConfiguration> estimateContextUsage({
    required String providerId,
    String? modelId,
    bool? supportsTools,
    required String toolPermissionMode,
    required String customInstructions,
    String? conversationId,
  }) async {
    final params = <String, Object?>{
      'providerId': providerId,
      'toolPermissionMode': toolPermissionMode,
    };
    if (modelId != null) params['modelId'] = modelId;
    if (supportsTools != null) params['supportsTools'] = supportsTools;
    if (customInstructions.trim().isNotEmpty) {
      params['customInstructions'] = customInstructions;
    }
    if (conversationId != null) params['conversationId'] = conversationId;
    final response = await _serviceClient.call(
      'chat.context.usage.estimate',
      params: params,
    );
    final instructionsTokens = response['instructionsTokens'];
    final rawDefinitions = response['toolDefinitions'];
    if (instructionsTokens is! int ||
        instructionsTokens < 0 ||
        rawDefinitions is! List) {
      throw const FormatException('Invalid context usage configuration.');
    }
    final definitions = <ToolDefinitionTokenEstimate>[];
    for (final rawDefinition in rawDefinitions) {
      if (rawDefinition is! Map) {
        throw const FormatException('Invalid context tool definition.');
      }
      final name = rawDefinition['name'];
      final tokens = rawDefinition['tokens'];
      if (name is! String || name.isEmpty || tokens is! int || tokens < 0) {
        throw const FormatException('Invalid context tool definition.');
      }
      definitions.add(ToolDefinitionTokenEstimate(name: name, tokens: tokens));
    }
    return ContextUsageConfiguration(
      instructionsTokens: instructionsTokens,
      toolDefinitions: List<ToolDefinitionTokenEstimate>.unmodifiable(
        definitions,
      ),
    );
  }

  Future<ConversationMemoryState> inspect(String conversationId) async {
    final response = await _serviceClient.call(
      'chat.memory.inspect',
      params: <String, Object?>{'conversationId': conversationId},
    );
    return ConversationMemoryState.fromServiceResponse(response);
  }

  Future<List<ArchivedMemoryResult>> search(
    String conversationId,
    String query,
  ) async {
    final response = await _serviceClient.call(
      'chat.memory.search',
      params: <String, Object?>{
        'conversationId': conversationId,
        'query': query,
      },
    );
    final rawResults = response['results'];
    if (rawResults is! List) {
      throw const FormatException('Invalid conversation memory results.');
    }
    return rawResults
        .map(ArchivedMemoryResult.fromServiceResponse)
        .toList(growable: false);
  }

  Future<void> resetCompactedContext(String conversationId) async {
    final response = await _serviceClient.call(
      'chat.memory.reset_compaction',
      params: <String, Object?>{'conversationId': conversationId},
    );
    if (response['reset'] is! bool) {
      throw const FormatException('Invalid conversation memory reset result.');
    }
  }

  Future<bool> semanticSearchIsReady() async {
    final response = await _serviceClient.call('chat.memory.semantic.status');
    return switch (response['ready']) {
      bool ready => ready,
      _ => throw const FormatException('Invalid semantic memory status.'),
    };
  }

  Future<SemanticSearchPreparation> prepareSemanticSearch() async {
    final operation = await _serviceClient.startOperation(
      'chat.memory.semantic.prepare',
    );
    final progress = operation.events
        .where((event) => event.name == 'chat.memory.semantic.progress')
        .map(
          (event) =>
              SemanticPreparationProgress.fromServiceResponse(event.data),
        );
    final completed = operation.result.then<bool>(
      (response) {
        if (response['ready'] != true) {
          throw const FormatException(
            'Semantic memory preparation did not finish.',
          );
        }
        return true;
      },
      onError: (Object error, StackTrace stackTrace) {
        if (error is OpenChatServiceException &&
            error.code == 'operation_cancelled') {
          return false;
        }
        Error.throwWithStackTrace(error, stackTrace);
      },
    );
    return SemanticSearchPreparation(
      progress: progress,
      completed: completed,
      cancel: operation.cancel,
    );
  }
}

class ContextUsageConfiguration {
  const ContextUsageConfiguration({
    required this.instructionsTokens,
    required this.toolDefinitions,
  });

  final int instructionsTokens;
  final List<ToolDefinitionTokenEstimate> toolDefinitions;
}

class ToolDefinitionTokenEstimate {
  const ToolDefinitionTokenEstimate({required this.name, required this.tokens});

  final String name;
  final int tokens;
}
