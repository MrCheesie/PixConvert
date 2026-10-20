/** Phase 3 studio — audio conversion (ffmpeg) + waveform peaks (symphonia). */

import { createFormatSelect } from '../../components/FormatSelect';
import { createProgressBar } from '../../components/ProgressBar';
import { createSubTabs } from '../../components/SubTabs';
import { createTimeline } from '../../components/Timeline';
import { createToolStatus } from '../../components/ToolStatus';
import { createWaveform } from '../../components/Waveform';
import {
  button,
  field,
  h,
  numberInput,
  panel,
  selectBox,
  setStatus,
  statusLine,
  textInput,
  toggle,
} from '../../lib/dom';
import { AUDIO_EXTENSIONS, AUDIO_FORMATS } from '../../lib/formats';
import {
  audioWaveform,
  compressAudio,
  convertAudio,
  editAudio,
  errorMessage,
  onProgress,
  probeMedia,
  runningInTauri,
} from '../../lib/tauri';
import { isFullRange } from '../../lib/timeline';
import type { AudioEditOps, Studio } from '../../lib/types';
import { baseName, pathsWithExtensions } from '../../state';

const BITRATES = [
  { value: '96k', label: '96 kbps' },
  { value: '128k', label: '128 kbps' },
  { value: '192k', label: '192 kbps' },
  { value: '256k', label: '256 kbps' },
  { value: '320k', label: '320 kbps' },
];

