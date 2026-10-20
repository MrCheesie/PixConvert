/** Router: tab switching, shared drop zone/file list, studio mounting. */

import { createDropZone } from './components/DropZone';
import { createFileList } from './components/FileList';
import {
  AUDIO_EXTENSIONS,
  DOC_EXTENSIONS,
  IMAGE_EXTENSIONS,
  VIDEO_EXTENSIONS,
} from './lib/formats';
import type { Studio, StudioId } from './lib/types';
import { createAudioStudio } from './studios/audio/AudioStudio';
import { createDocStudio } from './studios/doc/DocStudio';
import { createImageStudio } from './studios/image/ImageStudio';
import { createVideoStudio } from './studios/video/VideoStudio';
import { addPaths, getState, setActiveStudio, subscribe } from './state';

interface StudioMeta {
  label: string;
  extensions: string[];
}

const STUDIO_META: Record<StudioId, StudioMeta> = {
  image: { label: 'Images', extensions: IMAGE_EXTENSIONS },
  video: { label: 'Videos', extensions: VIDEO_EXTENSIONS },
  audio: { label: 'Audio', extensions: AUDIO_EXTENSIONS },
  doc: { label: 'Documents', extensions: DOC_EXTENSIONS },
};

export function initRouter(): void {
  const container = document.querySelector<HTMLElement>('#studioContainer');
  const tabBar = document.querySelector<HTMLElement>('#tabBar');
  const dropZoneHost = document.querySelector<HTMLElement>('#dropZoneHost');
  const fileListHost = document.querySelector<HTMLElement>('#fileListHost');

  if (!container || !tabBar || !dropZoneHost || !fileListHost) {
    console.error('PixConvert shell is missing required mount points.');
    return;
  }

  const studios: Record<StudioId, Studio> = {
    image: createImageStudio(),
    video: createVideoStudio(),
    audio: createAudioStudio(),
    doc: createDocStudio(),
  };

  for (const studio of Object.values(studios)) {
    container.append(studio.element);
  }

  const fileList = createFileList();
  fileListHost.append(fileList.element);

  const dropZone = createDropZone({
    getExtensions: () => STUDIO_META[getState().activeStudio].extensions,
    getLabel: () => STUDIO_META[getState().activeStudio].label,
    onPaths: (paths) => addPaths(paths),
  });
  dropZoneHost.append(dropZone);

  const tabs = Array.from(tabBar.querySelectorAll<HTMLButtonElement>('[data-studio]'));
  for (const tab of tabs) {
    tab.addEventListener('click', () => {
      const id = tab.dataset.studio as StudioId | undefined;
      if (id && id in studios) setActiveStudio(id);
    });
  }

  const render = (): void => {
    const { activeStudio } = getState();

    for (const tab of tabs) {
      const isActive = tab.dataset.studio === activeStudio;
      tab.classList.toggle('is-active', isActive);
      tab.setAttribute('aria-selected', String(isActive));
    }
    for (const [id, studio] of Object.entries(studios)) {
      studio.element.hidden = id !== activeStudio;
    }

    document.body.dataset.studio = activeStudio;
    fileList.refresh();
    studios[activeStudio].refresh();
  };

  subscribe(render);
  render();
}
