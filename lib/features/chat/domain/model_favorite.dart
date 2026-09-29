class FavoriteModel {
  const FavoriteModel({
    required this.providerId,
    required this.modelId,
    required this.displayName,
    this.sourceConnectionId,
  });

  final String providerId;
  final String modelId;
  final String displayName;
  final String? sourceConnectionId;
}
