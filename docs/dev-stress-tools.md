# Dev Stress Tools

These tools are development-only Tauri commands. They are not wired into the user UI.

## Use a Test Database

Set this environment variable before starting the app:

```powershell
$env:ILLUTAG_USE_TEST_DB = "1"
npm run tauri dev
```

The app will use `illutag.test.sqlite` in the normal app data directory instead of `illutag.sqlite`.

## Commands

In dev mode, the app installs `window.illuTagDevStress` in the WebView console.

### Create Fake Gallery Rows

```js
await window.illuTagDevStress.createFakeGalleryData({
  count: 100000,
  sourceFolder: 'D:\\Pictures\\Samples',
  randomizeDimensions: true,
  randomizeFileNames: true,
  randomizeImportedAt: true,
  randomizeFolders: true,
  randomizeFavorites: true,
  randomizeTags: true
})
```

Fake image ids use the `illutag-dev-fake:` prefix. Paths cycle over a small number of real sample images with a fake suffix, so the database can be large without duplicating image files.

### Create Small File Test Set

```js
await window.illuTagDevStress.createSmallFileTestSet({
  count: 10000,
  format: 'png',
  subfolderCount: 20
})
```

If `rootDir` is omitted, files are written under `.illutag-dev-test/small-files` beside the active database.

### Cleanup

```js
await window.illuTagDevStress.cleanupStressTestData()
```

Cleanup only removes records/files marked with the dev-test prefix or `.illutag-dev-test` path marker.
