import { invoke, isTauri } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { getCurrentWebview } from '@tauri-apps/api/webview';

// Browser mode: File objects (no paths available)
let selectedFiles: File[] = [];
// Tauri mode: native file paths (from dialog or drag-drop)
let selectedPaths: string[] = [];

const dropZone = document.getElementById('dropZone');
const fileInput = document.getElementById('fileInput') as HTMLInputElement | null;
const fileList = document.getElementById('fileList');
const formatSelect = document.getElementById('formatSelect') as HTMLSelectElement | null;
const convertBtn = document.getElementById('convertBtn');

function refreshFileList() {
  if (!fileList) return;
  const names =
    selectedPaths.length > 0
      ? selectedPaths.map((p) => p.split(/[\\/]/).pop() ?? p)
      : selectedFiles.map((f) => f.name);
  fileList.innerHTML =
    names.length > 0 ? `<strong>Selected Files:</strong> ${names.join(', ')}` : '';
}

// ---- File picking ----
fileInput?.addEventListener('change', () => {
  if (fileInput.files) {
    selectedFiles = Array.from(fileInput.files);
    refreshFileList();
  }
});

dropZone?.addEventListener('click', async () => {
  if (isTauri()) {
    // Tauri v2: use the native dialog plugin (returns real paths)
    const selection = await open({
      multiple: true,
      filters: [
        { name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp'] },
      ],
    });
    if (Array.isArray(selection)) selectedPaths = selection;
    else if (typeof selection === 'string') selectedPaths = [selection];
    else selectedPaths = [];
    refreshFileList();
  } else {
    // Plain browser (vite dev): fall back to the hidden input
    fileInput?.click();
  }
});

// ---- Drag & drop ----
dropZone?.addEventListener('dragover', (e) => {
  e.preventDefault();
  dropZone.classList.add('drop-zone--over');
});

['dragleave', 'dragend'].forEach((type) => {
  dropZone?.addEventListener(type, () => {
    dropZone.classList.remove('drop-zone--over');
  });
});

dropZone?.addEventListener('drop', (e) => {
  e.preventDefault();
  dropZone.classList.remove('drop-zone--over');
  if (e.dataTransfer?.files.length) {
    // Only reachable in a plain browser; Tauri intercepts the drop event
    selectedFiles = Array.from(e.dataTransfer.files);
    refreshFileList();
  }
});

// Tauri v2 intercepts HTML5 drag&drop, so native paths arrive via webview events
if (isTauri()) {
  getCurrentWebview()
    .onDragDropEvent((event) => {
      if (event.payload.type === 'enter' || event.payload.type === 'over') {
        dropZone?.classList.add('drop-zone--over');
      } else if (event.payload.type === 'leave') {
        dropZone?.classList.remove('drop-zone--over');
      } else if (event.payload.type === 'drop') {
        dropZone?.classList.remove('drop-zone--over');
        selectedPaths = event.payload.paths;
        refreshFileList();
      }
    })
    .catch((err) => console.error('Failed to register drag-drop listener:', err));
}

// ---- Conversion ----
convertBtn?.addEventListener('click', async () => {
  if (!isTauri()) {
    alert('Conversion requires the Tauri backend. Run the app with `npm run tauri dev`.');
    return;
  }

  if (selectedPaths.length === 0) {
    alert('Please select files first');
    return;
  }

  const format = formatSelect?.value ?? 'png';

  for (const path of selectedPaths) {
    try {
      const result = await invoke<string>('convert_image', {
        inputPath: path,
        outputFormat: format,
      });
      console.log('Conversion result:', result);
      alert(result);
    } catch (error) {
      console.error('Conversion failed:', error);
      alert(`Error converting ${path}: ${error}`);
    }
  }
});
