/** Before/after comparison with a draggable divider. */

import { h } from '../lib/dom';

export interface ComparisonSliderHandle {
  element: HTMLElement;
  setImages: (beforeUrl: string, afterUrl: string) => void;
}

export function createComparisonSlider(beforeUrl = '', afterUrl = ''): ComparisonSliderHandle {
  const before = h('img', { class: 'compare__image', alt: 'Original', draggable: false });
  const after = h('img', { class: 'compare__image compare__image--after', alt: 'Converted', draggable: false });
  const handle = h('div', { class: 'compare__handle' }, [h('span', { class: 'compare__grip' }, ['⇔'])]);
  const frame = h('div', { class: 'compare' }, [before, after, handle]);
  const hint = h('p', { class: 'compare__hint' }, [
    'Drag the divider to compare the original (left) with the result (right).',
  ]);

  let position = 50;
  const apply = (): void => {
    after.style.clipPath = `inset(0 0 0 ${position}%)`;
    handle.style.left = `${position}%`;
  };

  const setFromClientX = (clientX: number): void => {
    const rect = frame.getBoundingClientRect();
    if (rect.width === 0) return;
    position = Math.min(100, Math.max(0, ((clientX - rect.left) / rect.width) * 100));
    apply();
  };

  let dragging = false;
  const stop = (): void => {
    dragging = false;
    handle.classList.remove('is-dragging');
  };

  frame.addEventListener('pointerdown', (event) => {
    dragging = true;
    handle.classList.add('is-dragging');
    setFromClientX(event.clientX);
  });
  window.addEventListener('pointermove', (event) => {
    if (dragging) setFromClientX(event.clientX);
  });
  window.addEventListener('pointerup', stop);

  const setImages = (beforeSrc: string, afterSrc: string): void => {
    before.src = beforeSrc;
    after.src = afterSrc;
    position = 50;
    apply();
  };

  if (beforeUrl || afterUrl) setImages(beforeUrl, afterUrl);
  apply();

  const element = h('div', { class: 'compare-wrap', hidden: true }, [frame, hint]);
  return {
    element,
    setImages: (beforeSrc, afterSrc) => {
      setImages(beforeSrc, afterSrc);
      element.hidden = !beforeSrc || !afterSrc;
    },
  };
}
