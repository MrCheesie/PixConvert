/** Format matrix — the single source of truth for every FormatSelect. */

import type { FormatOption } from './types';

const format = (value: string, label: string, title: string): FormatOption => ({
  value,
  label,
  title,
});

export const IMAGE_FORMATS: FormatOption[] = [
  format('png', 'PNG', 'Lossless, transparency, crisp graphics.'),
  format('jpeg', 'JPEG', 'Best for photos; lossy, adjustable quality.'),
  format('webp', 'WebP', 'Small web images (written losslessly by v2.0).'),
  format('avif', 'AVIF', 'Modern, very small; great for photos. Slower to encode.'),
  format('gif', 'GIF', 'Single frame; use video studio for animation.'),
  format('bmp', 'BMP', 'Uncompressed bitmap; large but universal.'),
  format('tiff', 'TIFF', 'Lossless archival / print workflows.'),
  format('ico', 'ICO', 'Windows icons (max 256 x 256).'),
  format('icns', 'ICNS', 'macOS icons (16-512 px, written as a multi-size family).'),
  format('qoi', 'QOI', 'Fast lossless format for tooling.'),
  format('heic', 'HEIC', 'Apple photos. Requires ffmpeg.'),
  format('jxl', 'JPEG XL', 'Modern lossless/lossy format. Requires ffmpeg.'),
];

/** Pseudo format: raster → SVG through vtracer. */
export const VECTOR_FORMAT: FormatOption = format(
  'svg',
  'SVG (vectorize)',
  'Trace the image into paths. Needs a build with the `vectorize` feature.',
);

export const VIDEO_FORMATS: FormatOption[] = [
  format('mp4', 'MP4', 'H.264 + AAC; plays everywhere.'),
  format('mkv', 'MKV', 'Matroska container, keeps quality.'),
  format('webm', 'WebM', 'VP9 + Opus; best for the web.'),
  format('mov', 'MOV', 'QuickTime container for Apple tools.'),
  format('avi', 'AVI', 'Legacy container, broad player support.'),
  format('wmv', 'WMV', 'Windows Media Video.'),
  format('m4v', 'M4V', 'Apple-flavoured MP4.'),
  format('flv', 'FLV', 'Legacy streaming container.'),
  format('ts', 'TS', 'MPEG transport stream.'),
  format('gif', 'Animated GIF', 'Looping GIF (15 fps, 480 px wide).'),
  format('mp3', 'MP3 (audio only)', 'Extract the soundtrack as MP3.'),
];

export const AUDIO_FORMATS: FormatOption[] = [
  format('mp3', 'MP3', 'The universal lossy format.'),
  format('aac', 'AAC', 'Efficient lossy audio (raw ADTS).'),
  format('m4a', 'M4A', 'AAC in an MP4 container (Apple).'),
  format('wav', 'WAV', 'Uncompressed PCM.'),
  format('flac', 'FLAC', 'Lossless compression.'),
  format('ogg', 'OGG', 'Vorbis in an Ogg container.'),
  format('opus', 'Opus', 'Modern low-latency lossy codec.'),
  format('aiff', 'AIFF', 'Uncompressed Apple audio.'),
  format('wma', 'WMA', 'Windows Media Audio.'),
];

export const DOC_FORMATS: FormatOption[] = [
  format('pdf', 'PDF', 'Portable document; needs a LaTeX engine or typst.'),
  format('docx', 'DOCX', 'Microsoft Word document.'),
  format('odt', 'ODT', 'LibreOffice / OpenDocument text.'),
  format('rtf', 'RTF', 'Rich text that Word and LibreOffice read.'),
  format('md', 'Markdown', 'Plain-text markup, editable in this app.'),
  format('html', 'HTML', 'Standalone web page, editable in this app.'),
  format('txt', 'Plain text', 'No formatting at all.'),
  format('pptx', 'PPTX', 'PowerPoint slides (pandoc layout is basic).'),
  format('epub', 'EPUB', 'E-book package.'),
];

export const IMAGE_EXTENSIONS = [
  'png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp', 'tif', 'tiff', 'avif', 'ico', 'icns', 'qoi', 'heic', 'heif', 'jxl',
];

export const VIDEO_EXTENSIONS = [
  'mp4', 'mov', 'mkv', 'webm', 'avi', 'wmv', 'm4v', 'flv', 'ts', 'mpg', 'mpeg',
];

export const AUDIO_EXTENSIONS = [
  'mp3', 'aac', 'm4a', 'wav', 'flac', 'ogg', 'opus', 'aiff', 'aif', 'wma',
];

export const DOC_EXTENSIONS = [
  'md', 'markdown', 'txt', 'html', 'htm', 'docx', 'odt', 'rtf', 'pptx', 'epub', 'tex', 'rst', 'org', 'ipynb', 'csv', 'pdf',
];
