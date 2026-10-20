/** Thin progress bar driven by `pixconvert://progress` / tool downloads. */

import { h } from '../lib/dom';

export interface ProgressBarHandle {
  element: HTMLElement;
  set: (done: number, total: number, label?: string) => void;
  reset: () => void;
}

export function createProgressBar(): ProgressBarHandle {
  const fill = h('div', { class: 'progress__fill' });
  const text = h('span', { class: 'progress__text' }, ['']);
  const element = h('div', { class: 'progress', hidden: true }, [
    h('div', { class: 'progress__track' }, [fill]),
    text,
  ]);

  const set = (done: number, total: number, label = ''): void => {
    element.hidden = false;
    const ratio = total > 0 ? Math.min(1, done / total) : 0;
    fill.style.width = `${Math.round(ratio * 100)}%`;
    text.textContent = total > 0
      ? `${done} / ${total}${label ? ` — ${label}` : ''}`
      : label;
  };

  const reset = (): void => {
    element.hidden = true;
    fill.style.width = '0%';
    text.textContent = '';
  };

  return { element, set, reset };
}
