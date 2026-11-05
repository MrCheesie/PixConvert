/** Phase 1 studio — images, handled entirely in Rust. */

import { createBatchQueue } from '../../components/BatchQueue';
import { createComparisonSlider } from '../../components/ComparisonSlider';
import { createFormatSelect } from '../../components/FormatSelect';
import { runWithLoading } from '../../components/LoadingOverlay';
import { createProgressBar } from '../../components/ProgressBar';
import { createQualitySlider } from '../../components/QualitySlider';
import { createSubTabs } from '../../components/SubTabs';
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
import { IMAGE_EXTENSIONS, IMAGE_FORMATS, VECTOR_FORMAT } from '../../lib/formats';
import {
  backgroundRemovalAvailable,
  batchCompress,
  batchConvert,
  compressImage,
  convertImage,
  editImage,
  errorMessage,
  fileUrl,
  onProgress,
  removeBackground,
  runningInTauri,
  vectorizeImage,
} from '../../lib/tauri';
import type { ImageEditOps, Studio } from '../../lib/types';
import { baseName, extensionOf, getState, pathsWithExtensions } from '../../state';

export function createImageStudio(): Studio {
  const root = h('section', { class: 'studio', 'data-studio': 'image' });
  // One bar per studio: batch progress arrives from the shared event bus.
  const progress = createProgressBar();
  void onProgress((event) => progress.set(event.done, event.total, baseName(event.current)));

  /* ------------------------------------------------------------ convert --- */
  const convertFormat = createFormatSelect([...IMAGE_FORMATS, VECTOR_FORMAT]);
  const convertQuality = createQualitySlider({ label: 'Quality', value: 85 });
  const convertBatch = toggle('Batch the whole selection');
  const convertButton = button('Convert', () => void runConvert());
  const convertStatus = statusLine();
  const convertResults = createBatchQueue();
  const compare = createComparisonSlider();

  const syncQualityVisibility = (): void => {
    const fmt = convertFormat.getValue();
    convertQuality.element.hidden = !['jpeg', 'avif', 'heic', 'jxl'].includes(fmt);
  };
  convertFormat.element.querySelector('select')?.addEventListener('change', syncQualityVisibility);
  syncQualityVisibility();

  const convertPanel = panel([
    h('div', { class: 'grid grid--2' }, [convertFormat.element, convertQuality.element]),
    h('p', { class: 'hint' }, [
      'Vectorize writes an SVG and needs a build with the `vectorize` feature. HEIC/JXL output requires ffmpeg.',
    ]),
    convertBatch.element,
    convertButton,
    convertStatus,
    convertResults.element,
    compare.element,
  ]);

  async function runConvert(): Promise<void> {
    if (!runningInTauri()) {
      setStatus(convertStatus, 'The backend only runs inside Tauri — use `npm run tauri dev`.', 'error');
      return;
    }
    const paths = pathsWithExtensions(IMAGE_EXTENSIONS);
    if (paths.length === 0) {
      setStatus(convertStatus, 'Select at least one image first.', 'error');
      return;
    }

    const format = convertFormat.getValue();
    const quality = convertQuality.getValue();
    convertResults.clear();
    compare.element.hidden = true;

    try {
      if (format === 'svg') {
        const messages = await runWithLoading(`Vectorizing ${paths.length} image(s)`, async () => {
          const lines: string[] = [];
          for (const path of paths) {
            lines.push(await vectorizeImage(path));
          }
          return lines;
        });
        setStatus(convertStatus, messages.join('\n'), 'success');
        return;
      }

      if (convertBatch.input.checked) {
        const results = await runWithLoading(`Converting ${paths.length} images`, () =>
          batchConvert(paths, format, quality),
        );
        convertResults.render(results);
        const failures = results.filter((result) => result.error !== null).length;
        setStatus(
          convertStatus,
          `Converted ${results.length - failures} of ${results.length} files.`,
          failures > 0 ? 'error' : 'success',
        );
        return;
      }

      const path = paths[0];
      setStatus(convertStatus, `Converting ${baseName(path)}…`);
      const message = await runWithLoading(
        `Converting ${baseName(path)}`,
        () => convertImage(path, format, quality),
        format.toUpperCase(),
      );
      setStatus(convertStatus, message, 'success');
      compare.setImages(fileUrl(path), fileUrl(outputPathFrom(message, path, format, 'converted')));
    } catch (error) {
      setStatus(convertStatus, errorMessage(error), 'error');
    }
  }

  /* ----------------------------------------------------------- compress --- */
  const compressFormat = createFormatSelect(IMAGE_FORMATS.filter((f) => f.value !== 'jxl'));
  const compressQuality = createQualitySlider({ label: 'Quality', value: 60 });
  const maxWidth = numberInput(0, 0, 20000);
  const maxHeight = numberInput(0, 0, 20000);
  const compressBatch = toggle('Batch the whole selection');
  const compressButton = button('Compress', () => void runCompress());
  const compressStatus = statusLine();
  const compressResults = createBatchQueue();

  const compressPanel = panel([
    h('div', { class: 'grid grid--2' }, [compressFormat.element, compressQuality.element]),
    h('div', { class: 'grid grid--2' }, [
      field('Max width (px)', maxWidth, '0 keeps the original width'),
      field('Max height (px)', maxHeight, '0 keeps the original height'),
    ]),
    compressBatch.element,
    compressButton,
    compressStatus,
    compressResults.element,
  ]);

  async function runCompress(): Promise<void> {
    if (!runningInTauri()) {
      setStatus(compressStatus, 'The backend only runs inside Tauri — use `npm run tauri dev`.', 'error');
      return;
    }
    const paths = pathsWithExtensions(IMAGE_EXTENSIONS);
    if (paths.length === 0) {
      setStatus(compressStatus, 'Select at least one image first.', 'error');
      return;
    }
    const format = compressFormat.getValue();
    const quality = compressQuality.getValue();
    const width = Number(maxWidth.value) || undefined;
    const height = Number(maxHeight.value) || undefined;
    compressResults.clear();

    try {
      if (compressBatch.input.checked) {
        const results = await runWithLoading(`Compressing ${paths.length} images`, () =>
          batchCompress(paths, format, quality, width, height),
        );
        compressResults.render(results);
        const failures = results.filter((result) => result.error !== null).length;
        setStatus(
          compressStatus,
          `Compressed ${results.length - failures} of ${results.length} files.`,
          failures > 0 ? 'error' : 'success',
        );
        return;
      }
      setStatus(compressStatus, `Compressing ${baseName(paths[0])}…`);
      const message = await runWithLoading(
        `Compressing ${baseName(paths[0])}`,
        () => compressImage(paths[0], format, quality, width, height),
        format.toUpperCase(),
      );
      setStatus(compressStatus, message, 'success');
    } catch (error) {
      setStatus(compressStatus, errorMessage(error), 'error');
    }
  }

  /* --------------------------------------------------------------- edit --- */
  const editFormat = createFormatSelect(
    [{ value: '', label: 'Keep original format', title: 'Writes the same container as the input.' }, ...IMAGE_FORMATS],
  );

  const cropEnable = toggle('Crop');
  const cropX = numberInput(0, 0, 20000);
  const cropY = numberInput(0, 0, 20000);
  const cropW = numberInput(800, 1, 20000);
  const cropH = numberInput(600, 1, 20000);

  const resizeEnable = toggle('Resize');
  const resizeW = numberInput(1280, 1, 20000);
  const resizeH = numberInput(720, 1, 20000);
  const resizeKeep = toggle('Keep aspect ratio', true);

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

  const grayscale = toggle('Grayscale');
  const sharpen = toggle('Sharpen');
  const blurEnable = toggle('Blur');
  const blurSigma = numberInput(2, 0, 50, 0.5);
  const brightnessEnable = toggle('Brightness');
  const brightnessValue = numberInput(0, -100, 100);
  const contrastEnable = toggle('Contrast');
  const contrastValue = numberInput(1, 0, 4, 0.05);

  const textEnable = toggle('Add text');
  const textContent = textInput('Hello PixConvert');
  const textX = numberInput(24, 0, 20000);
  const textY = numberInput(24, 0, 20000);
  const textSize = numberInput(48, 4, 500);
  const textColor = textInput('#ffffff', '#ffffff');
  const textFont = textInput('/System/Library/Fonts/Supplemental/Arial.ttf', '');

  const applyButton = button('Apply edits', () => void runEdit());
  const editStatus = statusLine();
  const editCompare = createComparisonSlider();

  /* ---------------------------------------------------- background cut --- */
  const bgButton = button('Remove background', () => void runRemoveBackground(), 'secondary');
  const bgStatus = statusLine();
  const bgField = h('fieldset', { class: 'fieldset', id: 'bgRemovalField', hidden: true }, [
    h('legend', {}, ['Background removal']),
    h('p', { class: 'hint' }, [
      'Cuts the subject out on-device with Apple Vision (macOS 14+) and writes ',
      '<stem>_nobg_by_alris.png with a transparent background. No upload, no model download.',
    ]),
    bgButton,
    bgStatus,
  ]);

  void backgroundRemovalAvailable()
    .then((available) => {
      bgField.hidden = !available;
    })
    .catch(() => {
      bgField.hidden = true;
    });

  async function runRemoveBackground(): Promise<void> {
    if (!runningInTauri()) {
      setStatus(bgStatus, 'The backend only runs inside Tauri — use `npm run tauri dev`.', 'error');
      return;
    }
    const paths = pathsWithExtensions(IMAGE_EXTENSIONS);
    if (paths.length === 0) {
      setStatus(bgStatus, 'Select an image first.', 'error');
      return;
    }

    const path = paths[0];
    setStatus(bgStatus, `Cutting out ${baseName(path)}…`);
    try {
      const message = await removeBackground(path, 'png');
      setStatus(bgStatus, message, 'success');
      editCompare.setImages(fileUrl(path), fileUrl(outputPathFrom(message, path, 'png', 'nobg')));
    } catch (error) {
      setStatus(bgStatus, errorMessage(error), 'error');
    }
  }

  const editPanel = panel([
    editFormat.element,
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
      resizeEnable.element,
      h('div', { class: 'grid grid--2' }, [field('Width', resizeW), field('Height', resizeH)]),
      resizeKeep.element,
    ]),
    h('div', { class: 'grid grid--2' }, [
      field('Rotate', rotateSelect),
      h('div', { class: 'stack' }, [flipH.element, flipV.element]),
    ]),
    h('fieldset', { class: 'fieldset' }, [
      h('div', { class: 'grid grid--4' }, [
        grayscale.element,
        sharpen.element,
        blurEnable.element,
        field('Blur σ', blurSigma),
      ]),
      h('div', { class: 'grid grid--2' }, [
        brightnessEnable.element,
        field('Brightness (-100…100)', brightnessValue),
      ]),
      h('div', { class: 'grid grid--2' }, [
        contrastEnable.element,
        field('Contrast (1 = neutral)', contrastValue),
      ]),
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
      field('Font file (.ttf)', textFont, 'Leave empty to use a system font'),
    ]),
    bgField,
    applyButton,
    editStatus,
    editCompare.element,
  ]);

  async function runEdit(): Promise<void> {
    if (!runningInTauri()) {
      setStatus(editStatus, 'The backend only runs inside Tauri — use `npm run tauri dev`.', 'error');
      return;
    }
    const paths = pathsWithExtensions(IMAGE_EXTENSIONS);
    if (paths.length === 0) {
      setStatus(editStatus, 'Select an image first.', 'error');
      return;
    }

    const ops: ImageEditOps = {};
    if (cropEnable.input.checked) {
      ops.crop = {
        x: Number(cropX.value),
        y: Number(cropY.value),
        width: Number(cropW.value),
        height: Number(cropH.value),
      };
    }
    if (resizeEnable.input.checked) {
      ops.resize = {
        width: Number(resizeW.value),
        height: Number(resizeH.value),
        keepAspect: resizeKeep.input.checked,
      };
    }
    if (rotateSelect.value !== '0') ops.rotate = Number(rotateSelect.value);
    if (flipH.input.checked) ops.flipH = true;
    if (flipV.input.checked) ops.flipV = true;
    if (grayscale.input.checked) ops.grayscale = true;
    if (sharpen.input.checked) ops.sharpen = true;
    if (blurEnable.input.checked) ops.blur = Number(blurSigma.value);
    if (brightnessEnable.input.checked) ops.brightness = Number(brightnessValue.value);
    if (contrastEnable.input.checked) ops.contrast = Number(contrastValue.value);
    if (textEnable.input.checked) {
      ops.text = {
        content: textContent.value,
        x: Number(textX.value),
        y: Number(textY.value),
        size: Number(textSize.value),
        color: textColor.value || '#ffffff',
        fontPath: textFont.value.trim() || undefined,
      };
    }

    const format = editFormat.getValue();
    try {
      const path = paths[0];
      const message = await editImage(path, ops, format || undefined);
      setStatus(editStatus, message, 'success');
      editCompare.setImages(
        fileUrl(path),
        fileUrl(outputPathFrom(message, path, format || extensionOf(path), 'edited')),
      );
    } catch (error) {
      setStatus(editStatus, errorMessage(error), 'error');
    }
  }

  /* --------------------------------------------------------------- tabs --- */
  const subtabs = createSubTabs(
    [
      { id: 'convert', label: 'Convert' },
      { id: 'compress', label: 'Compress' },
      { id: 'edit', label: 'Edit' },
    ],
    { convert: convertPanel, compress: compressPanel, edit: editPanel },
    getState().activeSubTab,
  );

  root.append(
    h('header', { class: 'studio__header' }, [
      h('h2', { class: 'studio__title' }, ['Image Studio']),
      h('p', { class: 'studio__subtitle' }, [
        'Everything here is processed locally in Rust — no ffmpeg needed.',
      ]),
    ]),
    subtabs.element,
    progress.element,
    convertPanel,
    compressPanel,
    editPanel,
  );

  return {
    id: 'image',
    title: 'Image',
    element: root,
    refresh: () => {
      const count = pathsWithExtensions(IMAGE_EXTENSIONS).length;
      const label = count === 0 ? 'No images selected' : `${count} image(s) selected`;
      convertStatus.textContent = label;
      compressStatus.textContent = label;
      editStatus.textContent = label;
    },
  };
}

/**
 * The backend replies with `"… to /path/file.ext (123 bytes)"`; pull the path
 * back out so the comparison slider can load it.
 */
function outputPathFrom(
  message: string,
  inputPath: string,
  format: string,
  op: 'converted' | 'edited' | 'compressed' | 'nobg',
): string {
  // Backend replies are always `"<verb>: <path> (<details>)"`.
  const match = message.match(/: (.+?)\s*\(/);
  if (match && (match[1].includes('/') || match[1].includes('\\'))) return match[1].trim();

  const slash = Math.max(inputPath.lastIndexOf('/'), inputPath.lastIndexOf('\\'));
  const dir = slash === -1 ? '' : inputPath.slice(0, slash + 1);
  const name = baseName(inputPath);
  const dot = name.lastIndexOf('.');
  const stem = dot === -1 ? name : name.slice(0, dot);
  const ext = format || name.slice(dot + 1) || 'png';
  return `${dir}${stem}_${op}_by_alris.${ext}`;
}
