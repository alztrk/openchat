/** Canonical domain types for projects and tool permissions */

export type ProjectToolPermissionRule = 'ask' | 'allow' | 'deny';
export type ToolPermissionMode = 'require_approval' | 'approve_safe_operations' | 'full_access';

export interface ChatProject {
  id: string;
  name: string;
  folderPath: string;
  workspaceRoots: string[];
  createdAt: number;
  updatedAt: number;
}
