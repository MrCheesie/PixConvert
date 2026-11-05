/** Phase 4 studio — documents through pandoc, PDF size through ghostscript. */

import { createFormatSelect } from '../../components/FormatSelect';
import { runWithLoading } from '../../components/LoadingOverlay';
import { createProgressBar } from '../../components/ProgressBar';
import { createSubTabs } from '../../components/SubTabs';
import { createToolStatus } from '../../components/ToolStatus';
import {
  button,
  h,
  panel,
  setStatus,
  statusLine,
  textInput,
  toggle,
} from '../../lib/dom';
import { DOC_EXTENSIONS, DOC_FORMATS } from '../../lib/formats';
import {
  compressDoc,
  convertDoc,
  docToPdf,
  errorMessage,
  onProgress,
  readTextFile,
  runningInTauri,
  writeTextFile,
} from '../../lib/tauri';
import type { Studio } from '../../lib/types';
import { baseName, pathsWithExtensions } from '../../state';

const EDITABLE = ['md', 'markdown', 'txt', 'html', 'htm'];

export function createDocStudio(): Studio {
  const root = h('section', { class: 'studio', 'data-studio': 'doc' });
  const progress = createProgressBar();
  void onProgress((event) => progress.set(event.done, event.total, baseName(event.current)));
  const tool = createToolStatus('pandoc', 'pandoc');

  /* ------------------------------------------------------------ convert --- */
  const convertFormat = createFormatSelect(DOC_FORMATS);
  const convertButton = button('Convert', () => void runConvert());
  const pdfButton = button('Convert to PDF', () => void runPdf(), 'secondary');
  const convertStatus = statusLine();

  const convertPanel = panel([
    convertFormat.element,
    h('div', { class: 'row' }, [convertButton, pdfButton]),
    h('p', { class: 'hint' }, [
      'PDF output needs pdflatex (TeX), typst, wkhtmltopdf or weasyprint on your PATH.',
    ]),
    convertStatus,
  ]);

  async function runConvert(): Promise<void> {
    const path = requireDoc(convertStatus);
    if (!path) return;
    try {
      const message = await runWithLoading(`Converting ${baseName(path)}`, () =>
        convertDoc(path, convertFormat.getValue()),
      );
      setStatus(convertStatus, message, 'success');
    } catch (error) {
      setStatus(convertStatus, errorMessage(error), 'error');
    }
  }

  async function runPdf(): Promise<void> {
    const path = requireDoc(convertStatus);
    if (!path) return;
    try {
      const message = await runWithLoading(`Building a PDF from ${baseName(path)}`, () => docToPdf(path));
      setStatus(convertStatus, message, 'success');
    } catch (error) {
      setStatus(convertStatus, errorMessage(error), 'error');
    }
  }

  /* ----------------------------------------------------------- compress --- */
  const compressButton = button('Shrink PDF', () => void runCompress());
  const compressStatus = statusLine();

  const compressPanel = panel([
    h('p', { class: 'hint' }, [
      'PDF compression uses ghostscript (`gs`), which is not bundled. Everything else stays untouched.',
    ]),
    compressButton,
    compressStatus,
  ]);

  async function runCompress(): Promise<void> {
    const path = requireDoc(compressStatus);
    if (!path) return;
    try {
      const message = await runWithLoading(`Shrinking ${baseName(path)}`, () => compressDoc(path));
      setStatus(compressStatus, message, 'success');
    } catch (error) {
      setStatus(compressStatus, errorMessage(error), 'error');
    }
  }

  /* --------------------------------------------------------------- edit --- */
  const editor = h('textarea', { class: 'editor', spellcheck: 'false' });
  const preview = h('div', { class: 'preview' });
  const previewToggle = toggle('Live preview', true);
  const saveButton = button('Save', () => void saveCurrent());
  const saveAsField = textInput('/path/to/copy.md', '');
  const saveAsButton = button('Save a copy', () => void saveCopy(), 'secondary');
  const loadButton = button('Reload', () => void loadFile(), 'ghost');
  const editStatus = statusLine();

  const editPanel = panel([
    h('div', { class: 'row' }, [
      h('span', { class: 'hint' }, ['Editing markdown, HTML or plain text only. Convert other formats first.']),
      loadButton,
    ]),
    h('div', { class: 'editor-grid' }, [
      h('div', { class: 'editor-grid__pane' }, [editor]),
      h('div', { class: 'editor-grid__pane' }, [preview]),
    ]),
    previewToggle.element,
    h('div', { class: 'row' }, [saveButton]),
    h('div', { class: 'row' }, [saveAsField, saveAsButton]),
    editStatus,
  ]);

  let editingPath = '';

  async function loadFile(): Promise<void> {
    if (!runningInTauri()) {
      setStatus(editStatus, 'The editor needs the Tauri backend.', 'error');
      return;
    }
    const paths = pathsWithExtensions(EDITABLE);
    if (paths.length === 0) {
      editingPath = '';
      editor.value = '';
      preview.innerHTML = '';
      setStatus(editStatus, 'Select a .md, .txt or .html file to edit.', 'info');
      return;
    }
    editingPath = paths[0];
    try {
      editor.value = await readTextFile(editingPath);
      renderPreview();
      saveAsField.value = defaultCopyPath(editingPath);
      setStatus(editStatus, `Editing ${baseName(editingPath)}`, 'info');
    } catch (error) {
      setStatus(editStatus, errorMessage(error), 'error');
    }
  }

  function renderPreview(): void {
    if (!previewToggle.input.checked) {
      preview.innerHTML = '';
      return;
    }
    const isMarkdown = /\.(md|markdown)$/i.test(editingPath);
    preview.innerHTML = isMarkdown ? renderMarkdown(editor.value) : renderPlain(editor.value);
  }

  editor.addEventListener('input', () => renderPreview());
  previewToggle.input.addEventListener('change', () => renderPreview());

  async function saveCurrent(): Promise<void> {
    if (!editingPath) {
      setStatus(editStatus, 'Nothing loaded to save.', 'error');
      return;
    }
    try {
      const message = await writeTextFile(editingPath, editor.value);
      setStatus(editStatus, message, 'success');
    } catch (error) {
      setStatus(editStatus, errorMessage(error), 'error');
    }
  }

  async function saveCopy(): Promise<void> {
    if (!editingPath) {
      setStatus(editStatus, 'Nothing loaded to save.', 'error');
      return;
    }
    const target = saveAsField.value.trim();
    if (!target) {
      setStatus(editStatus, 'Enter a path for the copy.', 'error');
      return;
    }
    try {
      const message = await writeTextFile(target, editor.value);
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
      h('h2', { class: 'studio__title' }, ['Doc Studio']),
      h('p', { class: 'studio__subtitle' }, ['pandoc handles conversion; markdown and HTML are editable here.']),
    ]),
    tool.element,
    subtabs.element,
    progress.element,
    convertPanel,
    compressPanel,
    editPanel,
  );

  let lastLoaded = '';
  return {
    id: 'doc',
    title: 'Doc',
    element: root,
    refresh: () => {
      const count = pathsWithExtensions(DOC_EXTENSIONS).length;
      const label = count === 0 ? 'No documents selected' : `${count} document(s) selected`;
      convertStatus.textContent = label;
      compressStatus.textContent = label;

      const first = pathsWithExtensions(EDITABLE)[0] ?? '';
      if (first !== lastLoaded) {
        lastLoaded = first;
        void loadFile();
      }
    },
  };
}

