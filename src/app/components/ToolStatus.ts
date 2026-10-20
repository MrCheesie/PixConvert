/** Shows whether ffmpeg/pandoc is available and offers a download button. */

import { button, h } from '../lib/dom';
import { detectTools, ensureTool, errorMessage, onToolDownload, runningInTauri } from '../lib/tauri';

export type ToolName = 'ffmpeg' | 'pandoc';

export interface ToolStatusHandle {
  element: HTMLElement;
  refresh: () => Promise<void>;
}

export function createToolStatus(tool: ToolName, label: string): ToolStatusHandle {
  const stateText = h('span', { class: 'tool-status__state' }, ['Checking…']);
  const note = h('span', { class: 'tool-status__note' }, ['']);
  const download = button(`Download ${label}`, () => void install(), 'secondary');
  download.hidden = true;

  const element = h('div', { class: 'tool-status' }, [
    h('span', { class: `tool-status__dot tool-status__dot--${tool}` }),
    h('strong', {}, [label]),
    stateText,
    note,
    download,
  ]);

  async function install(): Promise<void> {
    download.disabled = true;
    note.textContent = 'Downloading…';
    try {
      const path = await ensureTool(tool);
      stateText.textContent = path;
      note.textContent = '';
      download.hidden = true;
      element.classList.remove('is-missing');
    } catch (error) {
      note.textContent = errorMessage(error);
    } finally {
      download.disabled = false;
    }
  }

  async function refresh(): Promise<void> {
    if (!runningInTauri()) {
      stateText.textContent = 'backend unavailable (browser preview)';
      element.classList.add('is-missing');
      return;
    }
    try {
      const status = await detectTools();
      const path = tool === 'ffmpeg' ? status.ffmpeg : status.pandoc;
      if (path) {
        stateText.textContent = path;
        element.classList.remove('is-missing');
        download.hidden = true;
      } else {
        stateText.textContent = 'not found';
        note.textContent = 'Required for this studio.';
        element.classList.add('is-missing');
        download.hidden = false;
      }
    } catch (error) {
      stateText.textContent = errorMessage(error);
      element.classList.add('is-missing');
    }
  }

  void onToolDownload((event) => {
    if (event.name !== tool) return;
    const percent = event.total > 0 ? Math.round((event.done / event.total) * 100) : 0;
    note.textContent = `Downloading… ${percent}%`;
  });

  void refresh();
  return { element, refresh };
}
