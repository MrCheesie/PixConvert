/** Canvas bar chart for the decoded audio peaks. */

import { h } from '../lib/dom';

export interface WaveformHandle {
  element: HTMLElement;
  setPeaks: (peaks: number[]) => void;
  clear: () => void;
}

export function createWaveform(): WaveformHandle {
  const canvas = h('canvas', { class: 'waveform__canvas' });
  const empty = h('p', { class: 'waveform__empty' }, ['Select an audio file to see its waveform.']);
  const element = h('div', { class: 'waveform' }, [canvas, empty]);

  let peaks: number[] = [];

  const draw = (): void => {
    const context = canvas.getContext('2d');
    if (!context) return;

    // Hidden studios report width 0; the ResizeObserver below redraws once the
    // panel is on screen.
    const width = element.clientWidth;
    if (width <= 0) return;
    const ratio = window.devicePixelRatio || 1;
    const height = 120;
    canvas.width = Math.floor(width * ratio);
    canvas.height = Math.floor(height * ratio);
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
    context.clearRect(0, 0, width, height);

    if (peaks.length === 0) {
      empty.hidden = false;
      return;
    }
    empty.hidden = true;

    const styles = getComputedStyle(document.documentElement);
    const accent = styles.getPropertyValue('--accent').trim() || '#5b8def';
    const mid = height / 2;
    const barWidth = width / peaks.length;

    context.strokeStyle = accent;
    context.lineWidth = Math.max(1, barWidth * 0.8);
    for (let i = 0; i < peaks.length; i += 1) {
      const amplitude = Math.max(0.02, peaks[i]) * (mid - 4);
      const x = i * barWidth + barWidth / 2;
      context.beginPath();
      context.moveTo(x, mid - amplitude);
      context.lineTo(x, mid + amplitude);
      context.stroke();
    }
  };

  const setPeaks = (next: number[]): void => {
    peaks = next;
    draw();
  };

  const clear = (): void => {
    peaks = [];
    draw();
  };

  const observer = new ResizeObserver(() => draw());
  observer.observe(element);
  queueMicrotask(draw);

  return { element, setPeaks, clear };
}