function requireDoc(status: HTMLElement): string | null {
  if (!runningInTauri()) {
    setStatus(status, 'The backend only runs inside Tauri — use `npm run tauri dev`.', 'error');
    return null;
  }
  const paths = pathsWithExtensions(DOC_EXTENSIONS);
  if (paths.length === 0) {
    setStatus(status, 'Select a document first.', 'error');
    return null;
  }
  return paths[0];
}

function defaultCopyPath(path: string): string {
  const dot = path.lastIndexOf('.');
  return dot === -1 ? `${path}_copy` : `${path.slice(0, dot)}_copy${path.slice(dot)}`;
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

/**
 * Deliberately tiny markdown renderer (headings, lists, code, emphasis, links).
 * Full CommonMark is out of scope for v2.0 — the source is escaped first, so the
 * preview can never execute script from a file.
 */
function renderMarkdown(source: string): string {
  const escaped = escapeHtml(source);
  const lines = escaped.split(/\r?\n/);
  const html: string[] = [];
  let inCode = false;
  let inList = false;

  for (const line of lines) {
    if (line.trimStart().startsWith('```')) {
      if (inList) {
        html.push('</ul>');
        inList = false;
      }
      html.push(inCode ? '</code></pre>' : '<pre><code>');
      inCode = !inCode;
      continue;
    }
    if (inCode) {
      html.push(`${line}\n`);
      continue;
    }

    const listItem = line.match(/^\s*[-*+]\s+(.*)$/);
    if (listItem) {
      if (!inList) {
        html.push('<ul>');
        inList = true;
      }
      html.push(`<li>${inline(listItem[1])}</li>`);
      continue;
    }
    if (inList) {
      html.push('</ul>');
      inList = false;
    }

    const heading = line.match(/^(#{1,6})\s+(.*)$/);
    if (heading) {
      const level = heading[1].length;
      html.push(`<h${level}>${inline(heading[2])}</h${level}>`);
    } else if (line.trim() === '') {
      html.push('');
    } else {
      html.push(`<p>${inline(line)}</p>`);
    }
  }
  if (inList) html.push('</ul>');
  if (inCode) html.push('</code></pre>');
  return html.join('\n');
}

function inline(text: string): string {
  return text
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
    .replace(/\*([^*]+)\*/g, '<em>$1</em>')
    .replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" target="_blank" rel="noreferrer">$1</a>');
}

function renderPlain(source: string): string {
  // Markup (HTML) is shown as text; never inject the file's HTML into the app.
  return `<pre>${escapeHtml(source)}</pre>`;
}
