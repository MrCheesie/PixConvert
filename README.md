# PixConvert
An image conversion app built with Rust

---

This app is meant for desktop only. It is available for Windows, MacOS and Linux. 

## Use
This app is used to convert image formats into:
- PNG
- JPEG
- GIF
- WebP

completely locally, no cloud whatsoever, and it does this at blazing fast speeds.

It uses the Rust package `image` to do so, while earlier versions used `ffmpeg`, but as that would make ffmpeg a dependancy, the idea was scrapped.

## Install 
Grab a release from the releases page.

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

## Roadmap
- [x] Get the releases to build
- [x] Test release on MacOS
- [ ] Test release on Linux
- [ ] Test release on Windows
- [ ] Add more image formats
- [ ] Add an option for batch conversion
- [ ] Add a progress bar for batch conversion


## License

It's [MIT](LICENSE). Do whatever you want.
