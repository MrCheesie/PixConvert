/** Phase 2 studio — video, backed by ffmpeg. */

import { createFormatSelect } from '../../components/FormatSelect';
import { createProgressBar } from '../../components/ProgressBar';
import { createQualitySlider } from '../../components/QualitySlider';
import { createSubTabs } from '../../components/SubTabs';
import { createTimeline } from '../../components/Timeline';
import { createToolStatus } from '../../components/ToolStatus';
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
import { VIDEO_EXTENSIONS, VIDEO_FORMATS } from '../../lib/formats';
import {
  compressVideo,
  convertVideo,
  editVideo,
  errorMessage,
  onProgress,
  probeMedia,
  runningInTauri,
} from '../../lib/tauri';
import { isFullRange } from '../../lib/timeline';
import type { Studio, VideoEditOps } from '../../lib/types';
import { baseName, pathsWithExtensions } from '../../state';

export function createVideoStudio(): Studio {
  const root = h('section', { class: 'studio', 'data-studio': 'video' });
  const progress = createProgressBar();
  void onProgress((event) => progress.set(event.done, event.total, baseName(event.current)));
  const tool = createToolStatus('ffmpeg', 'ffmpeg');

  /* ------------------------------------------------------------ convert --- */
  const convertFormat = createFormatSelect(VIDEO_FORMATS);
  const convertQuality = createQualitySlider({ label: 'Quality', value: 75 });
  const convertButton = button('Convert', () => void runConvert());
  const convertStatus = statusLine();

  const convertPanel = panel([
    h('div', { class: 'grid grid--2' }, [convertFormat.element, convertQuality.element]),
    h('p', { class: 'hint' }, [
      'Choosing an audio format extracts the soundtrack; choosing GIF builds an animated GIF at 15 fps, 480 px wide.',
    ]),
    convertButton,
    convertStatus,
  ]);

  async function runConvert(): Promise<void> {
    const guard = requireFile(convertStatus);
    if (!guard) return;
    try {
      setStatus(convertStatus, `Converting ${baseName(guard)}…`);
      const message = await convertVideo(
        guard,
        convertFormat.getValue(),
        convertQuality.getValue(),
      );
      setStatus(convertStatus, message, 'success');
    } catch (error) {
      setStatus(convertStatus, errorMessage(error), 'error');
    }
  }

  /* ----------------------------------------------------------- compress --- */
  const compressFormat = createFormatSelect(
    VIDEO_FORMATS.filter((option) => !['gif', 'mp3'].includes(option.value)),
  );
  const crf = createQualitySlider({ label: 'CRF (lower = better quality)', min: 0, max: 51, value: 28 });
  const preset = selectBox(
    [
      { value: 'ultrafast', label: 'ultrafast' },
      { value: 'veryfast', label: 'veryfast' },
      { value: 'medium', label: 'medium' },
      { value: 'slow', label: 'slow' },
    ],
    'medium',
  );
  const compressButton = button('Compress', () => void runCompress());
  const compressStatus = statusLine();

  const compressPanel = panel([
    h('div', { class: 'grid grid--2' }, [compressFormat.element, field('Preset', preset)]),
    crf.element,
    compressButton,
    compressStatus,
  ]);

  async function runCompress(): Promise<void> {
    const guard = requireFile(compressStatus);
    if (!guard) return;
    try {
      const message = await compressVideo(guard, compressFormat.getValue(), crf.getValue(), preset.value);
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

  // The timeline is the source of truth for the trim range; the number inputs
  // mirror it so precise values can still be typed.
  const timeline = createTimeline({
    label: 'Clip timeline',
    onChange: (range) => {
      syncingTrim = true;
      trimStart.value = String(Number(range.start.toFixed(2)));
      trimEnd.value = String(Number(range.end.toFixed(2)));
      const trimmed = mediaDuration > 0 && !isFullRange(range, mediaDuration);
      trimEnable.input.checked = trimmed;
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

  const cropEnable = toggle('Crop');
  const cropX = numberInput(0, 0, 20000);
  const cropY = numberInput(0, 0, 20000);
  const cropW = numberInput(1280, 2, 20000);
  const cropH = numberInput(720, 2, 20000);

  const scaleEnable = toggle('Scale');
  const scaleW = numberInput(1280, 0, 20000);
  const scaleH = numberInput(720, 0, 20000);

  const rotateSelect = selectBox(
    [
      { value: '0', label: 'No rotation' },
      { value: '90', label: '90° clockwise' },
      { value: '180', label: '180°' },
      { value: '270', label: '270° clockwise' },
    ],
    '0',
  );
  const flipH = toggle('Flip horizontally');
  const flipV = toggle('Flip vertically');
  const speedEnable = toggle('Change speed');
  const speedValue = numberInput(1.5, 0.25, 4, 0.05);
  const mute = toggle('Mute audio');

  const textEnable = toggle('Add text');
  const textContent = textInput('Hello PixConvert');
  const textX = numberInput(24, 0, 20000);
  const textY = numberInput(24, 0, 20000);
  const textSize = numberInput(40, 4, 500);
  const textColor = textInput('white', 'white');

  const audioPath = textInput('/path/to/music.mp3', '');

  const editFormat = createFormatSelect(
    [{ value: '', label: 'Keep original container', title: 'Uses the input extension.' }, ...VIDEO_FORMATS.filter((o) => !['gif', 'mp3'].includes(o.value))],
  );

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
      cropEnable.element,
      h('div', { class: 'grid grid--4' }, [
        field('X', cropX),
        field('Y', cropY),
        field('Width', cropW),
        field('Height', cropH),
      ]),
    ]),
    h('fieldset', { class: 'fieldset' }, [
      scaleEnable.element,
      h('div', { class: 'grid grid--2' }, [field('Width', scaleW), field('Height', scaleH)]),
    ]),
    h('div', { class: 'grid grid--2' }, [
      field('Rotate', rotateSelect),
      h('div', { class: 'stack' }, [flipH.element, flipV.element, mute.element]),
    ]),
    h('fieldset', { class: 'fieldset' }, [
      speedEnable.element,
      field('Speed multiplier', speedValue),
    ]),
    h('fieldset', { class: 'fieldset' }, [
      textEnable.element,
      field('Text', textContent),
      h('div', { class: 'grid grid--4' }, [
        field('X', textX),
        field('Y', textY),
        field('Size', textSize),
        field('Colour', textColor),
      ]),
    ]),
    field('Replace audio with', audioPath, 'Optional path to a music file'),
    applyButton,
    editStatus,
  ]);

  async function runEdit(): Promise<void> {
    const guard = requireFile(editStatus);
    if (!guard) return;

    const ops: VideoEditOps = {};
    const range = timeline.getRange();
    if (trimEnable.input.checked && mediaDuration > 0 && !isFullRange(range, mediaDuration)) {
      ops.trim = { start: range.start, end: range.end };
    }
    if (cropEnable.input.checked) {
      ops.crop = {
        x: Number(cropX.value),
        y: Number(cropY.value),
        width: Number(cropW.value),
        height: Number(cropH.value),
      };
    }
    if (scaleEnable.input.checked) {
      ops.scale = { width: Number(scaleW.value), height: Number(scaleH.value) };
    }
    if (rotateSelect.value !== '0') ops.rotate = Number(rotateSelect.value);
    if (flipH.input.checked) ops.flipH = true;
    if (flipV.input.checked) ops.flipV = true;
    if (speedEnable.input.checked) ops.speed = Number(speedValue.value);
    if (mute.input.checked) ops.mute = true;
    if (textEnable.input.checked) {
      ops.text = {
        content: textContent.value,
        x: Number(textX.value),
        y: Number(textY.value),
        size: Number(textSize.value),
        color: textColor.value || 'white',
      };
    }
    if (audioPath.value.trim()) ops.audioPath = audioPath.value.trim();

    try {
      const message = await editVideo(guard, ops, editFormat.getValue() || undefined);
      setStatus(editStatus, message, 'success');
    } catch (error) {
      setStatus(editStatus, errorMessage(error), 'error');
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
      h('h2', { class: 'studio__title' }, ['Video Studio']),
      h('p', { class: 'studio__subtitle' }, ['Conversion and editing are done by ffmpeg on this machine.']),
    ]),
    tool.element,
    subtabs.element,
    progress.element,
    convertPanel,
    compressPanel,
    editPanel,
  );

  // Timeline data for the first selected clip (single-file editing only).
  let probedPath = '';
  async function loadTimeline(): Promise<void> {
    const first = pathsWithExtensions(VIDEO_EXTENSIONS)[0] ?? '';
    if (first === probedPath) return;
    probedPath = first;
    if (!first || !runningInTauri()) {
      mediaDuration = 0;
      timeline.setMedia(0);
      return;
    }
    try {
      const info = await probeMedia(first);
      mediaDuration = info.durationSecs;
      timeline.setMedia(mediaDuration);
      trimStart.value = '0';
      trimEnd.value = String(Number(mediaDuration.toFixed(2)));
      trimEnable.input.checked = false;
    } catch (error) {
      mediaDuration = 0;
      timeline.setMedia(0);
      console.warn('Timeline probe failed:', errorMessage(error));
    }
  }

  return {
    id: 'video',
    title: 'Video',
    element: root,
    refresh: () => {
      const count = pathsWithExtensions(VIDEO_EXTENSIONS).length;
      const label = count === 0 ? 'No videos selected' : `${count} video(s) selected`;
      convertStatus.textContent = label;
      compressStatus.textContent = label;
      editStatus.textContent = label;
      void loadTimeline();
    },
  };
}

function requireFile(status: HTMLElement): string | null {
  if (!runningInTauri()) {
    setStatus(status, 'The backend only runs inside Tauri — use `npm run tauri dev`.', 'error');
    return null;
  }
  const paths = pathsWithExtensions(VIDEO_EXTENSIONS);
  if (paths.length === 0) {
    setStatus(status, 'Select a video file first.', 'error');
    return null;
  }
  return paths[0];
}
