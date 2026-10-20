/** Drag & drop + native dialog file picker (shared by all studios). */

import { getCurrentWebview } from '@tauri-apps/api/webview';
import { open } from '@tauri-apps/plugin-dialog';

import { h } from '../lib/dom';
import { runningInTauri } from '../lib/tauri';

export interface DropZoneOptions {
  /** Extensions the picker should show for the active studio. */
  getExtensions: () => string[];
  /** Studio label used in the picker filter name. */
  getLabel: () => string;
  onPaths: (paths: string[]) => void;
}

export function createDropZone(options: DropZoneOptions): HTMLElement {
  const input = h('input', { type: 'file', multiple: true, class: 'drop-zone__input' });
  const prompt = h('span', { class: 'drop-zone__prompt' }, [
    'Drag & drop files here, or click to choose',
  ]);
  const zone = h('div', { id: 'dropZone', class: 'drop-zone' }, [prompt, input]);

  input.addEventListener('change', () => {
    if (!input.files) return;
    // Browser mode has no absolute paths; the backend cannot read File objects.
    const names = Array.from(input.files).map((file) => file.name);
    options.onPaths(names);
    input.value = '';
  });

  zone.addEventListener('click', async () => {
    if (!runningInTauri()) {
      input.click();
      return;
    }
    const selection = await open({
      multiple: true,
      filters: [{ name: options.getLabel(), extensions: options.getExtensions() }],
    });
    if (Array.isArray(selection)) options.onPaths(selection);
    else if (typeof selection === 'string') options.onPaths([selection]);
  });

  zone.addEventListener('dragover', (event) => {
    event.preventDefault();
    zone.classList.add('drop-zone--over');
  });

  ['dragleave', 'dragend'].forEach((type) => {
    zone.addEventListener(type, () => zone.classList.remove('drop-zone--over'));
  });

  zone.addEventListener('drop', (event) => {
    event.preventDefault();
    zone.classList.remove('drop-zone--over');
    if (event.dataTransfer?.files.length) {
      options.onPaths(Array.from(event.dataTransfer.files).map((file) => file.name));
    }
  });

  // Tauri intercepts HTML5 drops, so native paths arrive through the webview.
  if (runningInTauri()) {
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === 'enter' || event.payload.type === 'over') {
          zone.classList.add('drop-zone--over');
        } else if (event.payload.type === 'leave') {
          zone.classList.remove('drop-zone--over');
        } else if (event.payload.type === 'drop') {
          zone.classList.remove('drop-zone--over');
          options.onPaths(event.payload.paths);
        }
      })
      .catch((error: unknown) => console.error('Drag-drop listener failed:', error));
  }

  return zone;
}
