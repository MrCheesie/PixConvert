/** Tiny observable store shared by every studio (no framework). */

import type { StudioId, SubTabId } from './lib/types';

export interface AppState {
  selectedPaths: string[];
  activeStudio: StudioId;
  activeSubTab: SubTabId;
}

type Listener = () => void;

const state: AppState = {
  selectedPaths: [],
  activeStudio: 'image',
  activeSubTab: 'convert',
};

const listeners = new Set<Listener>();

function notify(): void {
  listeners.forEach((listener) => listener());
}

export function getState(): AppState {
  return state;
}

export function subscribe(listener: Listener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function setPaths(paths: string[]): void {
  state.selectedPaths = paths;
  notify();
}

export function addPaths(paths: string[]): void {
  const merged = [...state.selectedPaths];
  for (const path of paths) {
    if (!merged.includes(path)) merged.push(path);
  }
  state.selectedPaths = merged;
  notify();
}

export function removePath(path: string): void {
  state.selectedPaths = state.selectedPaths.filter((item) => item !== path);
  notify();
}

export function clearPaths(): void {
  state.selectedPaths = [];
  notify();
}

export function setActiveStudio(studio: StudioId): void {
  state.activeStudio = studio;
  notify();
}

export function setActiveSubTab(tab: SubTabId): void {
  state.activeSubTab = tab;
  notify();
}

/** File name without its directory. */
export function baseName(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

/** Lower-case extension without the dot. */
export function extensionOf(path: string): string {
  const name = baseName(path);
  const dot = name.lastIndexOf('.');
  return dot === -1 ? '' : name.slice(dot + 1).toLowerCase();
}

/** Paths in the selection that this studio understands. */
export function pathsWithExtensions(extensions: string[]): string[] {
  return state.selectedPaths.filter((path) => extensions.includes(extensionOf(path)));
}
