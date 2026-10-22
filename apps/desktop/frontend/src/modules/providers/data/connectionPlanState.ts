export interface ChatGptWorkspacePlanState {
  id: string;
  displayName: string | null;
  planType: string | null;
  isSelected: boolean;
}

export interface ChatGptConnectionPlanState {
  id: string;
  authStatus: string;
  displayName: string | null;
  email: string | null;
  planType: string | null;
  isSelected: boolean;
  workspaces: ChatGptWorkspacePlanState[];
}

/** Applies plan metadata from an older refresh snapshot without restoring deleted rows. */
export function mergeConnectionPlanUpdates(
  current: readonly ChatGptConnectionPlanState[],
  refreshed: readonly ChatGptConnectionPlanState[],
): ChatGptConnectionPlanState[] {
  const refreshedById = new Map(refreshed.map((connection) => [connection.id, connection]));

  return current.map((connection) => {
    const updated = refreshedById.get(connection.id);
    if (!updated) return connection;

    const workspacePlans = new Map(updated.workspaces.map((workspace) => [workspace.id, workspace.planType]));
    return {
      ...connection,
      planType: updated.planType,
      workspaces: connection.workspaces.map((workspace) => {
        const planType = workspacePlans.get(workspace.id);
        return planType === undefined ? workspace : { ...workspace, planType };
      }),
    };
  });
}
