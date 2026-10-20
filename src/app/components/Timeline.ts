/** Single-clip timeline: ruler, optional waveform, in/out handles, playhead.
 *
 * Backs the trim controls in the Video and Audio studios. All the maths lives in
 * `lib/timeline.ts` so it stays testable without a DOM.
 */

import { h } from '../lib/dom';
import {
  chooseTickStep,
  clamp,
  dragEnd,
  dragStart,
  formatTick,
  formatTimecode,
  fullRange,
  isFullRange,
  moveRange,
  normalizeRange,
  percentOf,
  rangeDuration,
  type TimeRange,
} from '../lib/timeline';

export interface TimelineOptions {
  label?: string;
  duration?: number;
  peaks?: number[];
  onChange?: (range: TimeRange) => void;
}

export interface TimelineHandle {
  element: HTMLElement;
  setMedia: (duration: number, peaks?: number[]) => void;
  setPeaks: (peaks: number[]) => void;
  setRange: (range: TimeRange) => void;
  getRange: () => TimeRange;
  selectAll: () => void;
}

type DragMode = 'start' | 'end' | 'move' | 'scrub';

export function createTimeline(options: TimelineOptions = {}): TimelineHandle {
  let duration = options.duration ?? 0;
  let peaks = options.peaks ?? [];
  let range: TimeRange = fullRange(duration);

  const label = h('span', { class: 'timeline__label' }, [options.label ?? 'Timeline']);
  const readout = h('span', { class: 'timeline__readout' }, ['No media selected']);

  const canvas = h('canvas', { class: 'timeline__canvas' });
  const selection = h('div', { class: 'timeline__selection' }, [
    h('div', { class: 'timeline__handle timeline__handle--start', 'data-handle': 'start' }),
    h('div', { class: 'timeline__handle timeline__handle--end', 'data-handle': 'end' }),
  ]);
  const playhead = h('div', { class: 'timeline__playhead', hidden: true });
  const track = h('div', { class: 'timeline__track', role: 'presentation' }, [
    canvas,
    selection,
    playhead,
  ]);
  const selectAllButton = h('button', { class: 'btn btn--ghost', type: 'button' }, ['Select all']);

  const element = h('div', { class: 'timeline' }, [
    h('div', { class: 'timeline__head' }, [label, readout]),
    track,
    h('div', { class: 'timeline__foot' }, [
      h('span', { class: 'timeline__hint' }, [
        'Drag the handles to set the trim range; click the ruler to scrub.',
      ]),
      selectAllButton,
    ]),
  ]);

  const emit = (): void => options.onChange?.(range);

  const render = (): void => {
    const usable = duration > 0;
    element.classList.toggle('is-empty', !usable);

    if (!usable) {
      readout.textContent = 'No media selected';
      selection.hidden = true;
      playhead.hidden = true;
      drawTrack();
      return;
    }

    selection.hidden = false;
    const left = percentOf(range.start, duration);
    const right = percentOf(range.end, duration);
    selection.style.left = `${left}%`;
    selection.style.width = `${Math.max(0.4, right - left)}%`;
    readout.textContent = `${formatTimecode(range.start)} – ${formatTimecode(range.end)} · ${rangeDuration(range).toFixed(2)}s selected of ${formatTimecode(duration)}`;
    drawTrack();
  };

  const drawTrack = (): void => {
    const context = canvas.getContext('2d');
    if (!context) return;
    // Studios are mounted hidden, so the first draw happens at width 0. The
    // ResizeObserver below redraws as soon as the panel becomes visible.
    const width = track.clientWidth;
    if (width <= 0) return;
    const ratio = window.devicePixelRatio || 1;
    const height = 76;
    canvas.width = Math.floor(width * ratio);
    canvas.height = Math.floor(height * ratio);
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
    context.clearRect(0, 0, width, height);

    const styles = getComputedStyle(document.documentElement);
    const accent = styles.getPropertyValue('--accent').trim() || '#5b8def';
    const border = styles.getPropertyValue('--border').trim() || '#242c3a';
    const muted = styles.getPropertyValue('--text-faint').trim() || '#6b768a';
    const sunken = styles.getPropertyValue('--bg-sunken').trim() || '#0a0d12';

    // Clip body
    context.fillStyle = sunken;
    context.fillRect(0, 0, width, height);

    if (peaks.length > 0 && duration > 0) {
      const mid = height / 2;
      const barWidth = width / peaks.length;
      context.fillStyle = accent;
      context.globalAlpha = 0.55;
      for (let i = 0; i < peaks.length; i += 1) {
        const amplitude = Math.max(0.02, peaks[i]) * (mid - 8);
        context.fillRect(i * barWidth, mid - amplitude, Math.max(1, barWidth * 0.75), amplitude * 2);
      }
      context.globalAlpha = 1;
    } else if (duration > 0) {
      // Video clips have no peaks: draw a muted band so the track reads as media.
      context.fillStyle = border;
      context.fillRect(0, height / 2 - 10, width, 20);
    }

    // Ruler ticks
    if (duration > 0) {
      const step = chooseTickStep(duration);
      context.strokeStyle = border;
      context.fillStyle = muted;
      context.font = '10px Inter, sans-serif';
      context.lineWidth = 1;
      for (let t = 0; t <= duration + 1e-6; t += step) {
        const x = (t / duration) * width;
        context.beginPath();
        context.moveTo(x, height - 14);
        context.lineTo(x, height);
        context.stroke();
        const tickLabel = formatTick(t, duration);
        context.fillText(tickLabel, Math.min(width - 24, x + 3), height - 18);
      }
    }
  };

  const secondsFromEvent = (clientX: number): number => {
    const rect = track.getBoundingClientRect();
    if (rect.width === 0 || duration <= 0) return 0;
    return clamp(((clientX - rect.left) / rect.width) * duration, 0, duration);
  };

  let dragMode: DragMode | null = null;
  let dragOrigin = { x: 0, range: fullRange(0) };

  track.addEventListener('pointerdown', (event) => {
    if (duration <= 0) return;
    const target = event.target as HTMLElement;
    const handle = target.dataset.handle as 'start' | 'end' | undefined;
    const seconds = secondsFromEvent(event.clientX);

    if (handle === 'start' || handle === 'end') {
      dragMode = handle;
    } else if (target === selection || selection.contains(target)) {
      dragMode = 'move';
    } else {
      dragMode = 'scrub';
      playhead.hidden = false;
      playhead.style.left = `${percentOf(seconds, duration)}%`;
      return;
    }

    dragOrigin = { x: event.clientX, range: { ...range } };
    track.setPointerCapture(event.pointerId);
    track.classList.add('is-dragging');
    event.preventDefault();
  });

  track.addEventListener('pointermove', (event) => {
    if (!dragMode || duration <= 0) return;
    if (dragMode === 'scrub') {
      playhead.style.left = `${percentOf(secondsFromEvent(event.clientX), duration)}%`;
      return;
    }
    const seconds = secondsFromEvent(event.clientX);
    if (dragMode === 'start') {
      range = dragStart(range, seconds, duration);
    } else if (dragMode === 'end') {
      range = dragEnd(range, seconds, duration);
    } else {
      const rect = track.getBoundingClientRect();
      const deltaSeconds = rect.width > 0 ? ((event.clientX - dragOrigin.x) / rect.width) * duration : 0;
      range = moveRange(dragOrigin.range, deltaSeconds, duration);
    }
    render();
    emit();
  });

  const stopDrag = (): void => {
    if (!dragMode) return;
    dragMode = null;
    track.classList.remove('is-dragging');
  };
  track.addEventListener('pointerup', stopDrag);
  track.addEventListener('pointercancel', stopDrag);

  track.addEventListener('dblclick', () => selectAll());
  selectAllButton.addEventListener('click', () => selectAll());

  function selectAll(): void {
    if (duration <= 0) return;
    range = fullRange(duration);
    render();
    emit();
  }

  const setMedia = (nextDuration: number, nextPeaks?: number[]): void => {
    const wasFull = duration <= 0 || isFullRange(range, duration);
    duration = Math.max(0, Number.isFinite(nextDuration) ? nextDuration : 0);
    if (nextPeaks) peaks = nextPeaks;
    range = wasFull ? fullRange(duration) : normalizeRange(range, duration);
    if (duration <= 0) peaks = [];
    render();
  };

  const setPeaks = (nextPeaks: number[]): void => {
    peaks = nextPeaks;
    render();
  };

  const setRange = (next: TimeRange): void => {
    if (duration <= 0) return;
    const clamped = normalizeRange(next, duration);
    if (Math.abs(clamped.start - range.start) < 1e-6 && Math.abs(clamped.end - range.end) < 1e-6) {
      return;
    }
    range = clamped;
    render();
  };

  const observer = new ResizeObserver(() => drawTrack());
  observer.observe(track);
  queueMicrotask(render);

  return {
    element,
    setMedia,
    setPeaks,
    setRange,
    getRange: () => ({ ...range }),
    selectAll,
  };
}
