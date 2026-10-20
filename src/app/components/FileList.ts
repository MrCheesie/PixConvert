/** Chips for the current selection, with per-file removal. */

import { h } from '../lib/dom';
import { baseName, getState, removePath, clearPaths } from '../state';

export interface FileListHandle {
  element: HTMLElement;
  refresh: () => void;
}

export function createFileList(): FileListHandle {
  const list = h('div', { class: 'file-list__items' });
  const empty = h('p', { class: 'file-list__empty' }, ['No files selected yet.']);
  const clear = h('button', { class: 'btn btn--ghost', type: 'button' }, ['Clear all']);
  clear.addEventListener('click', () => clearPaths());

  const element = h('div', { id: 'fileList', class: 'file-list' }, [
    h('div', { class: 'file-list__header' }, [
      h('strong', {}, ['Selected files']),
      clear,
    ]),
    list,
    empty,
  ]);

  const refresh = (): void => {
    const paths = getState().selectedPaths;
    list.replaceChildren();
    empty.hidden = paths.length > 0;
    clear.hidden = paths.length === 0;

    for (const path of paths) {
      const remove = h('button', {
        class: 'chip__remove',
        type: 'button',
        title: `Remove ${baseName(path)}`,
      }, ['×']);
      remove.addEventListener('click', (event) => {
        event.stopPropagation();
        removePath(path);
      });
      const chip = h('span', { class: 'chip', title: path }, [
        h('span', { class: 'chip__name' }, [baseName(path)]),
        remove,
      ]);
      list.append(chip);
    }
  };

  refresh();
  return { element, refresh };
}
