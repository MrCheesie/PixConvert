/** Shared types for the PixConvert frontend. */

export type StudioId = 'image' | 'video' | 'audio' | 'doc';

export type SubTabId = 'convert' | 'compress' | 'edit';

/** A studio is a self-contained view with its own controls. */
export interface Studio {
  id: StudioId;
  title: string;
  element: HTMLElement;
  /** Called when the shared file selection changes. */
  refresh: () => void;
}

export interface FormatOption {
  /** Extension handed to the backend, e.g. `jpeg`. */
  value: string;
  label: string;
  /** Tooltip explaining when to pick this format. */
  title: string;
}

export interface BatchResult {
  input: string;
  output: string | null;
  error: string | null;
}

export interface ToolStatus {
  ffmpeg: string | null;
  pandoc: string | null;
}

/** Result of `probe_media` (ffprobe), used by the video/audio timelines. */
export interface MediaInfo {
  path: string;
  durationSecs: number;
  hasVideo: boolean;
  hasAudio: boolean;
  width: number | null;
  height: number | null;
  fps: number | null;
  sampleRate: number | null;
  channels: number | null;
}

export interface ProgressEvent {
  done: number;
  total: number;
  current: string;
}

/** Live encode percentage for one file (`pixconvert://job-progress`). */
export interface JobProgressEvent {
  path: string;
  percent: number;
}

export interface ToolDownloadEvent {
  name: string;
  done: number;
  total: number;
}

/* ------------------------------------------------------------- edit ops ---- */

export interface CropOp {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface ResizeOp {
  width: number;
  height: number;
  keepAspect: boolean;
}

export interface ImageTextOp {
  content: string;
  x: number;
  y: number;
  size: number;
  color: string;
  fontPath?: string;
}

export interface ImageEditOps {
  crop?: CropOp;
  rotate?: number;
  flipH?: boolean;
  flipV?: boolean;
  resize?: ResizeOp;
  grayscale?: boolean;
  blur?: number;
  brightness?: number;
  contrast?: number;
  sharpen?: boolean;
  text?: ImageTextOp;
}

export interface TrimOp {
  start: number;
  end?: number;
}

export interface VideoTextOp {
  content: string;
  x?: number;
  y?: number;
  size?: number;
  color?: string;
}

export interface VideoEditOps {
  trim?: TrimOp;
  crop?: CropOp;
  rotate?: number;
  flipH?: boolean;
  flipV?: boolean;
  scale?: { width: number; height: number };
  speed?: number;
  mute?: boolean;
  text?: VideoTextOp;
  audioPath?: string;
}

export interface AudioEditOps {
  trim?: TrimOp;
  speed?: number;
  volumeDb?: number;
  fadeIn?: number;
  fadeOut?: number;
  reverse?: boolean;
  mixPath?: string;
}