export function createAudioStudio(): Studio {
  const root = h('section', { class: 'studio', 'data-studio': 'audio' });
  const progress = createProgressBar();
  void onProgress((event) => progress.set(event.done, event.total, baseName(event.current)));
  const tool = createToolStatus('ffmpeg', 'ffmpeg');
  const waveform = createWaveform();

  /* ------------------------------------------------------------ convert --- */
  const convertFormat = createFormatSelect(AUDIO_FORMATS);
  const convertBitrate = selectBox(BITRATES, '192k');
  const convertButton = button('Convert', () => void runConvert());
  const convertStatus = statusLine();

  const convertPanel = panel([
    h('div', { class: 'grid grid--2' }, [
      convertFormat.element,
      field('Bitrate', convertBitrate, 'Ignored for lossless formats'),
    ]),
    convertButton,
    convertStatus,
  ]);

  async function runConvert(): Promise<void> {
    const path = requireAudio(convertStatus);
    if (!path) return;
    try {
      const message = await convertAudio(path, convertFormat.getValue(), convertBitrate.value);
      setStatus(convertStatus, message, 'success');
    } catch (error) {
      setStatus(convertStatus, errorMessage(error), 'error');
    }
  }

  /* ----------------------------------------------------------- compress --- */
  const compressFormat = createFormatSelect(AUDIO_FORMATS.filter((f) => f.value !== 'wav' && f.value !== 'flac'));
  const compressBitrate = selectBox(BITRATES, '128k');
  const compressButton = button('Compress', () => void runCompress());
  const compressStatus = statusLine();

  const compressPanel = panel([
    h('div', { class: 'grid grid--2' }, [
      compressFormat.element,
      field('Bitrate', compressBitrate),
    ]),
    compressButton,
    compressStatus,
  ]);

  async function runCompress(): Promise<void> {
    const path = requireAudio(compressStatus);
    if (!path) return;
    try {
      const message = await compressAudio(path, compressFormat.getValue(), compressBitrate.value);
      setStatus(compressStatus, message, 'success');
    } catch (error) {
      setStatus(compressStatus, errorMessage(error), 'error');
    }
  }

  /* --------------------------------------------------------------- edit --- */
  let mediaDuration = 0;
  let syncingTrim = false;

  const trimEnable = toggle('Trim');
  const trimStart = numberInput(0, 0, 36000, 0.1);
  const trimEnd = numberInput(0, 0, 36000, 0.1);

  const timeline = createTimeline({
    label: 'Track timeline',
    onChange: (range) => {
      syncingTrim = true;
      trimStart.value = String(Number(range.start.toFixed(2)));
      trimEnd.value = String(Number(range.end.toFixed(2)));
      trimEnable.input.checked = mediaDuration > 0 && !isFullRange(range, mediaDuration);
      syncingTrim = false;
    },
  });

  const pushInputsToTimeline = (): void => {
    if (syncingTrim || mediaDuration <= 0) return;
    const start = Number(trimStart.value) || 0;
    const end = Number(trimEnd.value) || mediaDuration;
    timeline.setRange({ start, end });
    trimEnable.input.checked = !isFullRange({ start, end }, mediaDuration);
  };
  trimStart.addEventListener('input', pushInputsToTimeline);
  trimEnd.addEventListener('input', pushInputsToTimeline);

  const speedEnable = toggle('Change speed');
  const speedValue = numberInput(1.5, 0.25, 4, 0.05);

  const volumeEnable = toggle('Volume');
  const volumeDb = numberInput(0, -60, 20, 0.5);

  const fadeEnable = toggle('Fade in / out');
  const fadeIn = numberInput(1, 0, 60, 0.1);
  const fadeOut = numberInput(1, 0, 60, 0.1);

  const reverse = toggle('Reverse');
  const mixPath = textInput('/path/to/second-track.mp3', '');

  const editFormat = createFormatSelect([
    { value: '', label: 'Keep original format', title: 'Uses the input extension.' },
    ...AUDIO_FORMATS,
  ]);
  const applyButton = button('Apply edits', () => void runEdit());
  const editStatus = statusLine();

  const editPanel = panel([
    timeline.element,
    editFormat.element,
    h('fieldset', { class: 'fieldset' }, [
      trimEnable.element,
      h('div', { class: 'grid grid--2' }, [
        field('Start (s)', trimStart),
        field('End (s)', trimEnd, '0 keeps everything after start'),
      ]),
    ]),
    h('fieldset', { class: 'fieldset' }, [
      speedEnable.element,
      field('Speed multiplier', speedValue),
    ]),
    h('fieldset', { class: 'fieldset' }, [
      volumeEnable.element,
      field('Gain (dB)', volumeDb),
    ]),
    h('fieldset', { class: 'fieldset' }, [
      fadeEnable.element,
      h('div', { class: 'grid grid--2' }, [
        field('Fade in (s)', fadeIn),
        field('Fade out (s)', fadeOut),
      ]),
    ]),
    reverse.element,
    field('Mix in a second track', mixPath, 'Optional; mixed at 60% volume'),
    applyButton,
    editStatus,
  ]);

  async function runEdit(): Promise<void> {
    const path = requireAudio(editStatus);
    if (!path) return;

    const ops: AudioEditOps = {};
    const range = timeline.getRange();
    if (trimEnable.input.checked && mediaDuration > 0 && !isFullRange(range, mediaDuration)) {
      ops.trim = { start: range.start, end: range.end };
    }
    if (speedEnable.input.checked) ops.speed = Number(speedValue.value);
    if (volumeEnable.input.checked) ops.volumeDb = Number(volumeDb.value);
    if (fadeEnable.input.checked) {
      ops.fadeIn = Number(fadeIn.value);
      ops.fadeOut = Number(fadeOut.value);
    }
    if (reverse.input.checked) ops.reverse = true;
    if (mixPath.value.trim()) ops.mixPath = mixPath.value.trim();

    try {
      const message = await editAudio(path, ops, editFormat.getValue() || undefined);
      setStatus(editStatus, message, 'success');
    } catch (error) {
      setStatus(editStatus, errorMessage(error), 'error');
    }
  }

  /* ------------------------------------------------ waveform + timeline --- */
  async function loadMedia(): Promise<void> {
    const path = pathsWithExtensions(AUDIO_EXTENSIONS)[0] ?? '';
    waveform.clear();
    timeline.setMedia(0);
    mediaDuration = 0;
    if (!path || !runningInTauri()) return;

    try {
      const info = await probeMedia(path);
      mediaDuration = info.durationSecs;
      trimStart.value = '0';
      trimEnd.value = String(Number(mediaDuration.toFixed(2)));
      trimEnable.input.checked = false;
    } catch (error) {
      console.warn('Media probe failed:', errorMessage(error));
    }

    try {
      const peaks = await audioWaveform(path);
      waveform.setPeaks(peaks);
      timeline.setMedia(mediaDuration, peaks);
    } catch (error) {
      console.warn('Waveform failed:', errorMessage(error));
      timeline.setMedia(mediaDuration);
    }
  }

  const subtabs = createSubTabs(
    [
      { id: 'convert', label: 'Convert' },
      { id: 'compress', label: 'Compress' },
      { id: 'edit', label: 'Edit' },
    ],
    { convert: convertPanel, compress: compressPanel, edit: editPanel },
    'convert',
  );

  root.append(
    h('header', { class: 'studio__header' }, [
      h('h2', { class: 'studio__title' }, ['Audio Studio']),
      h('p', { class: 'studio__subtitle' }, ['ffmpeg converts; the waveform is decoded locally.']),
    ]),
    tool.element,
    waveform.element,
    subtabs.element,
    progress.element,
    convertPanel,
    compressPanel,
    editPanel,
  );

  let lastLoaded = '';
  return {
    id: 'audio',
    title: 'Audio',
    element: root,
    refresh: () => {
      const count = pathsWithExtensions(AUDIO_EXTENSIONS).length;
      const label = count === 0 ? 'No audio selected' : `${count} audio file(s) selected`;
      convertStatus.textContent = label;
      compressStatus.textContent = label;
      editStatus.textContent = label;

      const first = pathsWithExtensions(AUDIO_EXTENSIONS)[0] ?? '';
      if (first !== lastLoaded) {
        lastLoaded = first;
        void loadMedia();
      }
    },
  };
}

function requireAudio(status: HTMLElement): string | null {
  if (!runningInTauri()) {
    setStatus(status, 'The backend only runs inside Tauri — use `npm run tauri dev`.', 'error');
    return null;
  }
  const paths = pathsWithExtensions(AUDIO_EXTENSIONS);
  if (paths.length === 0) {
    setStatus(status, 'Select an audio file first.', 'error');
    return null;
  }
  return paths[0];
}
