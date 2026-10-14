bool modelCatalogNeedsVerification({
  required String? selectedRouteKey,
  required Iterable<String> availableRouteKeys,
  required Set<String> staleRouteKeys,
}) {
  if (selectedRouteKey != null) {
    return staleRouteKeys.contains(selectedRouteKey);
  }
  var hasRoute = false;
  for (final routeKey in availableRouteKeys) {
    hasRoute = true;
    if (!staleRouteKeys.contains(routeKey)) {
      return false;
    }
  }
  return hasRoute;
}
