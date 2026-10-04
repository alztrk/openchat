import 'package:openchat/features/chat/domain/chat_file_change.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

class ChatFileChangesRepository {
  const ChatFileChangesRepository(this._serviceClient);

  final OpenChatServiceClient _serviceClient;

  Future<List<ChatFileChange>> list(String conversationId) async {
    final response = await _serviceClient.call(
      'chat.file_changes.list',
      params: <String, Object?>{'conversationId': conversationId},
    );
    return ChatFileChange.listFromJson(response);
  }

  Future<ChatFileChangeDiff> diff({
    required String conversationId,
    required String changeId,
  }) async {
    final response = await _serviceClient.call(
      'chat.file_changes.diff',
      params: <String, Object?>{
        'conversationId': conversationId,
        'changeId': changeId,
      },
    );
    return ChatFileChangeDiff.fromJson(response);
  }

  Future<List<ChatFileChange>> revert({
    required String conversationId,
    required String changeId,
  }) async {
    final response = await _serviceClient.call(
      'chat.file_changes.revert',
      params: <String, Object?>{
        'conversationId': conversationId,
        'changeId': changeId,
      },
    );
    return ChatFileChange.listFromJson(response);
  }

  Future<void> deleteConversation(String conversationId) async {
    await _serviceClient.call(
      'chat.file_changes.delete',
      params: <String, Object?>{'conversationId': conversationId},
    );
  }

  Future<void> deleteAll() async {
    await _serviceClient.call('chat.file_changes.delete_all');
  }
}
