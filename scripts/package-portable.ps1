param(
  [switch]$Build,
  [string]$Version = "0.1.0"
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$tauriRoot = Join-Path $repoRoot "src-tauri"
$scriptSourceDir = Join-Path $tauriRoot "scripts"
$releaseExe = Join-Path $tauriRoot "target\release\illutag.exe"
$portableRoot = Join-Path $repoRoot "release\portable"
$portableAppDir = Join-Path $portableRoot ("illuTag-{0}" -f $Version)
$zipPath = Join-Path $portableRoot ("illuTag-portable-{0}.zip" -f $Version)

if ($Build) {
  Push-Location $repoRoot
  try {
    Write-Host "[portable] running npm run tauri build -- --no-bundle ..."
    npm run tauri build -- --no-bundle
  } finally {
    Pop-Location
  }
}

if (!(Test-Path $releaseExe)) {
  throw "release exe not found: $releaseExe. Run npm run tauri build first, or use -Build."
}

$requiredPaths = @(
  @{ Path = $releaseExe; Label = "release exe" },
  @{ Path = (Join-Path $repoRoot "runtime\python\python.exe"); Label = "embedded python" },
  @{ Path = (Join-Path $scriptSourceDir "wd_tagger_service.py"); Label = "wd tagger service script" },
  @{ Path = (Join-Path $scriptSourceDir "chinese_clip_text_service.py"); Label = "clip text service script" },
  @{ Path = (Join-Path $scriptSourceDir "chinese_clip_image_service.py"); Label = "clip image service script" },
  @{ Path = (Join-Path $repoRoot "wd-swinv2-tagger-v3\model.onnx"); Label = "wd model" },
  @{ Path = (Join-Path $repoRoot "wd-swinv2-tagger-v3\selected_tags.csv"); Label = "wd tags csv" },
  @{ Path = (Join-Path $repoRoot "wd-swinv2-tagger-v3\selected_tags_full_translation.csv"); Label = "wd translation csv" },
  @{ Path = (Join-Path $repoRoot "model\chinese-clip-vit-base-patch16\onnx\chinese_clip_text_encoder.onnx"); Label = "clip text onnx" },
  @{ Path = (Join-Path $repoRoot "model\chinese-clip-vit-base-patch16\onnx\chinese_clip_image_encoder.onnx"); Label = "clip image onnx" },
  @{ Path = (Join-Path $repoRoot "model\chinese-clip-vit-base-patch16\vocab.txt"); Label = "clip vocab" }
)

foreach ($item in $requiredPaths) {
  if (!(Test-Path $item.Path)) {
    throw ("missing {0}: {1}" -f $item.Label, $item.Path)
  }
}

Write-Host "[portable] cleaning output: $portableRoot"
if (Test-Path $portableAppDir) {
  Remove-Item -LiteralPath $portableAppDir -Recurse -Force
}
if (Test-Path $zipPath) {
  try {
    Remove-Item -LiteralPath $zipPath -Force
  } catch {
    throw "zip file is locked: $zipPath. Close explorer preview/zip tools and retry."
  }
}
New-Item -ItemType Directory -Path $portableAppDir -Force | Out-Null

Write-Host "[portable] copying release exe"
Copy-Item -LiteralPath $releaseExe -Destination (Join-Path $portableAppDir "illutag.exe") -Force

Write-Host "[portable] copying runtime/python"
Copy-Item -LiteralPath (Join-Path $repoRoot "runtime") -Destination (Join-Path $portableAppDir "runtime") -Recurse -Force

Write-Host "[portable] copying scripts"
Copy-Item -LiteralPath $scriptSourceDir -Destination (Join-Path $portableAppDir "scripts") -Recurse -Force

Write-Host "[portable] copying wd model minimal files"
$wdTargetDir = Join-Path $portableAppDir "wd-swinv2-tagger-v3"
New-Item -ItemType Directory -Path $wdTargetDir | Out-Null
Copy-Item -LiteralPath (Join-Path $repoRoot "wd-swinv2-tagger-v3\model.onnx") -Destination (Join-Path $wdTargetDir "model.onnx") -Force
Copy-Item -LiteralPath (Join-Path $repoRoot "wd-swinv2-tagger-v3\selected_tags.csv") -Destination (Join-Path $wdTargetDir "selected_tags.csv") -Force
Copy-Item -LiteralPath (Join-Path $repoRoot "wd-swinv2-tagger-v3\selected_tags_full_translation.csv") -Destination (Join-Path $wdTargetDir "selected_tags_full_translation.csv") -Force
if (Test-Path (Join-Path $repoRoot "wd-swinv2-tagger-v3\dictionary01.xlsx")) {
  Copy-Item -LiteralPath (Join-Path $repoRoot "wd-swinv2-tagger-v3\dictionary01.xlsx") -Destination (Join-Path $wdTargetDir "dictionary01.xlsx") -Force
}

Write-Host "[portable] copying clip model minimal files"
$clipBase = Join-Path $portableAppDir "model\chinese-clip-vit-base-patch16"
$clipOnnx = Join-Path $clipBase "onnx"
New-Item -ItemType Directory -Path $clipOnnx -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $repoRoot "model\chinese-clip-vit-base-patch16\onnx\chinese_clip_text_encoder.onnx") -Destination (Join-Path $clipOnnx "chinese_clip_text_encoder.onnx") -Force
Copy-Item -LiteralPath (Join-Path $repoRoot "model\chinese-clip-vit-base-patch16\onnx\chinese_clip_image_encoder.onnx") -Destination (Join-Path $clipOnnx "chinese_clip_image_encoder.onnx") -Force
Copy-Item -LiteralPath (Join-Path $repoRoot "model\chinese-clip-vit-base-patch16\vocab.txt") -Destination (Join-Path $clipBase "vocab.txt") -Force
if (Test-Path (Join-Path $repoRoot "model\chinese-clip-vit-base-patch16\preprocessor_config.json")) {
  Copy-Item -LiteralPath (Join-Path $repoRoot "model\chinese-clip-vit-base-patch16\preprocessor_config.json") -Destination (Join-Path $clipBase "preprocessor_config.json") -Force
}

Write-Host "[portable] writing README-portable.txt"
$readme = @"
illuTag Portable Package
=======================

Contents:
- illutag.exe
- runtime/python (embedded python + dependencies)
- scripts (python services)
- wd-swinv2-tagger-v3 (minimal runtime files)
- model/chinese-clip-vit-base-patch16 (onnx + vocab + preprocessor_config)

Usage:
1) Keep all folders next to illutag.exe.
2) Launch illutag.exe directly.
3) First run will still store app data under:
   %AppData%\com.sunriseworks.illutag
"@
Set-Content -Path (Join-Path $portableAppDir "README-portable.txt") -Value $readme -Encoding UTF8

Write-Host "[portable] creating zip: $zipPath"
Compress-Archive -Path $portableAppDir -DestinationPath $zipPath -CompressionLevel Optimal

$dirSize = (Get-ChildItem -Recurse -File $portableAppDir | Measure-Object Length -Sum).Sum
$zipSize = (Get-Item $zipPath).Length
Write-Host ("[portable] done. app_dir_size_mb={0} zip_size_mb={1}" -f `
  [math]::Round($dirSize / 1MB, 2), `
  [math]::Round($zipSize / 1MB, 2))
Write-Host ("[portable] output dir: {0}" -f $portableAppDir)
Write-Host ("[portable] output zip: {0}" -f $zipPath)
