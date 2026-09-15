# PixConvert
An image conversion app built with Rust

---

This app is meant for desktop only.

## Use
This app is used to convert image formats into:
- PNG
- JPEG
- GIF
- WebP

completely locally, no cloud.

It uses the Rust package `image` to do so, while earlier versions used `ffmpeg`, but that would make ffmpeg a dependancy.

## Install 
Grab a release when I add one.

## Build from source
Ensure tauri, npm, typescript, git and rust are installed, then run:

```bash
git clone https://github.com/MrCheesie/PixConvert.git
cd PixConvert
```
and to start it, run
```bash
npm run tauri dev
```

## License

It's [MIT](LICENSE). Do whatever you want.
