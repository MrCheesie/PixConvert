/** Typed wrappers around the Rust commands and the app event bus. */

import { convertFileSrc, invoke, isTauri } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

import type {
  AudioEditOps,
  BatchResult,
  ImageEditOps,
  JobProgressEvent,
  MediaInfo,
  ProgressEvent,
  ToolDownloadEvent,
  ToolStatus,
  VideoEditOps,
} from './types';

export const runningInTauri = (): boolean => isTauri();

/** `asset://` URL for a local file (only meaningful inside Tauri). */
export function fileUrl(path: string): string {
  if (!runningInTauri()) return '';
  try {
    return convertFileSrc(path);
  } catch {
    return '';
  }
}

/* --------------------------------------------------------------- image ----- */

export const convertImage = (inputPath: string, outputFormat: string, quality?: number) =>
  invoke<string>('convert_image', { inputPath, outputFormat, quality });

export const compressImage = (
  inputPath: string,
  outputFormat: string,
  quality: number,
  maxWidth?: number,
  maxHeight?: number,
) => invoke<string>('compress_image', { inputPath, outputFormat, quality, maxWidth, maxHeight });

export const editImage = (inputPath: string, ops: ImageEditOps, outputFormat?: string) =>
  invoke<string>('edit_image', { inputPath, ops, outputFormat });

export const vectorizeImage = (inputPath: string, outputPath?: string) =>
  invoke<string>('vectorize_image', { inputPath, outputPath });

export const batchConvert = (inputs: string[], outputFormat: string, quality?: number) =>
  invoke<BatchResult[]>('batch_convert', { inputs, outputFormat, quality });

/** Cuts the subject out with Apple Vision; macOS 14+ only. */
export const removeBackground = (inputPath: string, outputFormat?: string) =>
  invoke<string>('remove_background', { inputPath, outputFormat });

export const backgroundRemovalAvailable = () =>
  invoke<boolean>('background_removal_available');

export const batchCompress = (
  inputs: string[],
  outputFormat: string,
  quality: number,
  maxWidth?: number,
  maxHeight?: number,
) => invoke<BatchResult[]>('batch_compress', { inputs, outputFormat, quality, maxWidth, maxHeight });

/* --------------------------------------------------------------- video ----- */

export const convertVideo = (inputPath: string, outputFormat: string, quality?: number) =>
  invoke<string>('convert_video', { inputPath, outputFormat, quality });

export const compressVideo = (
  inputPath: string,
  outputFormat: string,
  crf: number,
  preset: string,
  lowCpu = false,
) => invoke<string>('compress_video', { inputPath, outputFormat, crf, preset, lowCpu });

export const editVideo = (inputPath: string, ops: VideoEditOps, outputFormat?: string) =>
  invoke<string>('edit_video', { inputPath, ops, outputFormat });

export const videoToAudio = (inputPath: string, audioFormat: string, bitrate?: string) =>
  invoke<string>('video_to_audio', { inputPath, audioFormat, bitrate });

export const videoToGif = (inputPath: string, fps: number, width: number) =>
  invoke<string>('video_to_gif', { inputPath, fps, width });

/* --------------------------------------------------------------- audio ----- */

export const convertAudio = (inputPath: string, outputFormat: string, bitrate?: string) =>
  invoke<string>('convert_audio', { inputPath, outputFormat, bitrate });

export const compressAudio = (inputPath: string, outputFormat: string, bitrate: string) =>
  invoke<string>('compress_audio', { inputPath, outputFormat, bitrate });

export const editAudio = (inputPath: string, ops: AudioEditOps, outputFormat?: string) =>
  invoke<string>('edit_audio', { inputPath, ops, outputFormat });

export const audioWaveform = (inputPath: string) => invoke<number[]>('audio_waveform', { inputPath });

/* ----------------------------------------------------------------- doc ----- */

export const convertDoc = (inputPath: string, outputFormat: string) =>
  invoke<string>('convert_doc', { inputPath, outputFormat });

export const docToPdf = (inputPath: string) => invoke<string>('doc_to_pdf', { inputPath });

export const compressDoc = (inputPath: string) => invoke<string>('compress_doc', { inputPath });

export const readTextFile = (path: string) => invoke<string>('read_text_file', { path });

export const writeTextFile = (path: string, contents: string) =>
  invoke<string>('write_text_file', { path, contents });

/* --------------------------------------------------------------- tools ----- */

export const detectTools = () => invoke<ToolStatus>('detect_tools');

/** Duration + stream info for one file (drives the timelines). */
export const probeMedia = (path: string) => invoke<MediaInfo>('probe_media', { path });

export const ensureTool = (name: string) => invoke<string>('ensure_tool', { name });

/* ---------------------------------------------------- open with / files --- */

/** Paths handed over by Finder / Explorer before the UI was listening. */
export const takeOpenFiles = () => invoke<string[]>('take_open_files');

export const onOpenFiles = (handler: (paths: string[]) => void): Promise<UnlistenFn> =>
  listen<string[]>('pixconvert://open-files', (e) => handler(e.payload));

/* -------------------------------------------------------------- events ----- */

export const onProgress = (handler: (event: ProgressEvent) => void): Promise<UnlistenFn> =>
  listen<ProgressEvent>('pixconvert://progress', (e) => handler(e.payload));

/** Live percentage for the file ffmpeg is encoding right now. */
export const onJobProgress = (
  handler: (event: JobProgressEvent) => void,
): Promise<UnlistenFn> => listen<JobProgressEvent>('pixconvert://job-progress', (e) => handler(e.payload));

export const onToolDownload = (
  handler: (event: ToolDownloadEvent) => void,
): Promise<UnlistenFn> =>
  listen<ToolDownloadEvent>('pixconvert://tool-download', (e) => handler(e.payload));

/** Error objects from Rust are plain strings; normalise for display. */
export function errorMessage(error: unknown): string {
  if (typeof error === 'string') return error;
  if (error instanceof Error) return error.message;
  return String(error);
}
