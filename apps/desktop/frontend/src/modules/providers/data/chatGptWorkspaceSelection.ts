export function requiresChatGptWorkspaceSelection(workspaces: readonly unknown[]): boolean {
  let hasSelectableWorkspace = false;
  let hasSelectedWorkspace = false;

  for (const workspace of workspaces) {
    if (!isObject(workspace) || typeof workspace.id !== 'string' || workspace.id.trim().length === 0) continue;
    hasSelectableWorkspace = true;
    if (workspace.isSelected === true) hasSelectedWorkspace = true;
  }

  return hasSelectableWorkspace && !hasSelectedWorkspace;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
