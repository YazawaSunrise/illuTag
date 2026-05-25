# illuTag

Local reference image manager prototype for anime-style creators.

## Run

```powershell
npm install
npm run tauri dev
```

## WD Tagger Assets

The app looks for WD tagger assets in `wd-swinv2-tagger-v3/` at the project root.

- `selected_tags.csv` and `dictionary01.xlsx` are kept in the repository.
- `model.onnx` is intentionally ignored because it is a large local model file. Put your local copy at `wd-swinv2-tagger-v3/model.onnx` before using auto-tagging.

## Current Flow

1. Open the app.
2. Use the left sidebar settings button.
3. Enter a local image folder path, such as `D:\Pictures\Reference`.
4. Click the add-library-folder button.
5. The main page shows images in segmented masonry order.

The app stores the local library index in Tauri's app data directory as `illutag.sqlite`.
