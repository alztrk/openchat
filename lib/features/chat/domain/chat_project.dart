class ChatProject {
  const ChatProject({
    required this.id,
    required this.name,
    required this.folderPath,
    required this.createdAt,
    required this.updatedAt,
  });

  final String id;
  final String name;
  final String folderPath;
  final DateTime createdAt;
  final DateTime updatedAt;
}
