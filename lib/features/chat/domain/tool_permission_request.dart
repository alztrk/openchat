final class ToolPermissionRequest {
  const ToolPermissionRequest({
    required this.id,
    required this.toolName,
    required this.targetPath,
    required this.arguments,
  });

  final String id;
  final String toolName;
  final String targetPath;
  final Map<String, Object?> arguments;

  static ToolPermissionRequest? fromEvent(Map<String, Object?> event) {
    final id = event['approvalRequestId'];
    final toolName = event['toolName'];
    final targetPath = event['targetPath'];
    final arguments = event['arguments'];
    if (id is! String ||
        id.isEmpty ||
        toolName is! String ||
        toolName.isEmpty ||
        targetPath is! String ||
        targetPath.isEmpty ||
        arguments is! Map) {
      return null;
    }

    return ToolPermissionRequest(
      id: id,
      toolName: toolName,
      targetPath: targetPath,
      arguments: Map<String, Object?>.from(arguments),
    );
  }
}
