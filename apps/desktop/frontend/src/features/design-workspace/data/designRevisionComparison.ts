import type { DesignDocument } from './designDocument';

export interface DesignRevisionComparison {
  addedNodes: number;
  removedNodes: number;
  changedNodes: number;
}

export function compareDesignRevisions(current: DesignDocument, previous: DesignDocument): DesignRevisionComparison {
  const currentNodes = new Map(current.nodes.map((node) => [node.id, node]));
  const previousNodes = new Map(previous.nodes.map((node) => [node.id, node]));
  let addedNodes = 0;
  let changedNodes = 0;

  for (const [id, node] of currentNodes) {
    const previousNode = previousNodes.get(id);
    if (!previousNode) {
      addedNodes += 1;
    } else if (JSON.stringify(node) !== JSON.stringify(previousNode)) {
      changedNodes += 1;
    }
  }

  let removedNodes = 0;
  for (const id of previousNodes.keys()) {
    if (!currentNodes.has(id)) removedNodes += 1;
  }

  return { addedNodes, removedNodes, changedNodes };
}
