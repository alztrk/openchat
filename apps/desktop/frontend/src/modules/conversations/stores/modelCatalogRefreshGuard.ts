export function createModelCatalogRefreshGuard() {
  let currentRevision = 0;

  return {
    begin(): number {
      currentRevision += 1;
      return currentRevision;
    },
    isCurrent(revision: number): boolean {
      return revision === currentRevision;
    },
  };
}
