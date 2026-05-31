# illuTag

illuTag is a local-first image manager and reference-board tool for anime-style creators.

It is built with Tauri, Vue 3, Rust, SQLite, and local ONNX inference. The app focuses on fast local browsing, manual organization, reference-board workflows, Danbooru-style tagging, and semantic image search without uploading user images to a remote service.

## Features

- Local image library indexing
- Virtualized masonry gallery
- User folders and folder rules
- Reference boards with drag, paste, copy, export, transform, and layout tools
- Recycle-bin workflow for indexed images
- Favorite images and batch operations
- Danbooru tag search and Chinese tag suggestions
- WD tagger based automatic tagging
- Chinese-CLIP based text-to-image and image-to-image search
- Color and atmosphere similarity search
- Portable release packaging with embedded Python runtime support

## Current Status

This project is still in active development. Core gallery, folder, reference-board, tagging, and search workflows are usable, but the data schema and packaging layout may still change before a stable public release.

## Repository Contents

The source repository intentionally does not include large runtime assets.

Included:

- Tauri / Vue / Rust source code
- Python service scripts used by local ONNX inference
- `wd-swinv2-tagger-v3/selected_tags.csv`
- `wd-swinv2-tagger-v3/selected_tags_full_translation.csv`
- Portable packaging script

Not included:

- ONNX model weights
- Chinese-CLIP model files
- Embedded Python runtime
- Portable release zip files
- Local app database and cache files

## Development

Install dependencies:

```powershell
npm install
```

Run in development mode:

```powershell
npm run tauri dev
```

Build a release executable:

```powershell
npm run tauri build
```

The release executable is generated under:

```text
src-tauri/target/release/
```

## Local Data

By default, illuTag stores its app data in Tauri's app data directory.

On Windows this is usually:

```text
%AppData%\com.sunriseworks.illutag
```

The SQLite library index is stored there as `illutag.sqlite`.

## AI Runtime And Models

AI features require local model/runtime files that are not included in this repository.

Expected portable layout:

```text
illuTag/
  illutag.exe
  runtime/
    python/
      python.exe
      Lib/
      ...
  scripts/
    wd_tagger_service.py
    chinese_clip_text_service.py
    chinese_clip_image_service.py
  wd-swinv2-tagger-v3/
    model.onnx
    selected_tags.csv
    selected_tags_full_translation.csv
  model/
    chinese-clip-vit-base-patch16/
      vocab.txt
      onnx/
        chinese_clip_text_encoder.onnx
        chinese_clip_image_encoder.onnx
```

The app will prefer `runtime/python/python.exe` next to the executable. If no embedded Python runtime is found, it falls back to `python` from `PATH`.

## Portable Packaging

After preparing a release executable, runtime, scripts, and model files, create a portable package with:

```powershell
.\scripts\package-portable.ps1 -Version "0.1.0"
```

Or build first, then package:

```powershell
.\scripts\package-portable.ps1 -Build -Version "0.1.0"
```

The output is written to:

```text
release/portable/
```

## Third-Party Models And Acknowledgements

illuTag can use the following open-source models:

- [OFA-Sys/Chinese-CLIP](https://github.com/OFA-Sys/Chinese-CLIP), licensed under the MIT License.
- [SmilingWolf/wd-swinv2-tagger-v3](https://huggingface.co/SmilingWolf/wd-swinv2-tagger-v3), licensed under Apache-2.0.

These projects are not part of illuTag. Please follow their original licenses and model cards when downloading, redistributing, or using their model files.

The included Danbooru-style tag dictionary files are used for local tag lookup and Chinese tag suggestions. They are distributed for convenience as project data; third-party source materials retain their respective rights and licenses.

## License

illuTag source code is released under the MIT License. See [LICENSE](LICENSE).

Third-party models, runtimes, libraries, and datasets keep their own licenses.
