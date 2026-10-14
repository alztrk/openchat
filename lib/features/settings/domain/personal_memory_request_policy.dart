bool shouldIncludePersonalMemoryForRequest({
  required bool isEnabled,
  required String conversationId,
  required Set<String> excludedConversationIds,
  required bool excludeNewConversation,
}) {
  return isEnabled &&
      !excludeNewConversation &&
      !excludedConversationIds.contains(conversationId);
}
