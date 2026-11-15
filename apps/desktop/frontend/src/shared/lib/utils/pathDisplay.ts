/** Removes the Windows extended-length prefix from a path before showing it in the UI. */
export function formatDisplayPath(path: string): string {
  const extendedUncPrefix = '\\\\?\\UNC\\';
  if (path.toLowerCase().startsWith(extendedUncPrefix.toLowerCase())) {
    return `\\\\${path.slice(extendedUncPrefix.length)}`;
  }

  const extendedPathPrefix = '\\\\?\\';
  return path.startsWith(extendedPathPrefix) ? path.slice(extendedPathPrefix.length) : path;
}

/** Keeps the original Windows extended-length prefix when an editable display path changes. */
export function restoreDisplayPathPrefix(displayPath: string, originalPath: string): string {
  const extendedUncPrefix = '\\\\?\\UNC\\';
  const extendedPathPrefix = '\\\\?\\';

  if (displayPath.startsWith(extendedPathPrefix)) return displayPath;

  if (originalPath.toLowerCase().startsWith(extendedUncPrefix.toLowerCase()) && displayPath.startsWith('\\\\')) {
    return `${extendedUncPrefix}${displayPath.slice(2)}`;
  }

  if (originalPath.startsWith(extendedPathPrefix) && /^[a-z]:\\/iu.test(displayPath)) {
    return `${extendedPathPrefix}${displayPath}`;
  }

  return displayPath;
}
