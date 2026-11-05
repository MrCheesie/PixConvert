/**
 * Blocking overlay for conversion / compression jobs.
 *
 * It answers the two things the status line could not: which file is being
 * worked on, and how long it has left. Percentages come from ffmpeg
 * (`pixconvert://job-progress`) or from batch progress (`pixconvert://progress`);
 * while neither reports one the bar stays indeterminate and only the elapsed
 * time is shown.
 */

import { h } from '../lib/dom';
import { onJobProgress, onProgress } from '../lib/tauri';

interface Refs {
  root: HTMLElement;
  title: HTMLElement;
  detail: HTMLElement;
  track: HTMLElement;
  fill: HTMLElement;
  timing: HTMLElement;
}

let refs: Refs | null = null;
let visible = false;
let percent: number | null = null;
let startedAt = 0;
let ticker: number | null = null;
let lastFocus: HTMLElement | null = null;

function build(): Refs {
  if (refs) return refs;

  const title = h('h2', { class: 'loading__title', id: 'loadingTitle' }, ['Working…']);
  const detail = h('p', { class: 'loading__detail' }, ['']);
  const fill = h('div', { class: 'loading__fill is-indeterminate' });
  const track = h(
    'div',
    {
      class: 'loading__track',
      role: 'progressbar',
      'aria-valuemin': '0',
      'aria-valuemax': '100',
    },
    [fill],
  );
  const timing = h('p', { class: 'loading__timing' }, ['']);

  const root = h(
    'div',
    {
      class: 'loading',
      role: 'dialog',
      'aria-modal': 'true',
      'aria-labelledby': 'loadingTitle',
      'aria-busy': 'true',
      tabindex: '-1',
      hidden: true,
    },
    [
      h('div', { class: 'loading__card' }, [
        h('div', { class: 'loading__head' }, [
          h('div', { class: 'loading__spinner', 'aria-hidden': 'true' }),
          title,
        ]),
        detail,
        track,
        timing,
        h('p', { class: 'loading__hint' }, [
          'Everything is processed on this device — the window stays responsive.',
        ]),
      ]),
    ],
  );

  document.body.append(root);
  refs = { root, title, detail, track, fill, timing };

  // Subscribed once for the app's lifetime; both handlers no-op when hidden.
  void onProgress((event) => {
    if (!visible) return;
    setPercent(event.total > 0 ? (event.done / event.total) * 100 : null);
  });
  void onJobProgress((event) => {
    if (!visible) return;
    setPercent(event.percent);
  });

  return refs;
}

function setPercent(value: number | null): void {
  percent = value;
  const current = build();
  if (value === null) {
    current.fill.classList.add('is-indeterminate');
    current.fill.style.width = '';
    current.track.removeAttribute('aria-valuenow');
  } else {
    const clamped = Math.max(0, Math.min(100, value));
    current.fill.classList.remove('is-indeterminate');
    current.fill.style.width = `${clamped.toFixed(1)}%`;
    current.track.setAttribute('aria-valuenow', String(Math.round(clamped)));
  }
  renderTiming();
}

function renderTiming(): void {
  const current = refs;
  if (!current) return;
  if (!visible) {
    current.timing.textContent = '';
    return;
  }
  const elapsed = (performance.now() - startedAt) / 1000;
  const parts = [`${elapsed.toFixed(1)} s elapsed`];
  if (percent !== null && percent >= 3 && percent < 100) {
    const left = Math.max(0, elapsed / (percent / 100) - elapsed);
    parts.push(left >= 10 ? `about ${Math.round(left)} s left` : `about ${left.toFixed(1)} s left`);
  }
  current.timing.textContent = parts.join(' · ');
}

function show(title: string, detail: string): void {
  const current = build();
  lastFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  visible = true;
  startedAt = performance.now();
  current.title.textContent = title;
  current.detail.textContent = detail;
  current.detail.hidden = detail === '';
  current.root.hidden = false;
  setPercent(null);
  if (ticker !== null) window.clearInterval(ticker);
  ticker = window.setInterval(renderTiming, 250);
  current.root.focus();
}

function hide(): void {
  const current = refs;
  visible = false;
  percent = null;
  if (ticker !== null) {
    window.clearInterval(ticker);
    ticker = null;
  }
  if (!current) return;
  current.root.hidden = true;
  current.timing.textContent = '';
  if (lastFocus && document.contains(lastFocus)) lastFocus.focus();
  lastFocus = null;
}

/** Show the overlay until `task` settles, then hide it (even on failure). */
export async function runWithLoading<T>(
  title: string,
  task: () => Promise<T>,
  detail = '',
): Promise<T> {
  show(title, detail);
  try {
    return await task();
  } finally {
    hide();
  }
}
