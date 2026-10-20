/** Dropdown built from the format matrix. */

import { h } from '../lib/dom';
import type { FormatOption } from '../lib/types';

export interface FormatSelectHandle {
  element: HTMLElement;
  getValue: () => string;
  setOptions: (options: FormatOption[]) => void;
}

export function createFormatSelect(
  options: FormatOption[],
  label = 'Output format',
  onChange?: (value: string) => void,
): FormatSelectHandle {
  const select = h('select', { id: 'formatSelect', class: 'input' });
  const title = h('span', { class: 'field__hint' }, ['']);

  const setOptions = (next: FormatOption[]): void => {
    const previous = select.value;
    select.replaceChildren();
    for (const option of next) {
      const node = h('option', { value: option.value, title: option.title }, [option.label]);
      select.append(node);
    }
    if (next.some((option) => option.value === previous)) select.value = previous;
    syncHint();
  };

  const syncHint = (): void => {
    const current = options.find((option) => option.value === select.value);
    title.textContent = current ? current.title : '';
  };

  select.addEventListener('change', () => {
    syncHint();
    onChange?.(select.value);
  });

  setOptions(options);

  const element = h('label', { class: 'field' }, [
    h('span', { class: 'field__label' }, [label]),
    select,
    title,
  ]);

  return {
    element,
    getValue: () => select.value,
    setOptions: (next) => {
      options = next;
      setOptions(next);
    },
  };
}
