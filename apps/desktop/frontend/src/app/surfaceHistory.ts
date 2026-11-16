import type { AppSurface } from '@widgets/navigation-rail/NavigationRail';

export interface SurfaceHistoryState {
  surfaces: AppSurface[];
  index: number;
  didNavigateBeforeStorageReady: boolean;
}

export type SurfaceHistoryAction =
  | { type: 'storage-ready'; surface: AppSurface }
  | { type: 'navigation-intent' }
  | { type: 'navigate'; surface: AppSurface }
  | { type: 'move-in-history'; offset: number };

export function createSurfaceHistoryState(initialSurface: AppSurface): SurfaceHistoryState {
  return {
    surfaces: [initialSurface],
    index: 0,
    didNavigateBeforeStorageReady: false,
  };
}

export function surfaceHistoryReducer(
  state: SurfaceHistoryState,
  action: SurfaceHistoryAction,
): SurfaceHistoryState {
  switch (action.type) {
    case 'storage-ready':
      return state.didNavigateBeforeStorageReady
        ? state
        : { ...state, surfaces: [action.surface], index: 0 };
    case 'navigation-intent':
      return state.didNavigateBeforeStorageReady ? state : { ...state, didNavigateBeforeStorageReady: true };
    case 'navigate': {
      const didNavigateBeforeStorageReady = true;
      if (state.surfaces[state.index] === action.surface) {
        return { ...state, didNavigateBeforeStorageReady };
      }
      const surfaces = [...state.surfaces.slice(0, state.index + 1), action.surface];
      if (surfaces.length > 50) surfaces.shift();
      return { surfaces, index: surfaces.length - 1, didNavigateBeforeStorageReady };
    }
    case 'move-in-history': {
      const nextIndex = state.index + action.offset;
      return {
        ...state,
        index: state.surfaces[nextIndex] ? nextIndex : state.index,
        didNavigateBeforeStorageReady: true,
      };
    }
  }
}
