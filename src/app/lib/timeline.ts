/** Pure timeline math — no DOM, unit-tested by `tests/timeline.test.ts`. */

export interface TimeRange {
  /** Seconds from the start of the clip. */
  start: number;
  /** Seconds from the start of the clip, always > start. */
  end: number;
}

/** Smallest selectable region, in seconds. */
export const MIN_RANGE = 0.05;

const TICK_STEPS = [0.1, 0.25, 0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300, 600, 1800, 3600];

export function clamp(value: number, min: number, max: number): number {
  if (Number.isNaN(value)) return min;
  return Math.min(max, Math.max(min, value));
}

/** `m:ss.t`, or `h:mm:ss.t` once the clip passes an hour. */
export function formatTimecode(seconds: number): string {
  const safe = Number.isFinite(seconds) ? Math.max(0, seconds) : 0;
  const hours = Math.floor(safe / 3600);
  const minutes = Math.floor((safe % 3600) / 60);
  const secs = safe % 60;
  const secsText = secs.toFixed(1).padStart(4, '0');
  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, '0')}:${secsText}`;
  }
  return `${minutes}:${secsText}`;
}

/** Short label for ruler ticks (drops the tenths on long clips). */
export function formatTick(seconds: number, duration: number): string {
  if (duration >= 600) {
    const minutes = Math.round(seconds / 60);
    return `${minutes}m`;
  }
  if (duration >= 60) {
    const minutes = Math.floor(seconds / 60);
    const secs = Math.round(seconds % 60);
    return `${minutes}:${String(secs).padStart(2, '0')}`;
  }
  return `${Number(seconds.toFixed(1))}s`;
}

/** Nicesh tick spacing so a ruler shows at most `maxTicks` labels. */
export function chooseTickStep(duration: number, maxTicks = 8): number {
  if (!(duration > 0)) return 1;
  for (const step of TICK_STEPS) {
    if (duration / step <= maxTicks) return step;
  }
  const last = TICK_STEPS[TICK_STEPS.length - 1];
  return Math.ceil(duration / maxTicks / last) * last;
}

export function percentOf(seconds: number, duration: number): number {
  if (!(duration > 0)) return 0;
  return clamp((seconds / duration) * 100, 0, 100);
}

export function secondsOf(percent: number, duration: number): number {
  if (!(duration > 0)) return 0;
  return clamp((percent / 100) * duration, 0, duration);
}

/** Clamp a range into `0..duration` and keep at least `minGap` between edges. */
export function normalizeRange(range: TimeRange, duration: number, minGap = MIN_RANGE): TimeRange {
  if (!(duration > 0)) return { start: 0, end: 0 };
  const gap = Math.min(minGap, duration);
  let start = clamp(range.start, 0, duration);
  let end = clamp(range.end, 0, duration);
  if (end < start) [start, end] = [end, start];
  if (end - start < gap) {
    if (start + gap <= duration) {
      end = start + gap;
    } else {
      end = duration;
      start = Math.max(0, duration - gap);
    }
  }
  return { start, end };
}

export function fullRange(duration: number): TimeRange {
  return { start: 0, end: Math.max(0, duration) };
}

export function rangeDuration(range: TimeRange): number {
  return Math.max(0, range.end - range.start);
}

export function isFullRange(range: TimeRange, duration: number, epsilon = 0.01): boolean {
  // An empty clip has no "full" selection — avoids treating duration 0 as trimmed=false.
  if (!(duration > 0)) return false;
  return range.start <= epsilon && range.end >= duration - epsilon;
}

/** Move the in-point; the out-point stays put. */
export function dragStart(
  range: TimeRange,
  seconds: number,
  duration: number,
  minGap = MIN_RANGE,
): TimeRange {
  const end = clamp(range.end, 0, duration);
  const start = clamp(seconds, 0, Math.max(0, end - minGap));
  return normalizeRange({ start, end }, duration, minGap);
}

/** Move the out-point; the in-point stays put. */
export function dragEnd(
  range: TimeRange,
  seconds: number,
  duration: number,
  minGap = MIN_RANGE,
): TimeRange {
  const start = clamp(range.start, 0, duration);
  const end = clamp(seconds, Math.min(duration, start + minGap), duration);
  return normalizeRange({ start, end }, duration, minGap);
}

/** Slide the whole selection, keeping it inside the clip. */
export function moveRange(
  range: TimeRange,
  deltaSeconds: number,
  duration: number,
  minGap = MIN_RANGE,
): TimeRange {
  if (!(duration > 0)) return { start: 0, end: 0 };
  const width = Math.min(rangeDuration(range), duration);
  const start = clamp(range.start + deltaSeconds, 0, Math.max(0, duration - width));
  return normalizeRange({ start, end: start + width }, duration, minGap);
}
