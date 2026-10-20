/** Result list for batch jobs (successes and per-file errors). */

import { h } from '../lib/dom';
import type { BatchResult } from '../lib/types';
import { baseName } from '../state';

export interface BatchQueueHandle {
  element: HTMLElement;
  render: (results: BatchResult[]) => void;
  clear: () => void;
}

export function createBatchQueue(): BatchQueueHandle {
  const list = h('ul', { class: 'batch-list' });
  const element = h('div', { class: 'batch', hidden: true }, [
    h('h3', { class: 'batch__title' }, ['Batch results']),
    list,
  ]);

  const render = (results: BatchResult[]): void => {
    list.replaceChildren();
    element.hidden = results.length === 0;
    for (const result of results) {
      const ok = result.error === null;
      const detail = ok && result.output ? baseName(result.output) : result.error ?? '';
      list.append(
        h('li', { class: `batch-list__item ${ok ? 'is-ok' : 'is-error'}`, title: result.output ?? result.input }, [
          h('span', { class: 'batch-list__status' }, [ok ? '✓' : '✕']),
          h('span', { class: 'batch-list__name' }, [baseName(result.input)]),
          h('span', { class: 'batch-list__detail' }, [detail]),
        ]),
      );
    }
  };

  const clear = (): void => {
    list.replaceChildren();
    element.hidden = true;
  };

  return { element, render, clear };
}
