/**
 * “Open With” bridge: files opened from Finder / Explorer / the shell land in
 * the selection and switch to the studio that understands them.
 */

import {
  AUDIO_EXTENSIONS,
  DOC_EXTENSIONS,
  IMAGE_EXTENSIONS,
  VIDEO_EXTENSIONS,
} from './lib/formats';
import { onOpenFiles, runningInTauri, takeOpenFiles } from './lib/tauri';
import type { StudioId } from './lib/types';
import { addPaths, extensionOf, getState, setActiveStudio } from './state';

/** Studio that owns `extension`, or `null` for unsupported files. */
export function studioForExtension(extension: string): StudioId | null {
  const ext = extension.toLowerCase();
  if (IMAGE_EXTENSIONS.includes(ext)) return 'image';
  if (VIDEO_EXTENSIONS.includes(ext)) return 'video';
  if (AUDIO_EXTENSIONS.includes(ext)) return 'audio';
  if (DOC_EXTENSIONS.includes(ext)) return 'doc';
  return null;
}

/** Add the paths that are not selected yet and focus the matching studio. */
export function acceptOpenPaths(paths: string[]): void {
  const fresh = paths.filter((path) => !getState().selectedPaths.includes(path));
  if (fresh.length === 0) return;

  addPaths(fresh);
  const studio = studioForExtension(extensionOf(fresh[0]));
  if (studio) setActiveStudio(studio);
}

export async function initOpenWith(): Promise<void> {
  if (!runningInTauri()) return;
  try {
    // Listen first, then drain: anything that arrives in between is queued by
    // the backend and picked up by `takeOpenFiles`.
    await onOpenFiles((paths) => acceptOpenPaths(paths));
    acceptOpenPaths(await takeOpenFiles());
  } catch (error) {
    console.error('Open-with listener failed:', error);
  }
}
