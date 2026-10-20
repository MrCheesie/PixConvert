/** Range input with a live value bubble. */

import { h } from '../lib/dom';

export interface QualitySliderOptions {
  label: string;
  min?: number;
  max?: number;
  step?: number;
  value: number;
  onInput?: (value: number) => void;
}

export interface QualitySliderHandle {
  element: HTMLElement;
  getValue: () => number;
  setValue: (value: number) => void;
}

export function createQualitySlider(options: QualitySliderOptions): QualitySliderHandle {
  const { label, min = 1, max = 100, step = 1, value } = options;

  const input = h('input', {
    type: 'range',
    class: 'slider',
    min,
    max,
    step,
    value,
  });
  const output = h('output', { class: 'slider__value' }, [String(value)]);

  const sync = (): void => {
    output.textContent = input.value;
  };

  input.addEventListener('input', () => {
    sync();
    options.onInput?.(Number(input.value));
  });

  const element = h('label', { class: 'field' }, [
    h('span', { class: 'field__label' }, [h('span', {}, [label]), output]),
    input,
  ]);

  return {
    element,
    getValue: () => Number(input.value),
    setValue: (next: number) => {
      input.value = String(next);
      sync();
    },
  };
}
