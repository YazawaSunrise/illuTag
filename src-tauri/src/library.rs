use calamine::{Data as ExcelCell, Reader, open_workbook_auto};
use image::ImageReader;
use image::imageops::FilterType;
use image::GenericImageView;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter, types::Value};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    env,
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};
use walkdir::WalkDir;

const WD_TAGGER_MODEL_NAME: &str = "wd-swinv2-tagger-v3";
const CHINESE_CLIP_MODEL_NAME: &str = "chinese-clip-vit-base-patch16";
const THUMBNAIL_LONG_EDGE: u32 = 960;
const THUMBNAIL_WEBP_QUALITY: f32 = 85.0;

pub struct AppState {
    pub database_path: PathBuf,
    pub library: Arc<Mutex<Option<LibraryStore>>>,
    pub background_scan_running: Arc<Mutex<bool>>,
    pub background_scan_pending: Arc<Mutex<bool>>,
    pub background_scan_progress: Arc<Mutex<BackgroundScanProgress>>,
    pub startup_cleanup_running: Arc<Mutex<bool>>,
    pub startup_cleanup_generation: Arc<Mutex<i64>>,
    pub thumbnail_generation_running: Arc<Mutex<bool>>,
    pub thumbnail_generation_pending: Arc<Mutex<bool>>,
    pub thumbnail_generation_pause_requested: Arc<Mutex<bool>>,
    pub thumbnail_generation_stop_requested: Arc<Mutex<bool>>,
    pub thumbnail_generation_progress: Arc<Mutex<ThumbnailGenerationProgress>>,
    pub natural_language_scan_running: Arc<Mutex<bool>>,
    pub natural_language_scan_pending: Arc<Mutex<bool>>,
    pub natural_language_scan_progress: Arc<Mutex<NaturalLanguageScanProgress>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStore {
    pub folders: Vec<LibraryFolder>,
    pub images: Vec<GalleryImage>,
    pub user_folders: Vec<UserFolder>,
    pub image_folders: Vec<ImageFolderAssignment>,
    pub reference_board_folders: Vec<ReferenceBoardFolder>,
    pub reference_boards: Vec<ReferenceBoard>,
    pub reference_board_items: Vec<ReferenceBoardItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFolder {
    pub id: i64,
    pub path: String,
    pub added_at: i64,
    pub last_scanned_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryImage {
    pub id: String,
    pub path: String,
    pub thumbnail_path: Option<String>,
    pub file_name: String,
    pub ext: String,
    pub width: u32,
    pub height: u32,
    pub file_size: i64,
    pub modified_at: i64,
    pub imported_at: i64,
    pub folder_id: i64,
    pub missing: bool,
    pub trashed: bool,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserFolder {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub name: String,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageFolderAssignment {
    pub image_id: String,
    pub folder_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceBoardFolder {
    pub id: i64,
    pub name: String,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceBoard {
    pub id: i64,
    pub folder_id: Option<i64>,
    pub name: String,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceBoardItem {
    pub id: i64,
    pub board_id: i64,
    pub image_id: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub rotation: f32,
    pub z_index: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageBytes {
    pub bytes: Vec<u8>,
    pub mime_type: String,
    pub file_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WdTaggerTagScore {
    pub tag: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WdTaggerTestResult {
    pub image_id: String,
    pub ratings: Vec<WdTaggerTagScore>,
    pub general_tags: Vec<WdTaggerTagScore>,
    pub character_tags: Vec<WdTaggerTagScore>,
    pub general_threshold: f32,
    pub character_threshold: f32,
    pub elapsed_ms: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageAutoTag {
    pub category: String,
    pub tag_en: String,
    pub tag_zh: Option<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageAutoTagSummary {
    pub image_id: String,
    pub character_tags: Vec<ImageAutoTag>,
    pub general_tags: Vec<ImageAutoTag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownAutoTagSuggestion {
    pub tag_en: String,
    pub tag_zh: Option<String>,
    pub image_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GallerySearchFilters {
    pub chinese_tag_ens: Vec<String>,
    pub english_query: String,
    pub file_name_query: String,
    pub confidence_min: f32,
    pub confidence_max: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundScanStatus {
    pub running: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupCleanupStatus {
    pub running: bool,
    pub generation: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundScanProgress {
    pub running: bool,
    pub phase: String,
    pub scanned_folders: i64,
    pub total_folders: i64,
    pub new_images: i64,
    pub updated_images: i64,
    pub skipped_images: i64,
    pub removed_missing_images: i64,
    pub queued_images: i64,
    pub tagged_images: i64,
    pub failed_images: i64,
    pub last_error: Option<String>,
    pub recent_errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThumbnailGenerationProgress {
    pub running: bool,
    pub paused: bool,
    pub phase: String,
    pub total_candidates: i64,
    pub processed_images: i64,
    pub generated_images: i64,
    pub skipped_images: i64,
    pub failed_images: i64,
    pub last_error: Option<String>,
    pub recent_errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NaturalLanguageScanStatus {
    pub running: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NaturalLanguageScanProgress {
    pub running: bool,
    pub phase: String,
    pub total_images: i64,
    pub processed_images: i64,
    pub generated_images: i64,
    pub skipped_images: i64,
    pub failed_images: i64,
    pub last_error: Option<String>,
    pub recent_errors: Vec<String>,
}

impl Default for BackgroundScanProgress {
    fn default() -> Self {
        Self {
            running: false,
            phase: "idle".to_string(),
            scanned_folders: 0,
            total_folders: 0,
            new_images: 0,
            updated_images: 0,
            skipped_images: 0,
            removed_missing_images: 0,
            queued_images: 0,
            tagged_images: 0,
            failed_images: 0,
            last_error: None,
            recent_errors: Vec::new(),
        }
    }
}

impl Default for ThumbnailGenerationProgress {
    fn default() -> Self {
        Self {
            running: false,
            paused: false,
            phase: "idle".to_string(),
            total_candidates: 0,
            processed_images: 0,
            generated_images: 0,
            skipped_images: 0,
            failed_images: 0,
            last_error: None,
            recent_errors: Vec::new(),
        }
    }
}

impl Default for NaturalLanguageScanProgress {
    fn default() -> Self {
        Self {
            running: false,
            phase: "idle".to_string(),
            total_images: 0,
            processed_images: 0,
            generated_images: 0,
            skipped_images: 0,
            failed_images: 0,
            last_error: None,
            recent_errors: Vec::new(),
        }
    }
}

struct ScannedImage {
    path: String,
    file_name: String,
    ext: String,
    width: u32,
    height: u32,
    file_size: i64,
    modified_at: i64,
    imported_at: i64,
}

#[derive(Debug, Clone, Copy)]
struct ExistingImageMeta {
    width: u32,
    height: u32,
    file_size: i64,
    modified_at: i64,
}

struct ScanCollectResult {
    tag_queue_image_ids: Vec<String>,
}

struct ThumbnailCandidate {
    image_id: String,
    image_path: String,
    modified_at: i64,
    file_size: i64,
    current_thumb_path: Option<String>,
    current_source_modified_at: Option<i64>,
    current_source_file_size: Option<i64>,
}

struct NaturalLanguageEmbeddingCandidate {
    image_id: String,
    image_path: String,
    modified_at: i64,
    file_size: i64,
    current_source_modified_at: Option<i64>,
    current_source_file_size: Option<i64>,
}

pub fn list_library_from_state(state: &AppState) -> Result<LibraryStore, String> {
    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;

    if library.is_none() {
        let conn = open_database(&state.database_path)?;
        *library = Some(load_store(&conn)?);
    }

    Ok(library.clone().unwrap_or_default())
}

pub fn add_gallery_folder(folder_path: String, state: &AppState) -> Result<LibraryStore, String> {
    let folder_path = normalize_folder_path(&folder_path)?;
    let scanned_at = now_ms();
    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let mut conn = open_database(&state.database_path)?;

    let tx = conn
        .transaction()
        .map_err(|error| format!("打开图库事务失败：{error}"))?;
    let folder_id = upsert_folder(&tx, &folder_path, scanned_at)?;
    let mut seen_paths = HashSet::new();
    let existing_meta = load_existing_library_image_meta(&tx)?;
    let found = scan_images(Path::new(&folder_path), scanned_at, &mut seen_paths, &existing_meta);

    for image in found {
        upsert_image(&tx, folder_id, &image)?;
    }

    tx.commit()
        .map_err(|error| format!("保存图库索引失败：{error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn remove_gallery_folder(
    folder_path: String,
    state: &AppState,
) -> Result<LibraryStore, String> {
    let folder_path = normalize_existing_or_stored_folder_path(&folder_path);
    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let mut conn = open_database(&state.database_path)?;

    let tx = conn
        .transaction()
        .map_err(|error| format!("打开图库事务失败：{error}"))?;
    let folder_id = tx
        .query_row(
            "SELECT id FROM folders WHERE path = ?1",
            params![folder_path],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| format!("查询图库文件夹失败：{error}"))?;

    if let Some(folder_id) = folder_id {
        tx.execute(
            "DELETE FROM images WHERE folder_id = ?1",
            params![folder_id],
        )
        .map_err(|error| format!("删除图片索引失败：{error}"))?;
        tx.execute("DELETE FROM folders WHERE id = ?1", params![folder_id])
            .map_err(|error| format!("删除图库文件夹失败：{error}"))?;
    }

    tx.commit()
        .map_err(|error| format!("保存图库索引失败：{error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn remove_image_from_index(image_id: String, state: &AppState) -> Result<LibraryStore, String> {
    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let conn = open_database(&state.database_path)?;
    conn.execute("PRAGMA foreign_keys = ON", [])
        .map_err(|error| format!("启用数据库外键失败：{error}"))?;
    conn.execute("UPDATE images SET trashed = 1 WHERE id = ?1 AND source = 'library'", params![image_id])
        .map_err(|error| format!("Failed to move image to trash: {error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn restore_image_from_trash(image_id: String, state: &AppState) -> Result<LibraryStore, String> {
    let mut library = state
        .library
        .lock()
        .map_err(|_| "鍥惧簱鐘舵€佽鍗犵敤锛岃绋嶅悗鍐嶈瘯".to_string())?;
    let conn = open_database(&state.database_path)?;
    conn.execute("PRAGMA foreign_keys = ON", [])
        .map_err(|error| format!("鍚敤鏁版嵁搴撳閿け璐ワ細{error}"))?;
    conn.execute(
        "UPDATE images SET trashed = 0 WHERE id = ?1 AND source = 'library'",
        params![image_id],
    )
    .map_err(|error| format!("Failed to restore image from trash: {error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn read_image_bytes(image_id: String, state: &AppState) -> Result<ImageBytes, String> {
    let conn = open_database(&state.database_path)?;
    let image = load_image_record(&conn, &image_id)?;
    let bytes = fs::read(&image.path).map_err(|error| format!("读取图片文件失败：{error}"))?;
    Ok(ImageBytes {
        bytes,
        mime_type: mime_type_for_extension(&image.ext).to_string(),
        file_name: image.file_name,
    })
}

pub fn copy_image_to_system_clipboard(image_id: String, state: &AppState) -> Result<(), String> {
    let conn = open_database(&state.database_path)?;
    let image = load_image_record(&conn, &image_id)?;
    let image_path = PathBuf::from(&image.path);
    let mut clipboard =
        arboard::Clipboard::new().map_err(|error| format!("访问系统剪贴板失败：{error}"))?;

    // 优先写文件列表（Windows: CF_HDROP），速度更接近资源管理器复制文件行为。
    let file_copy_result = clipboard.set().file_list(&[image_path.clone()]);
    if file_copy_result.is_ok() {
        return Ok(());
    }
    let file_copy_error = file_copy_result
        .err()
        .map(|error| error.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // 文件列表写入失败时，回退到像素写入，保证尽可能兼容图片粘贴目标。
    let bytes = fs::read(&image_path).map_err(|error| {
        format!(
            "文件路径复制失败：{file_copy_error}\n图像像素 fallback 失败：读取图片文件失败：{error}"
        )
    })?;
    let decoded = image::load_from_memory(&bytes).map_err(|error| {
        format!(
            "文件路径复制失败：{file_copy_error}\n图像像素 fallback 失败：解码图片失败：{error}"
        )
    })?;
    let rgba = decoded.to_rgba8();
    let (width, height) = rgba.dimensions();

    clipboard
        .set_image(arboard::ImageData {
            width: width as usize,
            height: height as usize,
            bytes: Cow::Owned(rgba.into_raw()),
        })
        .map_err(|error| {
            format!(
                "文件路径复制失败：{file_copy_error}\n图像像素 fallback 失败：写入系统剪贴板失败：{error}"
            )
        })?;

    Ok(())
}

pub fn test_wd_swinv2_tagger(
    image_id: String,
    general_threshold: Option<f32>,
    character_threshold: Option<f32>,
    model_dir: Option<String>,
    state: &AppState,
) -> Result<WdTaggerTestResult, String> {
    let conn = open_database(&state.database_path)?;
    let image = load_image_record(&conn, &image_id)?;

    if !Path::new(&image.path).exists() {
        return Err("Tagging failed: image file is missing".to_string());
    }

    let general_threshold = general_threshold.unwrap_or(0.35).clamp(0.0, 1.0);
    let character_threshold = character_threshold.unwrap_or(0.85).clamp(0.0, 1.0);

    let model_root = resolve_wd_tagger_model_dir(model_dir.as_deref())?;
    let model_path = model_root.join("model.onnx");
    let tags_path = model_root.join("selected_tags.csv");
    let script_path = resolve_wd_tagger_script_path()?;

    if !model_path.is_file() {
        return Err(format!("model.onnx not found: {}", model_path.display()));
    }
    if !tags_path.is_file() {
        return Err(format!("selected_tags.csv not found: {}", tags_path.display()));
    }

    let output = Command::new("python")
        .arg(&script_path)
        .arg("--image")
        .arg(&image.path)
        .arg("--model")
        .arg(&model_path)
        .arg("--tags")
        .arg(&tags_path)
        .arg("--general-threshold")
        .arg(general_threshold.to_string())
        .arg("--character-threshold")
        .arg(character_threshold.to_string())
        .arg("--image-id")
        .arg(&image_id)
        .output()
        .map_err(|error| {
            format!(
                "Failed to start WD tagger script: {error}. Ensure Python and onnxruntime/numpy/pillow are installed."
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let detail = if stderr.is_empty() {
            "unknown error".to_string()
        } else {
            stderr
        };
        return Err(format!(
            "WD tagger failed: {detail}. If dependency missing, run: pip install onnxruntime numpy pillow"
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Err("WD tagger returned empty output".to_string());
    }

    let mut result: WdTaggerTestResult =
        serde_json::from_str(&stdout).map_err(|error| format!("Failed to parse tagger output: {error}"))?;
    result.image_id = image_id;
    result.general_threshold = general_threshold;
    result.character_threshold = character_threshold;
    Ok(result)
}

pub fn test_chinese_clip_onnx_search(
    image_path: String,
    texts: Vec<String>,
    top_k: Option<usize>,
    model_dir: Option<String>,
    provider: Option<String>,
) -> Result<serde_json::Value, String> {
    if image_path.trim().is_empty() {
        return Err("Image path is empty".to_string());
    }
    if !Path::new(&image_path).is_file() {
        return Err(format!("Image not found: {image_path}"));
    }

    let mut candidates = Vec::<String>::new();
    for item in texts {
        let trimmed = item.trim();
        if !trimmed.is_empty() {
            candidates.push(trimmed.to_string());
        }
    }
    if candidates.is_empty() {
        return Err("Please provide at least one candidate text".to_string());
    }

    let model_root = resolve_chinese_clip_model_dir(model_dir.as_deref())?;
    let script_path = resolve_chinese_clip_smoke_script_path()?;
    let top_k = top_k.unwrap_or(5).max(1).min(200);
    let provider = provider.unwrap_or_else(|| "cpu".to_string());

    let mut command = Command::new("python");
    command
        .arg("-X")
        .arg("utf8")
        .arg(&script_path)
        .arg("--model-dir")
        .arg(&model_root)
        .arg("--image")
        .arg(&image_path)
        .arg("--top-k")
        .arg(top_k.to_string())
        .arg("--provider")
        .arg(provider);
    command.env("PYTHONUTF8", "1");
    command.env("PYTHONIOENCODING", "utf-8");
    for text in &candidates {
        command.arg("--text").arg(text);
    }

    let output = command.output().map_err(|error| {
        format!(
            "Failed to start Chinese-CLIP smoke test script: {error}. Ensure Python and dependencies are installed."
        )
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let detail = if stderr.is_empty() {
            "unknown error".to_string()
        } else {
            stderr
        };
        return Err(format!(
            "Chinese-CLIP smoke test failed: {detail}. If dependency missing, run: pip install onnxruntime numpy pillow"
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Err("Chinese-CLIP smoke test returned empty output".to_string());
    }

    serde_json::from_str(&stdout).map_err(|error| format!("Failed to parse Chinese-CLIP output: {error}"))
}

pub fn start_scan_all_folders_with_tagging(state: &AppState) -> Result<bool, String> {
    let mut running = state
        .background_scan_running
        .lock()
        .map_err(|_| "Background scan state is locked".to_string())?;
    if *running {
        if let Ok(mut pending) = state.background_scan_pending.lock() {
            *pending = true;
        }
        return Ok(false);
    }
    *running = true;
    drop(running);
    if let Ok(mut pending) = state.background_scan_pending.lock() {
        *pending = false;
    }

    let database_path = state.database_path.clone();
    let library_cache = Arc::clone(&state.library);
    let background_scan_running = Arc::clone(&state.background_scan_running);
    let background_scan_pending = Arc::clone(&state.background_scan_pending);
    let background_scan_progress = Arc::clone(&state.background_scan_progress);
    let startup_cleanup_running = Arc::clone(&state.startup_cleanup_running);
    thread::spawn(move || {
        wait_until_startup_cleanup_finished(&startup_cleanup_running);
        eprintln!("[wd-scan] worker started");
        loop {
            set_scan_progress(
                &background_scan_progress,
                BackgroundScanProgress {
                    running: true,
                    phase: "collecting".to_string(),
                    ..BackgroundScanProgress::default()
                },
            );

            match scan_all_folders_and_collect_new_images(&database_path, &background_scan_progress) {
                Ok(scan_result) => {
                    if let Ok(mut cache) = library_cache.lock() {
                        *cache = None;
                    }
                    if let Err(error) = tag_images_with_wd_model(
                        &database_path,
                        &scan_result.tag_queue_image_ids,
                        &background_scan_progress,
                    ) {
                        set_scan_progress_error(&background_scan_progress, &error);
                        push_scan_progress_recent_error(&background_scan_progress, &error);
                        eprintln!("[wd-tag] {error}");
                    }
                }
                Err(error) => {
                    set_scan_progress_error(&background_scan_progress, &error);
                    push_scan_progress_recent_error(&background_scan_progress, &error);
                    eprintln!("[wd-scan] {error}");
                }
            }

            if let Ok(mut cache) = library_cache.lock() {
                *cache = None;
            }

            let rerun = if let Ok(mut pending) = background_scan_pending.lock() {
                if *pending {
                    *pending = false;
                    true
                } else {
                    false
                }
            } else {
                false
            };
            if rerun {
                eprintln!("[wd-scan] pending rerun");
                continue;
            }
            break;
        }

        set_scan_progress_done(&background_scan_progress);
        if let Ok(mut cache) = library_cache.lock() {
            *cache = None;
        }
        if let Ok(mut running) = background_scan_running.lock() {
            *running = false;
        }
        if let Ok(mut pending) = background_scan_pending.lock() {
            *pending = false;
        }
        eprintln!("[wd-scan] worker finished");
    });

    Ok(true)
}

pub fn background_scan_status(state: &AppState) -> Result<BackgroundScanStatus, String> {
    let running = state
        .background_scan_running
        .lock()
        .map_err(|_| "Background scan state is locked".to_string())?;
    Ok(BackgroundScanStatus { running: *running })
}

pub fn background_scan_progress(state: &AppState) -> Result<BackgroundScanProgress, String> {
    state
        .background_scan_progress
        .lock()
        .map_err(|_| "Background scan progress state is locked".to_string())
        .map(|value| value.clone())
}

pub fn start_startup_cleanup(state: &AppState) -> Result<bool, String> {
    let background_scan_running = state
        .background_scan_running
        .lock()
        .map_err(|_| "Background scan state is locked".to_string())?;
    if *background_scan_running {
        return Ok(false);
    }
    drop(background_scan_running);

    let mut running = state
        .startup_cleanup_running
        .lock()
        .map_err(|_| "Startup cleanup state is locked".to_string())?;
    if *running {
        return Ok(false);
    }
    *running = true;
    drop(running);

    let database_path = state.database_path.clone();
    let library_cache = Arc::clone(&state.library);
    let startup_cleanup_running = Arc::clone(&state.startup_cleanup_running);
    let startup_cleanup_generation = Arc::clone(&state.startup_cleanup_generation);

    thread::spawn(move || {
        let result = cleanup_missing_library_images_batched(&database_path, 256);
        if let Err(error) = &result {
            eprintln!("[startup-cleanup] {error}");
        } else if let Ok(removed) = result {
            eprintln!("[startup-cleanup] removed {removed} missing library images");
        }

        if let Ok(mut cache) = library_cache.lock() {
            *cache = None;
        }
        if let Ok(mut generation) = startup_cleanup_generation.lock() {
            *generation += 1;
        }
        if let Ok(mut state) = startup_cleanup_running.lock() {
            *state = false;
        }
    });

    Ok(true)
}

fn wait_until_startup_cleanup_finished(startup_cleanup_running: &Arc<Mutex<bool>>) {
    loop {
        let running = match startup_cleanup_running.lock() {
            Ok(state) => *state,
            Err(_) => false,
        };
        if !running {
            break;
        }
        thread::sleep(std::time::Duration::from_millis(120));
    }
}

pub fn startup_cleanup_status(state: &AppState) -> Result<StartupCleanupStatus, String> {
    let running = state
        .startup_cleanup_running
        .lock()
        .map_err(|_| "Startup cleanup state is locked".to_string())
        .map(|value| *value)?;
    let generation = state
        .startup_cleanup_generation
        .lock()
        .map_err(|_| "Startup cleanup state is locked".to_string())
        .map(|value| *value)?;
    Ok(StartupCleanupStatus {
        running,
        generation,
    })
}

pub fn start_thumbnail_generation(state: &AppState) -> Result<bool, String> {
    let mut running = state
        .thumbnail_generation_running
        .lock()
        .map_err(|_| "Thumbnail generation state is locked".to_string())?;
    if *running {
        if let Ok(mut pending) = state.thumbnail_generation_pending.lock() {
            *pending = true;
        }
        return Ok(false);
    }
    *running = true;
    drop(running);

    if let Ok(mut pending) = state.thumbnail_generation_pending.lock() {
        *pending = false;
    }
    if let Ok(mut pause_requested) = state.thumbnail_generation_pause_requested.lock() {
        *pause_requested = false;
    }
    if let Ok(mut stop_requested) = state.thumbnail_generation_stop_requested.lock() {
        *stop_requested = false;
    }
    set_thumbnail_progress(
        &state.thumbnail_generation_progress,
        ThumbnailGenerationProgress {
            running: true,
            paused: false,
            phase: "queueing".to_string(),
            ..ThumbnailGenerationProgress::default()
        },
    );

    let database_path = state.database_path.clone();
    let library_cache = Arc::clone(&state.library);
    let thumb_running = Arc::clone(&state.thumbnail_generation_running);
    let thumb_pending = Arc::clone(&state.thumbnail_generation_pending);
    let thumb_pause_requested = Arc::clone(&state.thumbnail_generation_pause_requested);
    let thumb_stop_requested = Arc::clone(&state.thumbnail_generation_stop_requested);
    let thumb_progress = Arc::clone(&state.thumbnail_generation_progress);

    thread::spawn(move || {
        loop {
            set_thumbnail_progress_phase(&thumb_progress, "queueing");
            match generate_thumbnails_once(
                &database_path,
                &thumb_progress,
                &thumb_pause_requested,
                &thumb_stop_requested,
            ) {
                Ok(generated) => {
                    if generated > 0 {
                        if let Ok(mut cache) = library_cache.lock() {
                            *cache = None;
                        }
                    }
                }
                Err(error) => {
                    set_thumbnail_progress_error(&thumb_progress, &error);
                    push_thumbnail_progress_recent_error(&thumb_progress, &error);
                }
            }

            let rerun = if let Ok(mut pending) = thumb_pending.lock() {
                if *pending {
                    *pending = false;
                    true
                } else {
                    false
                }
            } else {
                false
            };
            if rerun {
                continue;
            }
            break;
        }

        set_thumbnail_progress_done(&thumb_progress);
        if let Ok(mut running) = thumb_running.lock() {
            *running = false;
        }
        if let Ok(mut pending) = thumb_pending.lock() {
            *pending = false;
        }
        if let Ok(mut pause_requested) = thumb_pause_requested.lock() {
            *pause_requested = false;
        }
        if let Ok(mut stop_requested) = thumb_stop_requested.lock() {
            *stop_requested = false;
        }
    });

    Ok(true)
}

pub fn thumbnail_generation_status(state: &AppState) -> Result<ThumbnailGenerationProgress, String> {
    state
        .thumbnail_generation_progress
        .lock()
        .map_err(|_| "Thumbnail generation progress state is locked".to_string())
        .map(|value| value.clone())
}

pub fn pause_thumbnail_generation(state: &AppState) -> Result<bool, String> {
    let running = state
        .thumbnail_generation_running
        .lock()
        .map_err(|_| "Thumbnail generation state is locked".to_string())
        .map(|value| *value)?;
    if !running {
        return Ok(false);
    }
    if let Ok(mut pause_requested) = state.thumbnail_generation_pause_requested.lock() {
        *pause_requested = true;
    }
    update_thumbnail_progress(&state.thumbnail_generation_progress, |progress| {
        progress.paused = true;
        progress.phase = "paused".to_string();
    });
    Ok(true)
}

pub fn resume_thumbnail_generation(state: &AppState) -> Result<bool, String> {
    let running = state
        .thumbnail_generation_running
        .lock()
        .map_err(|_| "Thumbnail generation state is locked".to_string())
        .map(|value| *value)?;
    if !running {
        return Ok(false);
    }
    if let Ok(mut pause_requested) = state.thumbnail_generation_pause_requested.lock() {
        *pause_requested = false;
    }
    update_thumbnail_progress(&state.thumbnail_generation_progress, |progress| {
        progress.paused = false;
        if progress.phase == "paused" {
            progress.phase = "generating".to_string();
        }
    });
    Ok(true)
}

pub fn stop_thumbnail_generation(state: &AppState) -> Result<bool, String> {
    let running = state
        .thumbnail_generation_running
        .lock()
        .map_err(|_| "Thumbnail generation state is locked".to_string())
        .map(|value| *value)?;
    if !running {
        return Ok(false);
    }
    if let Ok(mut stop_requested) = state.thumbnail_generation_stop_requested.lock() {
        *stop_requested = true;
    }
    if let Ok(mut pause_requested) = state.thumbnail_generation_pause_requested.lock() {
        *pause_requested = false;
    }
    update_thumbnail_progress(&state.thumbnail_generation_progress, |progress| {
        progress.paused = false;
        progress.phase = "stopping".to_string();
    });
    Ok(true)
}

pub fn clear_thumbnail_cache(state: &AppState) -> Result<(), String> {
    let running = state
        .thumbnail_generation_running
        .lock()
        .map_err(|_| "Thumbnail generation state is locked".to_string())
        .map(|value| *value)?;
    if running {
        return Err("缩略图任务正在运行，请先停止后再清理缓存".to_string());
    }

    let conn = open_database(&state.database_path)?;
    clear_thumbnail_cache_storage(&conn, &state.database_path)?;
    if let Ok(mut cache) = state.library.lock() {
        *cache = None;
    }
    set_thumbnail_progress(
        &state.thumbnail_generation_progress,
        ThumbnailGenerationProgress::default(),
    );
    Ok(())
}

pub fn rebuild_thumbnail_cache(state: &AppState) -> Result<bool, String> {
    clear_thumbnail_cache(state)?;
    start_thumbnail_generation(state)
}

pub fn start_natural_language_scan(state: &AppState) -> Result<bool, String> {
    let mut running = state
        .natural_language_scan_running
        .lock()
        .map_err(|_| "Natural language scan state is locked".to_string())?;
    if *running {
        if let Ok(mut pending) = state.natural_language_scan_pending.lock() {
            *pending = true;
        }
        return Ok(false);
    }
    *running = true;
    drop(running);
    if let Ok(mut pending) = state.natural_language_scan_pending.lock() {
        *pending = false;
    }

    let database_path = state.database_path.clone();
    let library_cache = Arc::clone(&state.library);
    let scan_running = Arc::clone(&state.natural_language_scan_running);
    let scan_pending = Arc::clone(&state.natural_language_scan_pending);
    let scan_progress = Arc::clone(&state.natural_language_scan_progress);

    thread::spawn(move || {
        loop {
            set_natural_language_scan_progress(
                &scan_progress,
                NaturalLanguageScanProgress {
                    running: true,
                    phase: "collecting".to_string(),
                    ..NaturalLanguageScanProgress::default()
                },
            );

            if let Err(error) = generate_natural_language_embeddings_once(&database_path, &scan_progress) {
                set_natural_language_scan_progress_error(&scan_progress, &error);
                push_natural_language_scan_recent_error(&scan_progress, &error);
                eprintln!("[clip-scan] {error}");
            }

            if let Ok(mut cache) = library_cache.lock() {
                *cache = None;
            }

            let rerun = if let Ok(mut pending) = scan_pending.lock() {
                if *pending {
                    *pending = false;
                    true
                } else {
                    false
                }
            } else {
                false
            };
            if rerun {
                eprintln!("[clip-scan] pending rerun");
                continue;
            }
            break;
        }

        set_natural_language_scan_progress_done(&scan_progress);
        if let Ok(mut running) = scan_running.lock() {
            *running = false;
        }
        if let Ok(mut pending) = scan_pending.lock() {
            *pending = false;
        }
        eprintln!("[clip-scan] worker finished");
    });

    Ok(true)
}

pub fn natural_language_scan_status(state: &AppState) -> Result<NaturalLanguageScanStatus, String> {
    let running = state
        .natural_language_scan_running
        .lock()
        .map_err(|_| "Natural language scan state is locked".to_string())?;
    Ok(NaturalLanguageScanStatus { running: *running })
}

pub fn natural_language_scan_progress(state: &AppState) -> Result<NaturalLanguageScanProgress, String> {
    state
        .natural_language_scan_progress
        .lock()
        .map_err(|_| "Natural language scan progress state is locked".to_string())
        .map(|value| value.clone())
}

pub fn search_gallery_image_ids_by_natural_language(
    query: String,
    candidate_image_ids: Option<Vec<String>>,
    state: &AppState,
) -> Result<Vec<String>, String> {
    let trimmed_query = query.trim();
    if trimmed_query.is_empty() {
        return Ok(Vec::new());
    }

    let conn = open_database(&state.database_path)?;
    let model_root = resolve_chinese_clip_model_dir(None)?;
    let script_path = resolve_chinese_clip_embed_script_path()?;
    let text_embedding = run_chinese_clip_text_embedding(trimmed_query, &model_root, &script_path)?;
    if text_embedding.is_empty() {
        return Ok(Vec::new());
    }

    let candidate_filter = candidate_image_ids.map(|list| {
        list.into_iter()
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty())
            .collect::<HashSet<_>>()
    });
    if matches!(candidate_filter, Some(ref set) if set.is_empty()) {
        return Ok(Vec::new());
    }

    let mut stmt = conn
        .prepare(
            "
            SELECT e.image_id, e.vector_json
            FROM image_clip_embeddings e
            JOIN images i ON i.id = e.image_id
            WHERE e.model_name = ?1
              AND i.source = 'library'
              AND COALESCE(i.trashed, 0) = 0
            ",
        )
        .map_err(|error| format!("Failed to prepare natural language search query: {error}"))?;
    let rows = stmt
        .query_map(params![CHINESE_CLIP_MODEL_NAME], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| format!("Failed to run natural language search query: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to run natural language search query: {error}"))?;
    drop(stmt);

    let mut scored = Vec::<(String, f32)>::new();
    for (image_id, vector_json) in rows {
        if let Some(filter) = &candidate_filter {
            if !filter.contains(&image_id) {
                continue;
            }
        }
        let vector: Vec<f32> = serde_json::from_str(&vector_json)
            .map_err(|error| format!("Failed to parse image embedding for {image_id}: {error}"))?;
        if vector.len() != text_embedding.len() {
            continue;
        }
        if let Some(score) = cosine_similarity(&text_embedding, &vector) {
            scored.push((image_id, score));
        }
    }

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    Ok(scored.into_iter().map(|(image_id, _)| image_id).collect())
}

pub fn list_image_auto_tags(
    image_id: String,
    state: &AppState,
) -> Result<ImageAutoTagSummary, String> {
    let conn = open_database(&state.database_path)?;
    let mut stmt = conn
        .prepare(
            "
            SELECT category, tag_en, tag_zh, confidence
            FROM image_auto_tags
            WHERE image_id = ?1
            ORDER BY
              CASE category WHEN 'character' THEN 0 WHEN 'general' THEN 1 ELSE 2 END,
              confidence DESC,
              tag_en COLLATE NOCASE
            ",
        )
        .map_err(|error| format!("Failed to load image auto tags: {error}"))?;

    let tags = stmt
        .query_map(params![image_id.clone()], |row| {
            Ok(ImageAutoTag {
                category: row.get(0)?,
                tag_en: row.get(1)?,
                tag_zh: row.get(2)?,
                confidence: row.get(3)?,
            })
        })
        .map_err(|error| format!("Failed to load image auto tags: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to load image auto tags: {error}"))?;

    let mut character_tags = Vec::<ImageAutoTag>::new();
    let mut general_tags = Vec::<ImageAutoTag>::new();
    for tag in tags {
        if tag.category == "character" {
            character_tags.push(tag);
        } else if tag.category == "general" {
            general_tags.push(tag);
        }
    }

    Ok(ImageAutoTagSummary {
        image_id,
        character_tags,
        general_tags,
    })
}

pub fn suggest_known_auto_tags(
    query: String,
    limit: Option<i64>,
    state: &AppState,
) -> Result<Vec<KnownAutoTagSuggestion>, String> {
    let keyword = query.trim();
    if keyword.is_empty() {
        return Ok(Vec::new());
    }
    let conn = open_database(&state.database_path)?;
    let keyword_lower = keyword.to_lowercase();
    let like = format!("%{}%", escape_like_pattern(&keyword_lower));
    let like_prefix = format!("{}%", escape_like_pattern(&keyword_lower));
    let limit = limit.unwrap_or(20).clamp(1, 80);
    let mut stmt = conn
        .prepare(
            "
            SELECT
              k.tag_en,
              COALESCE(NULLIF(k.tag_zh, ''), d.tag_zh) AS tag_zh,
              k.image_count
            FROM known_image_tags k
            LEFT JOIN tag_dictionary d ON d.tag_en = k.tag_en
            WHERE k.model_name = ?1
              AND k.image_count > 0
              AND (
                LOWER(k.tag_en) LIKE ?2 ESCAPE '\\'
                OR LOWER(COALESCE(NULLIF(k.tag_zh, ''), d.tag_zh, '')) LIKE ?2 ESCAPE '\\'
              )
            ORDER BY
              CASE
                WHEN LOWER(COALESCE(NULLIF(k.tag_zh, ''), d.tag_zh, '')) LIKE ?3 ESCAPE '\\' THEN 0
                WHEN LOWER(k.tag_en) LIKE ?3 ESCAPE '\\' THEN 1
                ELSE 2
              END,
              k.image_count DESC,
              k.tag_en COLLATE NOCASE
            LIMIT ?4
            ",
        )
        .map_err(|error| format!("Failed to prepare known tag suggestion query: {error}"))?;

    let rows = stmt
        .query_map(params![WD_TAGGER_MODEL_NAME, like, like_prefix, limit], |row| {
            Ok(KnownAutoTagSuggestion {
                tag_en: row.get(0)?,
                tag_zh: row.get(1)?,
                image_count: row.get(2)?,
            })
        })
        .map_err(|error| format!("Failed to query known tag suggestions: {error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to query known tag suggestions: {error}"))
}

pub fn search_gallery_image_ids(
    filters: GallerySearchFilters,
    state: &AppState,
) -> Result<Vec<String>, String> {
    let conn = open_database(&state.database_path)?;
    let mut sql = String::from(
        "
        SELECT images.id
        FROM images
        WHERE images.source = 'library'
          AND COALESCE(images.trashed, 0) = 0
        ",
    );
    let mut params_values = Vec::<Value>::new();

    let file_name_query = filters.file_name_query.trim().to_lowercase();
    if !file_name_query.is_empty() {
        sql.push_str(" AND LOWER(images.file_name) LIKE ? ESCAPE '\\'");
        params_values.push(Value::Text(format!("%{}%", escape_like_pattern(&file_name_query))));
    }

    let confidence_min = filters.confidence_min.clamp(0.0, 1.0);
    let confidence_max = filters.confidence_max.clamp(confidence_min, 1.0);
    let has_confidence_filter = confidence_min > 0.000_1 || confidence_max < 0.999_9;

    let mut zh_tags = filters
        .chinese_tag_ens
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    zh_tags.sort();
    zh_tags.dedup();
    let has_zh_tag_constraints = !zh_tags.is_empty();
    for tag_en in zh_tags {
        sql.push_str(
            "
            AND EXISTS (
              SELECT 1
              FROM image_auto_tags t
              WHERE t.image_id = images.id
                AND t.model_name = ?
                AND t.tag_en = ?
                AND t.confidence BETWEEN ? AND ?
            )
            ",
        );
        params_values.push(Value::Text(WD_TAGGER_MODEL_NAME.to_string()));
        params_values.push(Value::Text(tag_en));
        params_values.push(Value::Real(confidence_min as f64));
        params_values.push(Value::Real(confidence_max as f64));
    }

    let english_tokens = split_search_tokens(&filters.english_query);
    let has_english_constraints = !english_tokens.is_empty();
    for token in english_tokens {
        sql.push_str(
            "
            AND EXISTS (
              SELECT 1
              FROM image_auto_tags t
              WHERE t.image_id = images.id
                AND t.model_name = ?
                AND LOWER(REPLACE(t.tag_en, '_', ' ')) LIKE ? ESCAPE '\\'
                AND t.confidence BETWEEN ? AND ?
            )
            ",
        );
        params_values.push(Value::Text(WD_TAGGER_MODEL_NAME.to_string()));
        params_values.push(Value::Text(format!("%{}%", escape_like_pattern(&token))));
        params_values.push(Value::Real(confidence_min as f64));
        params_values.push(Value::Real(confidence_max as f64));
    }

    if has_confidence_filter && !has_zh_tag_constraints && !has_english_constraints {
        sql.push_str(
            "
            AND EXISTS (
              SELECT 1
              FROM image_auto_tags t
              WHERE t.image_id = images.id
                AND t.model_name = ?
                AND t.confidence BETWEEN ? AND ?
            )
            ",
        );
        params_values.push(Value::Text(WD_TAGGER_MODEL_NAME.to_string()));
        params_values.push(Value::Real(confidence_min as f64));
        params_values.push(Value::Real(confidence_max as f64));
    }

    sql.push_str(" ORDER BY images.modified_at DESC, images.path ASC");
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|error| format!("Failed to prepare gallery search query: {error}"))?;

    let rows = stmt
        .query_map(params_from_iter(params_values.iter()), |row| {
            row.get::<_, String>(0)
        })
        .map_err(|error| format!("Failed to run gallery search query: {error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to run gallery search query: {error}"))
}

pub fn create_user_folder(
    parent_id: Option<i64>,
    name: String,
    state: &AppState,
) -> Result<LibraryStore, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("请输入文件夹名称".to_string());
    }

    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let conn = open_database(&state.database_path)?;
    let now = now_ms();
    let sort_order = next_user_folder_sort_order(&conn, parent_id)?;

    conn.execute(
        "
        INSERT INTO user_folders (parent_id, name, sort_order, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?4)
        ",
        params![parent_id, name, sort_order, now],
    )
    .map_err(|error| format!("创建文件夹失败：{error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn rename_user_folder(
    folder_id: i64,
    name: String,
    state: &AppState,
) -> Result<LibraryStore, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("请输入文件夹名称".to_string());
    }

    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let conn = open_database(&state.database_path)?;
    conn.execute(
        "UPDATE user_folders SET name = ?1, updated_at = ?2 WHERE id = ?3",
        params![name, now_ms(), folder_id],
    )
    .map_err(|error| format!("重命名文件夹失败：{error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn reorder_user_folder(
    folder_id: i64,
    target_folder_id: i64,
    state: &AppState,
) -> Result<LibraryStore, String> {
    if folder_id == target_folder_id {
        return list_library_from_state(state);
    }

    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let mut conn = open_database(&state.database_path)?;

    let dragged_parent = user_folder_parent_id(&conn, folder_id)?;
    let target_parent = user_folder_parent_id(&conn, target_folder_id)?;
    if dragged_parent != target_parent {
        return Err("暂时只支持同层级文件夹排序".to_string());
    }

    let tx = conn
        .transaction()
        .map_err(|error| format!("打开文件夹排序事务失败：{error}"))?;
    let sibling_ids = load_user_folder_sibling_ids(&tx, dragged_parent)?;
    let Some(from_index) = sibling_ids.iter().position(|id| *id == folder_id) else {
        return Err("找不到要排序的文件夹".to_string());
    };
    let Some(to_index) = sibling_ids.iter().position(|id| *id == target_folder_id) else {
        return Err("找不到目标文件夹".to_string());
    };

    let mut reordered = sibling_ids;
    let moved = reordered.remove(from_index);
    let insert_index = to_index;
    reordered.insert(insert_index, moved);

    for (index, id) in reordered.iter().enumerate() {
        tx.execute(
            "UPDATE user_folders SET sort_order = ?1, updated_at = ?2 WHERE id = ?3",
            params![index as i64, now_ms(), id],
        )
        .map_err(|error| format!("保存文件夹排序失败：{error}"))?;
    }

    tx.commit()
        .map_err(|error| format!("保存文件夹排序失败：{error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn delete_user_folder(folder_id: i64, state: &AppState) -> Result<LibraryStore, String> {
    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let conn = open_database(&state.database_path)?;

    conn.execute("PRAGMA foreign_keys = ON", [])
        .map_err(|error| format!("启用数据库外键失败：{error}"))?;
    conn.execute("DELETE FROM user_folders WHERE id = ?1", params![folder_id])
        .map_err(|error| format!("删除文件夹失败：{error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

fn next_user_folder_sort_order(conn: &Connection, parent_id: Option<i64>) -> Result<i64, String> {
    conn.query_row(
        "
        SELECT COALESCE(MAX(sort_order), -1) + 1
        FROM user_folders
        WHERE parent_id IS ?1
        ",
        params![parent_id],
        |row| row.get(0),
    )
    .map_err(|error| format!("读取文件夹排序失败：{error}"))
}

fn next_sort_order(
    conn: &Connection,
    table_name: &str,
    folder_id: Option<i64>,
) -> Result<i64, String> {
    let sql = match table_name {
        "reference_board_folders" => {
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM reference_board_folders"
        }
        "reference_boards" => {
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM reference_boards WHERE folder_id IS ?1"
        }
        _ => return Err("未知排序表".to_string()),
    };

    if table_name == "reference_boards" {
        conn.query_row(sql, params![folder_id], |row| row.get(0))
    } else {
        conn.query_row(sql, [], |row| row.get(0))
    }
    .map_err(|error| format!("读取排序失败：{error}"))
}

fn user_folder_parent_id(conn: &Connection, folder_id: i64) -> Result<Option<i64>, String> {
    conn.query_row(
        "SELECT parent_id FROM user_folders WHERE id = ?1",
        params![folder_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(|error| format!("读取文件夹层级失败：{error}"))?
    .ok_or_else(|| "找不到文件夹".to_string())
}

fn load_user_folder_sibling_ids(
    conn: &Connection,
    parent_id: Option<i64>,
) -> Result<Vec<i64>, String> {
    let mut stmt = conn
        .prepare(
            "
            SELECT id
            FROM user_folders
            WHERE parent_id IS ?1
            ORDER BY sort_order, name COLLATE NOCASE, id
            ",
        )
        .map_err(|error| format!("读取同级文件夹失败：{error}"))?;

    let ids = stmt
        .query_map(params![parent_id], |row| row.get::<_, i64>(0))
        .map_err(|error| format!("读取同级文件夹失败：{error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取同级文件夹失败：{error}"))?;

    Ok(ids)
}

pub fn assign_image_to_user_folder(
    image_id: String,
    folder_id: i64,
    state: &AppState,
) -> Result<LibraryStore, String> {
    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let conn = open_database(&state.database_path)?;

    let has_children: i64 = conn
        .query_row(
            "
            SELECT EXISTS(
              SELECT 1
              FROM user_folders
              WHERE parent_id = ?1
            )
            ",
            params![folder_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("检查文件夹层级失败：{error}"))?;
    if has_children != 0 {
        return Err("只能将图片放入最小层级文件夹".to_string());
    }

    conn.execute(
        "
        INSERT OR IGNORE INTO image_user_folders (image_id, folder_id, assigned_at)
        VALUES (?1, ?2, ?3)
        ",
        params![image_id, folder_id, now_ms()],
    )
    .map_err(|error| format!("添加到文件夹失败：{error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn remove_image_from_user_folder(
    image_id: String,
    folder_id: i64,
    state: &AppState,
) -> Result<LibraryStore, String> {
    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let conn = open_database(&state.database_path)?;

    conn.execute(
        "
        DELETE FROM image_user_folders
        WHERE image_id = ?1 AND folder_id = ?2
        ",
        params![image_id, folder_id],
    )
    .map_err(|error| format!("从文件夹移除图片失败：{error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn create_reference_board_folder(
    name: String,
    state: &AppState,
) -> Result<LibraryStore, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("请输入参考板文件夹名称".to_string());
    }

    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let conn = open_database(&state.database_path)?;
    let now = now_ms();
    let sort_order = next_sort_order(&conn, "reference_board_folders", None)?;

    conn.execute(
        "
        INSERT INTO reference_board_folders (name, sort_order, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?3)
        ",
        params![name, sort_order, now],
    )
    .map_err(|error| format!("创建参考板文件夹失败：{error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn create_reference_board(
    folder_id: Option<i64>,
    name: String,
    state: &AppState,
) -> Result<LibraryStore, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("请输入参考板名称".to_string());
    }

    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let conn = open_database(&state.database_path)?;
    let now = now_ms();
    let sort_order = next_sort_order(&conn, "reference_boards", folder_id)?;

    conn.execute(
        "
        INSERT INTO reference_boards (folder_id, name, sort_order, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?4)
        ",
        params![folder_id, name, sort_order, now],
    )
    .map_err(|error| format!("创建参考板失败：{error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn add_image_to_reference_board(
    image_id: String,
    board_id: i64,
    state: &AppState,
) -> Result<LibraryStore, String> {
    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let conn = open_database(&state.database_path)?;
    let (source_width, source_height) = conn
        .query_row(
            "SELECT width, height FROM images WHERE id = ?1",
            params![image_id],
            |row| Ok((row.get::<_, u32>(0)?, row.get::<_, u32>(1)?)),
        )
        .map_err(|error| format!("读取参考图尺寸失败：{error}"))?;
    let (item_width, item_height) = default_reference_board_item_size(source_width, source_height);
    let next_index = conn
        .query_row(
            "
            SELECT COALESCE(MAX(z_index), -1) + 1
            FROM reference_board_items
            WHERE board_id = ?1
            ",
            params![board_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("读取参考板层级失败：{error}"))?;

    conn.execute(
        "
        INSERT OR IGNORE INTO reference_board_items (
          board_id, image_id, x, y, width, height, rotation, z_index, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?8)
        ",
        params![
            board_id,
            image_id,
            (next_index % 5) as f32 * 28.0,
            (next_index / 5) as f32 * 28.0,
            item_width,
            item_height,
            next_index,
            now_ms()
        ],
    )
    .map_err(|error| format!("添加到参考板失败：{error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn paste_image_to_reference_board(
    board_id: i64,
    image_bytes: Vec<u8>,
    mime_type: String,
    x: f32,
    y: f32,
    state: &AppState,
) -> Result<LibraryStore, String> {
    if image_bytes.is_empty() {
        return Err("剪贴板里没有可用的图片数据".to_string());
    }

    let decoded = image::load_from_memory(&image_bytes)
        .map_err(|error| format!("读取剪贴板图片失败：{error}"))?;
    let source_width = decoded.width();
    let source_height = decoded.height();
    let (item_width, item_height) = default_reference_board_item_size(source_width, source_height);
    let extension = clipboard_image_extension(&mime_type);

    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let mut conn = open_database(&state.database_path)?;
    let app_data_dir = state
        .database_path
        .parent()
        .ok_or_else(|| "找不到应用数据目录".to_string())?;
    let clipboard_dir = app_data_dir.join("clipboard");
    fs::create_dir_all(&clipboard_dir)
        .map_err(|error| format!("创建剪贴板图片目录失败：{error}"))?;

    let now = now_ms();
    let mut file_path = clipboard_dir.join(format!("clipboard-{now}.{extension}"));
    let mut suffix = 1;
    while file_path.exists() {
        file_path = clipboard_dir.join(format!("clipboard-{now}-{suffix}.{extension}"));
        suffix += 1;
    }
    fs::write(&file_path, &image_bytes).map_err(|error| format!("保存剪贴板图片失败：{error}"))?;

    let tx = conn
        .transaction()
        .map_err(|error| format!("打开参考板粘贴事务失败：{error}"))?;
    let folder_path = clipboard_dir.to_string_lossy().to_string();
    let folder_id = upsert_hidden_folder(&tx, &folder_path, now)?;
    let file_name = file_path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| format!("clipboard.{extension}"));
    let image_path = file_path.to_string_lossy().to_string();
    let file_size = image_bytes.len() as i64;

    tx.execute(
        "
        INSERT INTO images (
          id, path, file_name, ext, width, height, file_size, modified_at,
          imported_at, folder_id, missing
        )
        VALUES (?1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?8, 0)
        ",
        params![
            image_path,
            file_name,
            extension,
            source_width,
            source_height,
            file_size,
            now,
            folder_id,
        ],
    )
    .map_err(|error| format!("保存剪贴板图片索引失败：{error}"))?;
    tx.execute(
        "UPDATE images SET source = 'reference' WHERE id = ?1",
        params![image_path],
    )
    .map_err(|error| format!("标记临时参考图失败：{error}"))?;

    let next_index = tx
        .query_row(
            "
            SELECT COALESCE(MAX(z_index), -1) + 1
            FROM reference_board_items
            WHERE board_id = ?1
            ",
            params![board_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("读取参考板层级失败：{error}"))?;

    tx.execute(
        "
        INSERT INTO reference_board_items (
          board_id, image_id, x, y, width, height, rotation, z_index, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?8)
        ",
        params![
            board_id,
            image_path,
            x,
            y,
            item_width,
            item_height,
            next_index,
            now,
        ],
    )
    .map_err(|error| format!("粘贴到参考板失败：{error}"))?;

    tx.commit()
        .map_err(|error| format!("保存参考板粘贴事务失败：{error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn rename_reference_board(
    board_id: i64,
    name: String,
    state: &AppState,
) -> Result<LibraryStore, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("请输入参考板名称".to_string());
    }

    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let conn = open_database(&state.database_path)?;
    conn.execute(
        "UPDATE reference_boards SET name = ?1, updated_at = ?2 WHERE id = ?3",
        params![name, now_ms(), board_id],
    )
    .map_err(|error| format!("重命名参考板失败：{error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn rename_reference_board_folder(
    folder_id: i64,
    name: String,
    state: &AppState,
) -> Result<LibraryStore, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("请输入参考板文件夹名称".to_string());
    }

    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let conn = open_database(&state.database_path)?;
    conn.execute(
        "UPDATE reference_board_folders SET name = ?1, updated_at = ?2 WHERE id = ?3",
        params![name, now_ms(), folder_id],
    )
    .map_err(|error| format!("重命名参考板文件夹失败：{error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn reorder_reference_board_folder(
    folder_id: i64,
    target_folder_id: i64,
    state: &AppState,
) -> Result<LibraryStore, String> {
    if folder_id == target_folder_id {
        return list_library_from_state(state);
    }

    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let mut conn = open_database(&state.database_path)?;
    let tx = conn
        .transaction()
        .map_err(|error| format!("打开参考板文件夹排序事务失败：{error}"))?;
    let folder_ids = load_reference_board_folder_ids(&tx)?;
    let Some(from_index) = folder_ids.iter().position(|id| *id == folder_id) else {
        return Err("找不到要排序的参考板文件夹".to_string());
    };
    let Some(to_index) = folder_ids.iter().position(|id| *id == target_folder_id) else {
        return Err("找不到目标参考板文件夹".to_string());
    };

    let mut reordered = folder_ids;
    let moved = reordered.remove(from_index);
    reordered.insert(to_index, moved);
    for (index, id) in reordered.iter().enumerate() {
        tx.execute(
            "UPDATE reference_board_folders SET sort_order = ?1, updated_at = ?2 WHERE id = ?3",
            params![index as i64, now_ms(), id],
        )
        .map_err(|error| format!("保存参考板文件夹排序失败：{error}"))?;
    }
    tx.commit()
        .map_err(|error| format!("保存参考板文件夹排序事务失败：{error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn move_reference_board_to_folder(
    board_id: i64,
    folder_id: Option<i64>,
    state: &AppState,
) -> Result<LibraryStore, String> {
    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let conn = open_database(&state.database_path)?;
    let sort_order = next_sort_order(&conn, "reference_boards", folder_id)?;
    conn.execute(
        "
        UPDATE reference_boards
        SET folder_id = ?1, sort_order = ?2, updated_at = ?3
        WHERE id = ?4
        ",
        params![folder_id, sort_order, now_ms(), board_id],
    )
    .map_err(|error| format!("移动参考板失败：{error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn reorder_reference_board(
    board_id: i64,
    target_board_id: i64,
    state: &AppState,
) -> Result<LibraryStore, String> {
    if board_id == target_board_id {
        return list_library_from_state(state);
    }

    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let mut conn = open_database(&state.database_path)?;
    let target_folder = reference_board_folder_id(&conn, target_board_id)?;
    let tx = conn
        .transaction()
        .map_err(|error| format!("打开参考板排序事务失败：{error}"))?;
    tx.execute(
        "UPDATE reference_boards SET folder_id = ?1 WHERE id = ?2",
        params![target_folder, board_id],
    )
    .map_err(|error| format!("移动参考板失败：{error}"))?;
    let sibling_ids = load_reference_board_sibling_ids(&tx, target_folder)?;
    let Some(from_index) = sibling_ids.iter().position(|id| *id == board_id) else {
        return Err("找不到要排序的参考板".to_string());
    };
    let Some(to_index) = sibling_ids.iter().position(|id| *id == target_board_id) else {
        return Err("找不到目标参考板".to_string());
    };

    let mut reordered = sibling_ids;
    let moved = reordered.remove(from_index);
    reordered.insert(to_index, moved);
    for (index, id) in reordered.iter().enumerate() {
        tx.execute(
            "UPDATE reference_boards SET sort_order = ?1, updated_at = ?2 WHERE id = ?3",
            params![index as i64, now_ms(), id],
        )
        .map_err(|error| format!("保存参考板排序失败：{error}"))?;
    }
    tx.commit()
        .map_err(|error| format!("保存参考板排序事务失败：{error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn delete_reference_board(board_id: i64, state: &AppState) -> Result<LibraryStore, String> {
    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let conn = open_database(&state.database_path)?;
    conn.execute("PRAGMA foreign_keys = ON", [])
        .map_err(|error| format!("启用数据库外键失败：{error}"))?;
    conn.execute(
        "DELETE FROM reference_boards WHERE id = ?1",
        params![board_id],
    )
    .map_err(|error| format!("删除参考板失败：{error}"))?;

    cleanup_orphan_reference_images(&conn)?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn delete_reference_board_folder(
    folder_id: i64,
    state: &AppState,
) -> Result<LibraryStore, String> {
    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let conn = open_database(&state.database_path)?;
    conn.execute("PRAGMA foreign_keys = ON", [])
        .map_err(|error| format!("启用数据库外键失败：{error}"))?;
    conn.execute(
        "DELETE FROM reference_board_folders WHERE id = ?1",
        params![folder_id],
    )
    .map_err(|error| format!("删除参考板文件夹失败：{error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn remove_reference_board_item(item_id: i64, state: &AppState) -> Result<LibraryStore, String> {
    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let conn = open_database(&state.database_path)?;
    conn.execute(
        "DELETE FROM reference_board_items WHERE id = ?1",
        params![item_id],
    )
    .map_err(|error| format!("移除参考图失败：{error}"))?;

    cleanup_orphan_reference_images(&conn)?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn update_reference_board_item_layout(
    item_id: i64,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    rotation: f32,
    state: &AppState,
) -> Result<LibraryStore, String> {
    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let conn = open_database(&state.database_path)?;
    conn.execute(
        "
        UPDATE reference_board_items
        SET x = ?1, y = ?2, width = ?3, height = ?4, rotation = ?5
        WHERE id = ?6
        ",
        params![x, y, width.max(48.0), height.max(48.0), rotation, item_id],
    )
    .map_err(|error| format!("保存参考图布局失败：{error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn bring_reference_board_item_to_front(item_id: i64, state: &AppState) -> Result<LibraryStore, String> {
    let mut library = state
        .library
        .lock()
        .map_err(|_| "鍥惧簱鐘舵€佽鍗犵敤锛岃绋嶅悗鍐嶈瘯".to_string())?;
    let conn = open_database(&state.database_path)?;
    let item = load_reference_board_item(&conn, item_id)?;
    let next_index = next_reference_board_z_index(&conn, item.board_id)?;
    conn.execute(
        "
        UPDATE reference_board_items
        SET z_index = ?1
        WHERE id = ?2
        ",
        params![next_index, item_id],
    )
    .map_err(|error| format!("缃簬鏈€鍓嶅眰澶辫触锛歿{error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn duplicate_reference_board_item(
    item_id: i64,
    x: Option<f32>,
    y: Option<f32>,
    state: &AppState,
) -> Result<LibraryStore, String> {
    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let conn = open_database(&state.database_path)?;
    let item = load_reference_board_item(&conn, item_id)?;
    let next_index = next_reference_board_z_index(&conn, item.board_id)?;
    conn.execute(
        "
        INSERT INTO reference_board_items (
          board_id, image_id, x, y, width, height, rotation, z_index, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ",
        params![
            item.board_id,
            item.image_id,
            x.unwrap_or(item.x + 28.0),
            y.unwrap_or(item.y + 28.0),
            item.width,
            item.height,
            item.rotation,
            next_index,
            now_ms(),
        ],
    )
    .map_err(|error| format!("复制参考图失败：{error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn restore_reference_board_item(
    board_id: i64,
    image_id: String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    rotation: f32,
    z_index: i64,
    state: &AppState,
) -> Result<LibraryStore, String> {
    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let conn = open_database(&state.database_path)?;
    conn.execute(
        "
        INSERT INTO reference_board_items (
          board_id, image_id, x, y, width, height, rotation, z_index, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ",
        params![
            board_id,
            image_id,
            x,
            y,
            width.max(48.0),
            height.max(48.0),
            rotation,
            z_index,
            now_ms(),
        ],
    )
    .map_err(|error| format!("恢复参考图失败：{error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn export_reference_board_item_from_state(
    item_id: i64,
    destination: String,
    state: &AppState,
) -> Result<(), String> {
    let conn = open_database(&state.database_path)?;
    let item = load_reference_board_item(&conn, item_id)?;
    copy_image_to_destination(&item.image_id, Path::new(&destination))
}

pub fn export_gallery_image_from_state(
    image_id: String,
    destination: String,
    state: &AppState,
) -> Result<(), String> {
    let conn = open_database(&state.database_path)?;
    let image = load_image_record(&conn, &image_id)?;
    copy_image_to_destination(&image.path, Path::new(&destination))
}

pub fn import_reference_board_item_to_library(
    item_id: i64,
    folder_id: i64,
    state: &AppState,
) -> Result<LibraryStore, String> {
    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let mut conn = open_database(&state.database_path)?;
    let item = load_reference_board_item(&conn, item_id)?;
    let image = load_image_record(&conn, &item.image_id)?;
    if image.source != "reference" {
        let store = load_store(&conn)?;
        *library = Some(store.clone());
        return Ok(store);
    }
    let folder_path = conn
        .query_row(
            "SELECT path FROM folders WHERE id = ?1 AND COALESCE(hidden, 0) = 0",
            params![folder_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("读取图库文件夹失败：{error}"))?
        .ok_or_else(|| "找不到目标图库文件夹".to_string())?;
    let target_path = unique_file_path(Path::new(&folder_path), &image.file_name);

    fs::copy(&image.path, &target_path)
        .map_err(|error| format!("复制到图库文件夹失败：{error}"))?;
    let metadata =
        fs::metadata(&target_path).map_err(|error| format!("读取图库图片信息失败：{error}"))?;
    let target_path_text = target_path.to_string_lossy().to_string();
    let now = now_ms();
    let tx = conn
        .transaction()
        .map_err(|error| format!("打开加入图库事务失败：{error}"))?;
    tx.execute(
        "
        INSERT INTO images (
          id, path, file_name, ext, width, height, file_size, modified_at,
          imported_at, folder_id, missing, source
        )
        VALUES (?1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, 'library')
        ",
        params![
            target_path_text,
            target_path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| image.file_name.clone()),
            image.ext.clone(),
            image.width,
            image.height,
            metadata.len() as i64,
            metadata.modified().ok().map(system_time_ms).unwrap_or(now),
            now,
            folder_id,
        ],
    )
    .map_err(|error| format!("保存图库图片索引失败：{error}"))?;
    tx.execute(
        "UPDATE reference_board_items SET image_id = ?1 WHERE id = ?2",
        params![target_path_text, item_id],
    )
    .map_err(|error| format!("更新参考图索引失败：{error}"))?;
    tx.commit()
        .map_err(|error| format!("保存加入图库事务失败：{error}"))?;
    cleanup_orphan_reference_images(&conn)?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn auto_arrange_reference_board(
    board_id: i64,
    state: &AppState,
) -> Result<LibraryStore, String> {
    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let mut conn = open_database(&state.database_path)?;
    let items = load_reference_board_items_for_board(&conn, board_id)?;
    let tx = conn
        .transaction()
        .map_err(|error| format!("打开自动排列事务失败：{error}"))?;
    let total_area: f32 = items.iter().map(|item| item.width * item.height).sum();
    let max_item_width: f32 = items.iter().map(|item| item.width).fold(0.0, f32::max);
    let max_row_width = (total_area.sqrt() * 1.18).max(max_item_width);
    let mut x = 0.0;
    let mut y = 0.0;
    let mut row_height: f32 = 0.0;
    let gap = 24.0;
    for item in items {
        if x > 0.0 && x + item.width > max_row_width {
            x = 0.0;
            y += row_height + gap;
            row_height = 0.0;
        }
        tx.execute(
            "
            UPDATE reference_board_items
            SET x = ?1, y = ?2, rotation = 0
            WHERE id = ?3
            ",
            params![x, y, item.id],
        )
        .map_err(|error| format!("自动排列参考图失败：{error}"))?;
        x += item.width + gap;
        row_height = row_height.max(item.height);
    }
    tx.commit()
        .map_err(|error| format!("保存自动排列事务失败：{error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

fn open_database(database_path: &Path) -> Result<Connection, String> {
    if let Some(parent) = database_path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建图库目录失败：{error}"))?;
    }

    let conn =
        Connection::open(database_path).map_err(|error| format!("打开图库数据库失败：{error}"))?;
    migrate_database(&conn)?;
    Ok(conn)
}

fn migrate_database(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;

        CREATE TABLE IF NOT EXISTS folders (
          id INTEGER PRIMARY KEY,
          path TEXT NOT NULL UNIQUE,
          added_at INTEGER NOT NULL,
          last_scanned_at INTEGER,
          hidden INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS images (
          id TEXT PRIMARY KEY,
          path TEXT NOT NULL UNIQUE,
          file_name TEXT NOT NULL,
          ext TEXT NOT NULL,
          width INTEGER NOT NULL,
          height INTEGER NOT NULL,
          file_size INTEGER NOT NULL,
          modified_at INTEGER NOT NULL,
          imported_at INTEGER NOT NULL,
          folder_id INTEGER NOT NULL,
          missing INTEGER NOT NULL DEFAULT 0,
          trashed INTEGER NOT NULL DEFAULT 0,
          content_hash TEXT,
          source TEXT NOT NULL DEFAULT 'library',
          FOREIGN KEY(folder_id) REFERENCES folders(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_images_folder_id ON images(folder_id);
        CREATE INDEX IF NOT EXISTS idx_images_modified_at ON images(modified_at DESC);

        CREATE TABLE IF NOT EXISTS tags (
          id INTEGER PRIMARY KEY,
          name TEXT NOT NULL,
          namespace TEXT,
          source TEXT NOT NULL DEFAULT 'manual',
          UNIQUE(name, namespace, source)
        );

        CREATE TABLE IF NOT EXISTS image_tags (
          image_id TEXT NOT NULL,
          tag_id INTEGER NOT NULL,
          confidence REAL,
          source TEXT NOT NULL DEFAULT 'manual',
          PRIMARY KEY(image_id, tag_id, source),
          FOREIGN KEY(image_id) REFERENCES images(id) ON DELETE CASCADE,
          FOREIGN KEY(tag_id) REFERENCES tags(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS user_folders (
          id INTEGER PRIMARY KEY,
          parent_id INTEGER,
          name TEXT NOT NULL,
          sort_order INTEGER NOT NULL DEFAULT 0,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL,
          FOREIGN KEY(parent_id) REFERENCES user_folders(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_user_folders_parent_id ON user_folders(parent_id);

        CREATE TABLE IF NOT EXISTS image_user_folders (
          image_id TEXT NOT NULL,
          folder_id INTEGER NOT NULL,
          assigned_at INTEGER NOT NULL,
          PRIMARY KEY(image_id, folder_id),
          FOREIGN KEY(image_id) REFERENCES images(id) ON DELETE CASCADE,
          FOREIGN KEY(folder_id) REFERENCES user_folders(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_image_user_folders_folder_id ON image_user_folders(folder_id);

        CREATE TABLE IF NOT EXISTS ai_models (
          id INTEGER PRIMARY KEY,
          kind TEXT NOT NULL,
          name TEXT NOT NULL,
          version TEXT,
          metadata_json TEXT,
          UNIQUE(kind, name, version)
        );

        CREATE TABLE IF NOT EXISTS image_ai_state (
          image_id TEXT NOT NULL,
          model_id INTEGER NOT NULL,
          status TEXT NOT NULL,
          updated_at INTEGER NOT NULL,
          PRIMARY KEY(image_id, model_id),
          FOREIGN KEY(image_id) REFERENCES images(id) ON DELETE CASCADE,
          FOREIGN KEY(model_id) REFERENCES ai_models(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS tag_dictionary (
          tag_en TEXT PRIMARY KEY,
          tag_zh TEXT NOT NULL,
          updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS image_auto_tags (
          image_id TEXT NOT NULL,
          category TEXT NOT NULL,
          tag_en TEXT NOT NULL,
          tag_zh TEXT,
          confidence REAL NOT NULL,
          model_name TEXT NOT NULL,
          updated_at INTEGER NOT NULL,
          PRIMARY KEY(image_id, category, tag_en, model_name),
          FOREIGN KEY(image_id) REFERENCES images(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_image_auto_tags_image_id
          ON image_auto_tags(image_id, category, confidence DESC);

        CREATE TABLE IF NOT EXISTS known_image_tags (
          model_name TEXT NOT NULL,
          tag_en TEXT NOT NULL,
          tag_zh TEXT,
          image_count INTEGER NOT NULL DEFAULT 0,
          updated_at INTEGER NOT NULL,
          PRIMARY KEY(model_name, tag_en)
        );

        CREATE INDEX IF NOT EXISTS idx_known_image_tags_model_zh
          ON known_image_tags(model_name, tag_zh);
        CREATE INDEX IF NOT EXISTS idx_known_image_tags_model_en
          ON known_image_tags(model_name, tag_en);

        CREATE TABLE IF NOT EXISTS image_thumbnails (
          image_id TEXT PRIMARY KEY,
          thumb_path TEXT NOT NULL,
          source_modified_at INTEGER NOT NULL,
          source_file_size INTEGER NOT NULL,
          updated_at INTEGER NOT NULL,
          FOREIGN KEY(image_id) REFERENCES images(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_image_thumbnails_updated_at
          ON image_thumbnails(updated_at DESC);

        CREATE TABLE IF NOT EXISTS image_clip_embeddings (
          image_id TEXT NOT NULL,
          model_name TEXT NOT NULL,
          dim INTEGER NOT NULL,
          vector_json TEXT NOT NULL,
          source_modified_at INTEGER NOT NULL,
          source_file_size INTEGER NOT NULL,
          updated_at INTEGER NOT NULL,
          PRIMARY KEY(image_id, model_name),
          FOREIGN KEY(image_id) REFERENCES images(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_image_clip_embeddings_model_updated
          ON image_clip_embeddings(model_name, updated_at DESC);

        CREATE TRIGGER IF NOT EXISTS trg_image_auto_tags_insert_known
        AFTER INSERT ON image_auto_tags
        WHEN NOT EXISTS (
          SELECT 1
          FROM image_auto_tags t
          WHERE t.image_id = NEW.image_id
            AND t.model_name = NEW.model_name
            AND t.tag_en = NEW.tag_en
            AND t.rowid <> NEW.rowid
        )
        BEGIN
          INSERT INTO known_image_tags (model_name, tag_en, tag_zh, image_count, updated_at)
          VALUES (NEW.model_name, NEW.tag_en, NEW.tag_zh, 1, NEW.updated_at)
          ON CONFLICT(model_name, tag_en) DO UPDATE SET
            image_count = known_image_tags.image_count + 1,
            tag_zh = COALESCE(excluded.tag_zh, known_image_tags.tag_zh),
            updated_at = excluded.updated_at;
        END;

        CREATE TRIGGER IF NOT EXISTS trg_image_auto_tags_update_known
        AFTER UPDATE OF tag_zh, updated_at ON image_auto_tags
        BEGIN
          UPDATE known_image_tags
          SET
            tag_zh = COALESCE(NEW.tag_zh, known_image_tags.tag_zh),
            updated_at = NEW.updated_at
          WHERE model_name = NEW.model_name
            AND tag_en = NEW.tag_en;
        END;

        CREATE TRIGGER IF NOT EXISTS trg_image_auto_tags_delete_known
        AFTER DELETE ON image_auto_tags
        WHEN NOT EXISTS (
          SELECT 1
          FROM image_auto_tags t
          WHERE t.image_id = OLD.image_id
            AND t.model_name = OLD.model_name
            AND t.tag_en = OLD.tag_en
        )
        BEGIN
          UPDATE known_image_tags
          SET
            image_count = MAX(0, image_count - 1),
            updated_at = OLD.updated_at
          WHERE model_name = OLD.model_name
            AND tag_en = OLD.tag_en;
          DELETE FROM known_image_tags
          WHERE model_name = OLD.model_name
            AND tag_en = OLD.tag_en
            AND image_count <= 0;
        END;

        CREATE TABLE IF NOT EXISTS reference_board_folders (
          id INTEGER PRIMARY KEY,
          name TEXT NOT NULL,
          sort_order INTEGER NOT NULL DEFAULT 0,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS reference_boards (
          id INTEGER PRIMARY KEY,
          folder_id INTEGER,
          name TEXT NOT NULL,
          sort_order INTEGER NOT NULL DEFAULT 0,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL,
          FOREIGN KEY(folder_id) REFERENCES reference_board_folders(id) ON DELETE SET NULL
        );

        CREATE INDEX IF NOT EXISTS idx_reference_boards_folder_id
          ON reference_boards(folder_id);

        CREATE TABLE IF NOT EXISTS reference_board_items (
          id INTEGER PRIMARY KEY,
          board_id INTEGER NOT NULL,
          image_id TEXT NOT NULL,
          x REAL NOT NULL DEFAULT 0,
          y REAL NOT NULL DEFAULT 0,
          width REAL NOT NULL DEFAULT 220,
          height REAL NOT NULL DEFAULT 220,
          rotation REAL NOT NULL DEFAULT 0,
          z_index INTEGER NOT NULL DEFAULT 0,
          created_at INTEGER NOT NULL,
          FOREIGN KEY(board_id) REFERENCES reference_boards(id) ON DELETE CASCADE,
          FOREIGN KEY(image_id) REFERENCES images(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_reference_board_items_board_id
          ON reference_board_items(board_id, z_index);
        ",
    )
    .map_err(|error| format!("初始化图库数据库失败：{error}"))?;

    ensure_library_columns(conn)?;
    ensure_thumbnail_table(conn)?;
    ensure_user_folder_sort_order(conn)?;
    ensure_reference_board_items_allow_duplicates(conn)?;
    ensure_known_image_tags_bootstrap(conn)?;

    Ok(())
}

fn ensure_known_image_tags_bootstrap(conn: &Connection) -> Result<(), String> {
    let known_count = conn
        .query_row("SELECT COUNT(*) FROM known_image_tags", [], |row| row.get::<_, i64>(0))
        .map_err(|error| format!("Failed to read known_image_tags count: {error}"))?;
    if known_count > 0 {
        return Ok(());
    }
    let auto_tag_count = conn
        .query_row("SELECT COUNT(*) FROM image_auto_tags", [], |row| row.get::<_, i64>(0))
        .map_err(|error| format!("Failed to read image_auto_tags count: {error}"))?;
    if auto_tag_count == 0 {
        return Ok(());
    }
    rebuild_known_image_tags(conn)
}

fn rebuild_known_image_tags(conn: &Connection) -> Result<(), String> {
    let now = now_ms();
    conn.execute("DELETE FROM known_image_tags", [])
        .map_err(|error| format!("Failed to reset known image tags: {error}"))?;
    conn.execute(
        "
        INSERT INTO known_image_tags (model_name, tag_en, tag_zh, image_count, updated_at)
        SELECT
          image_auto_tags.model_name,
          image_auto_tags.tag_en,
          MAX(NULLIF(image_auto_tags.tag_zh, '')) AS tag_zh,
          COUNT(DISTINCT image_auto_tags.image_id) AS image_count,
          ?1
        FROM image_auto_tags
        JOIN images ON images.id = image_auto_tags.image_id
        WHERE images.source = 'library'
          AND COALESCE(images.trashed, 0) = 0
        GROUP BY image_auto_tags.model_name, image_auto_tags.tag_en
        ",
        params![now],
    )
    .map_err(|error| format!("Failed to rebuild known image tags: {error}"))?;
    Ok(())
}

fn ensure_library_columns(conn: &Connection) -> Result<(), String> {
    if !table_has_column(conn, "folders", "hidden")? {
        conn.execute(
            "ALTER TABLE folders ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|error| format!("升级图库文件夹隐藏字段失败：{error}"))?;
    }
    if !table_has_column(conn, "images", "source")? {
        conn.execute(
            "ALTER TABLE images ADD COLUMN source TEXT NOT NULL DEFAULT 'library'",
            [],
        )
        .map_err(|error| format!("升级图片来源字段失败：{error}"))?;
    }
    if !table_has_column(conn, "images", "trashed")? {
        conn.execute(
            "ALTER TABLE images ADD COLUMN trashed INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|error| format!("Failed to upgrade images.trashed column: {error}"))?;
    }
    Ok(())
}

fn ensure_thumbnail_table(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS image_thumbnails (
          image_id TEXT PRIMARY KEY,
          thumb_path TEXT NOT NULL,
          source_modified_at INTEGER NOT NULL,
          source_file_size INTEGER NOT NULL,
          updated_at INTEGER NOT NULL,
          FOREIGN KEY(image_id) REFERENCES images(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_image_thumbnails_updated_at
          ON image_thumbnails(updated_at DESC);
        ",
    )
    .map_err(|error| format!("Failed to ensure image_thumbnails table: {error}"))?;
    Ok(())
}

fn table_has_column(
    conn: &Connection,
    table_name: &str,
    column_name: &str,
) -> Result<bool, String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table_name})"))
        .map_err(|error| format!("读取表结构失败：{error}"))?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| format!("读取表结构失败：{error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取表结构失败：{error}"))?;
    Ok(columns.iter().any(|column| column == column_name))
}

fn ensure_reference_board_items_allow_duplicates(conn: &Connection) -> Result<(), String> {
    let has_unique_constraint = conn
        .prepare("PRAGMA index_list(reference_board_items)")
        .and_then(|mut stmt| {
            stmt.query_map([], |row| {
                Ok((row.get::<_, String>(1)?, row.get::<_, i64>(2)? != 0))
            })?
            .collect::<Result<Vec<_>, _>>()
        })
        .map_err(|error| format!("读取参考板图片索引失败：{error}"))?
        .iter()
        .any(|(_, is_unique)| *is_unique);

    if !has_unique_constraint {
        return Ok(());
    }

    conn.execute_batch(
        "
        ALTER TABLE reference_board_items RENAME TO reference_board_items_old;
        CREATE TABLE reference_board_items (
          id INTEGER PRIMARY KEY,
          board_id INTEGER NOT NULL,
          image_id TEXT NOT NULL,
          x REAL NOT NULL DEFAULT 0,
          y REAL NOT NULL DEFAULT 0,
          width REAL NOT NULL DEFAULT 220,
          height REAL NOT NULL DEFAULT 220,
          rotation REAL NOT NULL DEFAULT 0,
          z_index INTEGER NOT NULL DEFAULT 0,
          created_at INTEGER NOT NULL,
          FOREIGN KEY(board_id) REFERENCES reference_boards(id) ON DELETE CASCADE,
          FOREIGN KEY(image_id) REFERENCES images(id) ON DELETE CASCADE
        );
        INSERT INTO reference_board_items (
          id, board_id, image_id, x, y, width, height, rotation, z_index, created_at
        )
        SELECT id, board_id, image_id, x, y, width, height, rotation, z_index, created_at
        FROM reference_board_items_old;
        DROP TABLE reference_board_items_old;
        CREATE INDEX IF NOT EXISTS idx_reference_board_items_board_id
          ON reference_board_items(board_id, z_index);
        ",
    )
    .map_err(|error| format!("升级参考板图片表失败：{error}"))?;
    Ok(())
}

fn ensure_user_folder_sort_order(conn: &Connection) -> Result<(), String> {
    let has_sort_order = conn
        .prepare("PRAGMA table_info(user_folders)")
        .and_then(|mut stmt| {
            stmt.query_map([], |row| row.get::<_, String>(1))?
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(|error| format!("读取文件夹表结构失败：{error}"))?
        .iter()
        .any(|column| column == "sort_order");

    if !has_sort_order {
        conn.execute(
            "ALTER TABLE user_folders ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|error| format!("升级文件夹排序字段失败：{error}"))?;
    }

    Ok(())
}

fn load_store(conn: &Connection) -> Result<LibraryStore, String> {
    Ok(LibraryStore {
        folders: load_folders(conn)?,
        images: load_images(conn)?,
        user_folders: load_user_folders(conn)?,
        image_folders: load_image_folder_assignments(conn)?,
        reference_board_folders: load_reference_board_folders(conn)?,
        reference_boards: load_reference_boards(conn)?,
        reference_board_items: load_reference_board_items(conn)?,
    })
}

fn load_folders(conn: &Connection) -> Result<Vec<LibraryFolder>, String> {
    let mut stmt = conn
        .prepare(
            "
            SELECT id, path, added_at, last_scanned_at
            FROM folders
            WHERE COALESCE(hidden, 0) = 0
            ORDER BY added_at DESC, path ASC
            ",
        )
        .map_err(|error| format!("读取图库文件夹失败：{error}"))?;

    let folders = stmt
        .query_map([], |row| {
            Ok(LibraryFolder {
                id: row.get(0)?,
                path: row.get(1)?,
                added_at: row.get(2)?,
                last_scanned_at: row.get(3)?,
            })
        })
        .map_err(|error| format!("读取图库文件夹失败：{error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取图库文件夹失败：{error}"))?;

    Ok(folders)
}

fn load_images(conn: &Connection) -> Result<Vec<GalleryImage>, String> {
    let mut stmt = conn
        .prepare(
            "
            SELECT
              i.id, i.path, t.thumb_path, i.file_name, i.ext, i.width, i.height, i.file_size, i.modified_at,
              i.imported_at, i.folder_id, i.missing, i.trashed, i.source
            FROM images i
            LEFT JOIN image_thumbnails t ON t.image_id = i.id
            ORDER BY modified_at DESC, path ASC
            ",
        )
        .map_err(|error| format!("读取图片索引失败：{error}"))?;

    let images = stmt
        .query_map([], |row| {
            Ok(GalleryImage {
                id: row.get(0)?,
                path: row.get(1)?,
                thumbnail_path: row.get(2)?,
                file_name: row.get(3)?,
                ext: row.get(4)?,
                width: row.get(5)?,
                height: row.get(6)?,
                file_size: row.get(7)?,
                modified_at: row.get(8)?,
                imported_at: row.get(9)?,
                folder_id: row.get(10)?,
                missing: row.get::<_, i64>(11)? != 0,
                trashed: row.get::<_, i64>(12)? != 0,
                source: row.get(13)?,
            })
        })
        .map_err(|error| format!("读取图片索引失败：{error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取图片索引失败：{error}"))?;

    Ok(images)
}

fn load_user_folders(conn: &Connection) -> Result<Vec<UserFolder>, String> {
    let mut stmt = conn
        .prepare(
            "
            SELECT id, parent_id, name, sort_order, created_at, updated_at
            FROM user_folders
            ORDER BY parent_id IS NOT NULL, parent_id, sort_order, name COLLATE NOCASE, id
            ",
        )
        .map_err(|error| format!("读取文件夹失败：{error}"))?;

    let folders = stmt
        .query_map([], |row| {
            Ok(UserFolder {
                id: row.get(0)?,
                parent_id: row.get(1)?,
                name: row.get(2)?,
                sort_order: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })
        .map_err(|error| format!("读取文件夹失败：{error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取文件夹失败：{error}"))?;

    Ok(folders)
}

fn load_image_folder_assignments(conn: &Connection) -> Result<Vec<ImageFolderAssignment>, String> {
    let mut stmt = conn
        .prepare(
            "
            SELECT image_id, folder_id
            FROM image_user_folders
            ORDER BY assigned_at DESC
            ",
        )
        .map_err(|error| format!("读取图片文件夹关系失败：{error}"))?;

    let assignments = stmt
        .query_map([], |row| {
            Ok(ImageFolderAssignment {
                image_id: row.get(0)?,
                folder_id: row.get(1)?,
            })
        })
        .map_err(|error| format!("读取图片文件夹关系失败：{error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取图片文件夹关系失败：{error}"))?;

    Ok(assignments)
}

fn load_reference_board_folders(conn: &Connection) -> Result<Vec<ReferenceBoardFolder>, String> {
    let mut stmt = conn
        .prepare(
            "
            SELECT id, name, sort_order, created_at, updated_at
            FROM reference_board_folders
            ORDER BY sort_order, name COLLATE NOCASE, id
            ",
        )
        .map_err(|error| format!("读取参考板文件夹失败：{error}"))?;

    let folders = stmt
        .query_map([], |row| {
            Ok(ReferenceBoardFolder {
                id: row.get(0)?,
                name: row.get(1)?,
                sort_order: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })
        .map_err(|error| format!("读取参考板文件夹失败：{error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取参考板文件夹失败：{error}"))?;

    Ok(folders)
}

fn load_reference_boards(conn: &Connection) -> Result<Vec<ReferenceBoard>, String> {
    let mut stmt = conn
        .prepare(
            "
            SELECT id, folder_id, name, sort_order, created_at, updated_at
            FROM reference_boards
            ORDER BY folder_id IS NOT NULL, folder_id, sort_order, name COLLATE NOCASE, id
            ",
        )
        .map_err(|error| format!("读取参考板失败：{error}"))?;

    let boards = stmt
        .query_map([], |row| {
            Ok(ReferenceBoard {
                id: row.get(0)?,
                folder_id: row.get(1)?,
                name: row.get(2)?,
                sort_order: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })
        .map_err(|error| format!("读取参考板失败：{error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取参考板失败：{error}"))?;

    Ok(boards)
}

fn load_reference_board_items(conn: &Connection) -> Result<Vec<ReferenceBoardItem>, String> {
    let mut stmt = conn
        .prepare(
            "
            SELECT id, board_id, image_id, x, y, width, height, rotation, z_index, created_at
            FROM reference_board_items
            ORDER BY board_id, z_index, id
            ",
        )
        .map_err(|error| format!("读取参考板图片失败：{error}"))?;

    let items = stmt
        .query_map([], |row| {
            Ok(ReferenceBoardItem {
                id: row.get(0)?,
                board_id: row.get(1)?,
                image_id: row.get(2)?,
                x: row.get(3)?,
                y: row.get(4)?,
                width: row.get(5)?,
                height: row.get(6)?,
                rotation: row.get(7)?,
                z_index: row.get(8)?,
                created_at: row.get(9)?,
            })
        })
        .map_err(|error| format!("读取参考板图片失败：{error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取参考板图片失败：{error}"))?;

    Ok(items)
}

fn load_reference_board_item(
    conn: &Connection,
    item_id: i64,
) -> Result<ReferenceBoardItem, String> {
    conn.query_row(
        "
        SELECT id, board_id, image_id, x, y, width, height, rotation, z_index, created_at
        FROM reference_board_items
        WHERE id = ?1
        ",
        params![item_id],
        |row| {
            Ok(ReferenceBoardItem {
                id: row.get(0)?,
                board_id: row.get(1)?,
                image_id: row.get(2)?,
                x: row.get(3)?,
                y: row.get(4)?,
                width: row.get(5)?,
                height: row.get(6)?,
                rotation: row.get(7)?,
                z_index: row.get(8)?,
                created_at: row.get(9)?,
            })
        },
    )
    .optional()
    .map_err(|error| format!("读取参考图失败：{error}"))?
    .ok_or_else(|| "找不到参考图".to_string())
}

fn load_reference_board_items_for_board(
    conn: &Connection,
    board_id: i64,
) -> Result<Vec<ReferenceBoardItem>, String> {
    let mut stmt = conn
        .prepare(
            "
            SELECT id, board_id, image_id, x, y, width, height, rotation, z_index, created_at
            FROM reference_board_items
            WHERE board_id = ?1
            ORDER BY z_index, id
            ",
        )
        .map_err(|error| format!("读取参考板图片失败：{error}"))?;
    let items = stmt
        .query_map(params![board_id], |row| {
            Ok(ReferenceBoardItem {
                id: row.get(0)?,
                board_id: row.get(1)?,
                image_id: row.get(2)?,
                x: row.get(3)?,
                y: row.get(4)?,
                width: row.get(5)?,
                height: row.get(6)?,
                rotation: row.get(7)?,
                z_index: row.get(8)?,
                created_at: row.get(9)?,
            })
        })
        .map_err(|error| format!("读取参考板图片失败：{error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取参考板图片失败：{error}"))?;
    Ok(items)
}

fn load_image_record(conn: &Connection, image_id: &str) -> Result<GalleryImage, String> {
    conn.query_row(
        "
        SELECT
          i.id, i.path, t.thumb_path, i.file_name, i.ext, i.width, i.height, i.file_size, i.modified_at,
          i.imported_at, i.folder_id, i.missing, i.trashed, i.source
        FROM images i
        LEFT JOIN image_thumbnails t ON t.image_id = i.id
        WHERE i.id = ?1
        ",
        params![image_id],
        |row| {
            Ok(GalleryImage {
                id: row.get(0)?,
                path: row.get(1)?,
                thumbnail_path: row.get(2)?,
                file_name: row.get(3)?,
                ext: row.get(4)?,
                width: row.get(5)?,
                height: row.get(6)?,
                file_size: row.get(7)?,
                modified_at: row.get(8)?,
                imported_at: row.get(9)?,
                folder_id: row.get(10)?,
                missing: row.get::<_, i64>(11)? != 0,
                trashed: row.get::<_, i64>(12)? != 0,
                source: row.get(13)?,
            })
        },
    )
    .optional()
    .map_err(|error| format!("读取图片索引失败：{error}"))?
    .ok_or_else(|| "找不到图片索引".to_string())
}

fn next_reference_board_z_index(conn: &Connection, board_id: i64) -> Result<i64, String> {
    conn.query_row(
        "
        SELECT COALESCE(MAX(z_index), -1) + 1
        FROM reference_board_items
        WHERE board_id = ?1
        ",
        params![board_id],
        |row| row.get(0),
    )
    .map_err(|error| format!("读取参考板层级失败：{error}"))
}

fn reference_board_folder_id(conn: &Connection, board_id: i64) -> Result<Option<i64>, String> {
    conn.query_row(
        "SELECT folder_id FROM reference_boards WHERE id = ?1",
        params![board_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(|error| format!("读取参考板文件夹失败：{error}"))?
    .ok_or_else(|| "找不到参考板".to_string())
}

fn load_reference_board_sibling_ids(
    conn: &Connection,
    folder_id: Option<i64>,
) -> Result<Vec<i64>, String> {
    let mut stmt = conn
        .prepare(
            "
            SELECT id
            FROM reference_boards
            WHERE folder_id IS ?1
            ORDER BY sort_order, name COLLATE NOCASE, id
            ",
        )
        .map_err(|error| format!("读取参考板排序失败：{error}"))?;
    let board_ids = stmt
        .query_map(params![folder_id], |row| row.get::<_, i64>(0))
        .map_err(|error| format!("读取参考板排序失败：{error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取参考板排序失败：{error}"))?;
    Ok(board_ids)
}

fn load_reference_board_folder_ids(conn: &Connection) -> Result<Vec<i64>, String> {
    let mut stmt = conn
        .prepare(
            "
            SELECT id
            FROM reference_board_folders
            ORDER BY sort_order, name COLLATE NOCASE, id
            ",
        )
        .map_err(|error| format!("读取参考板文件夹排序失败：{error}"))?;
    let folder_ids = stmt
        .query_map([], |row| row.get::<_, i64>(0))
        .map_err(|error| format!("读取参考板文件夹排序失败：{error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取参考板文件夹排序失败：{error}"))?;
    Ok(folder_ids)
}

fn copy_image_to_destination(source: &str, destination: &Path) -> Result<(), String> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建导出目录失败：{error}"))?;
    }
    fs::copy(source, destination).map_err(|error| format!("导出参考图失败：{error}"))?;
    Ok(())
}

fn unique_file_path(folder_path: &Path, file_name: &str) -> PathBuf {
    let base_name = Path::new(file_name)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| "reference".to_string());
    let extension = Path::new(file_name)
        .extension()
        .map(|extension| extension.to_string_lossy().to_string())
        .unwrap_or_else(|| "png".to_string());
    let mut candidate = folder_path.join(format!("{base_name}.{extension}"));
    let mut suffix = 1;
    while candidate.exists() {
        candidate = folder_path.join(format!("{base_name}-{suffix}.{extension}"));
        suffix += 1;
    }
    candidate
}

fn cleanup_orphan_reference_images(conn: &Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare(
            "
            SELECT id, path
            FROM images
            WHERE source = 'reference'
              AND NOT EXISTS (
                SELECT 1 FROM reference_board_items WHERE image_id = images.id
              )
            ",
        )
        .map_err(|error| format!("读取临时参考图失败：{error}"))?;
    let images = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| format!("读取临时参考图失败：{error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取临时参考图失败：{error}"))?;
    drop(stmt);
    for (image_id, path) in images {
        conn.execute("DELETE FROM images WHERE id = ?1", params![image_id])
            .map_err(|error| format!("删除临时参考图索引失败：{error}"))?;
        let _ = fs::remove_file(path);
    }
    Ok(())
}

fn cleanup_missing_library_images_count(conn: &Connection) -> Result<i64, String> {
    let mut stmt = conn
        .prepare(
            "
            SELECT i.id, i.path, f.path
            FROM images i
            LEFT JOIN folders f ON f.id = i.folder_id
            WHERE i.source = 'library'
            ",
        )
        .map_err(|error| format!("读取图库图片索引失败：{error}"))?;
    let images = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })
        .map_err(|error| format!("读取图库图片索引失败：{error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取图库图片索引失败：{error}"))?;
    drop(stmt);
    let mut removed = 0i64;
    let mut root_online_cache = HashMap::<String, bool>::new();

    for (image_id, path, folder_path) in images {
        let Some(folder_path) = folder_path else {
            continue;
        };
        let root_online = *root_online_cache
            .entry(folder_path.clone())
            .or_insert_with(|| {
                let normalized = normalize_existing_or_stored_folder_path(&folder_path);
                Path::new(&normalized).is_dir()
            });
        if !root_online {
            continue;
        }
        if !Path::new(&path).exists() {
            conn.execute("DELETE FROM images WHERE id = ?1", params![image_id])
                .map_err(|error| format!("清理缺失图片索引失败：{error}"))?;
            removed += 1;
        }
    }
    Ok(removed)
}

fn cleanup_missing_library_images_batched(database_path: &Path, batch_size: usize) -> Result<i64, String> {
    let conn = open_database(database_path)?;
    let mut removed_total = 0i64;
    let mut last_rowid = 0i64;
    let limit = (batch_size.max(32)) as i64;
    let mut root_online_cache = HashMap::<String, bool>::new();

    loop {
        let mut stmt = conn
            .prepare(
                "
                SELECT i.rowid, i.id, i.path, f.path
                FROM images i
                LEFT JOIN folders f ON f.id = i.folder_id
                WHERE i.source = 'library' AND i.rowid > ?1
                ORDER BY i.rowid ASC
                LIMIT ?2
                ",
            )
            .map_err(|error| format!("Failed to load library image batch for startup cleanup: {error}"))?;

        let rows = stmt
            .query_map(params![last_rowid, limit], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })
            .map_err(|error| format!("Failed to load library image batch for startup cleanup: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Failed to load library image batch for startup cleanup: {error}"))?;
        drop(stmt);

        if rows.is_empty() {
            break;
        }

        let mut missing_ids = Vec::<String>::new();
        let mut next_rowid = last_rowid;
        for (rowid, image_id, path, folder_path) in rows {
            next_rowid = rowid;
            let Some(folder_path) = folder_path else {
                continue;
            };
            let root_online = *root_online_cache
                .entry(folder_path.clone())
                .or_insert_with(|| {
                    let normalized = normalize_existing_or_stored_folder_path(&folder_path);
                    Path::new(&normalized).is_dir()
                });
            if !root_online {
                continue;
            }
            if !Path::new(&path).exists() {
                missing_ids.push(image_id);
            }
        }
        last_rowid = next_rowid;

        for image_id in missing_ids {
            conn.execute("DELETE FROM images WHERE id = ?1", params![image_id])
                .map_err(|error| format!("Failed to delete missing image during startup cleanup: {error}"))?;
            removed_total += 1;
        }

        thread::sleep(std::time::Duration::from_millis(2));
    }

    Ok(removed_total)
}

fn scan_all_folders_and_collect_new_images(
    database_path: &Path,
    progress: &Arc<Mutex<BackgroundScanProgress>>,
) -> Result<ScanCollectResult, String> {
    let conn = open_database(database_path)?;
    let scanned_at = now_ms();
    let mut seen_paths = HashSet::new();
    let removed_missing_images = cleanup_missing_library_images_count(&conn)?;
    set_scan_progress_removed_missing_images(progress, removed_missing_images);
    let mut known_paths = load_known_paths(&conn)?;
    let mut existing_meta = load_existing_library_image_meta(&conn)?;

    let folders = conn
        .prepare(
            "
            SELECT id, path
            FROM folders
            WHERE COALESCE(hidden, 0) = 0
            ORDER BY added_at ASC, id ASC
            ",
        )
        .and_then(|mut stmt| {
            stmt.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))?
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(|error| format!("Failed to load folders for scan: {error}"))?;
    set_scan_progress_total_folders(progress, folders.len() as i64);

    let mut new_image_ids = Vec::<String>::new();
    let mut updated_images = 0i64;
    let mut skipped_images = 0i64;
    let mut scanned_folders = 0i64;

    for (_, folder_path) in folders {
        let folder_path = normalize_existing_or_stored_folder_path(&folder_path);
        if !Path::new(&folder_path).is_dir() {
            scanned_folders += 1;
            set_scan_progress_scanned_folders(progress, scanned_folders);
            continue;
        }
        let folder_id = upsert_folder(&conn, &folder_path, scanned_at)?;
        let found = scan_images(Path::new(&folder_path), scanned_at, &mut seen_paths, &existing_meta);
        let found_count = found.len() as i64;
        for image in found {
            let previous = existing_meta.get(&image.path).copied();
            let is_new = !known_paths.contains(&image.path);
            upsert_image(&conn, folder_id, &image)?;
            existing_meta.insert(
                image.path.clone(),
                ExistingImageMeta {
                    width: image.width,
                    height: image.height,
                    file_size: image.file_size,
                    modified_at: image.modified_at,
                },
            );
            if is_new {
                known_paths.insert(image.path.clone());
                new_image_ids.push(image.path);
            } else if let Some(meta) = previous {
                if meta.modified_at != image.modified_at
                    || meta.file_size != image.file_size
                    || meta.width != image.width
                    || meta.height != image.height
                {
                    updated_images += 1;
                } else {
                    skipped_images += 1;
                }
            } else {
                updated_images += 1;
            }
        }
        scanned_folders += 1;
        set_scan_progress_new_images(progress, new_image_ids.len() as i64);
        set_scan_progress_updated_images(progress, updated_images);
        set_scan_progress_skipped_images(progress, skipped_images);
        set_scan_progress_scanned_folders(progress, scanned_folders);
        if found_count > 0 {
            eprintln!("[wd-scan] folder scanned, new images: {found_count}");
        }
    }

    let mut tag_queue_image_ids = collect_pending_tag_image_ids(&conn)?;
    tag_queue_image_ids.sort();
    tag_queue_image_ids.dedup();
    set_scan_progress_phase(progress, "tagging");
    set_scan_progress_queued_images(progress, tag_queue_image_ids.len() as i64);

    Ok(ScanCollectResult { tag_queue_image_ids })
}

fn tag_images_with_wd_model(
    database_path: &Path,
    image_ids: &[String],
    progress: &Arc<Mutex<BackgroundScanProgress>>,
) -> Result<(), String> {
    if image_ids.is_empty() {
        return Ok(());
    }

    let mut conn = open_database(database_path)?;
    let dictionary = load_cn_tag_dictionary_map()?;
    let model_root = resolve_wd_tagger_model_dir(None)?;
    let model_path = model_root.join("model.onnx");
    let tags_path = model_root.join("selected_tags.csv");
    let script_path = resolve_wd_tagger_script_path()?;

    if !model_path.is_file() || !tags_path.is_file() || !script_path.is_file() {
        let err = "Model files or wd_tagger_test.py not found; skip tagging".to_string();
        set_scan_progress_error(progress, &err);
        push_scan_progress_recent_error(progress, &err);
        eprintln!("[wd-tag] {err}");
        return Ok(());
    }

    eprintln!("[wd-tag] queue size: {}", image_ids.len());
    for image_id in image_ids {
        if !Path::new(image_id).is_file() {
            increment_scan_progress_failed(progress);
            continue;
        }
        match run_wd_tagger_script(
            image_id,
            image_id,
            &model_path,
            &tags_path,
            &script_path,
            0.35,
            0.85,
        ) {
            Ok(result) => {
                save_wd_tagger_result(&mut conn, image_id, &result, &dictionary)?;
                increment_scan_progress_tagged(progress);
            }
            Err(error) => {
                eprintln!("[wd-tag] {error}");
                increment_scan_progress_failed(progress);
                set_scan_progress_error(progress, &error);
                push_scan_progress_recent_error(progress, &error);
            }
        }
    }

    Ok(())
}

fn run_wd_tagger_script(
    image_id: &str,
    image_path: &str,
    model_path: &Path,
    tags_path: &Path,
    script_path: &Path,
    general_threshold: f32,
    character_threshold: f32,
) -> Result<WdTaggerTestResult, String> {
    let output = Command::new("python")
        .arg(script_path)
        .arg("--image")
        .arg(image_path)
        .arg("--model")
        .arg(model_path)
        .arg("--tags")
        .arg(tags_path)
        .arg("--general-threshold")
        .arg(general_threshold.to_string())
        .arg("--character-threshold")
        .arg(character_threshold.to_string())
        .arg("--image-id")
        .arg(image_id)
        .output()
        .map_err(|error| format!("Failed to run wd tagger script: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let detail = if stderr.is_empty() {
            "unknown error".to_string()
        } else {
            stderr
        };
        return Err(format!("WD tagger script failed: {detail}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Err("WD tagger script returned empty output".to_string());
    }
    serde_json::from_str(&stdout).map_err(|error| format!("Failed to parse wd tagger output: {error}"))
}

fn save_wd_tagger_result(
    conn: &mut Connection,
    image_id: &str,
    result: &WdTaggerTestResult,
    dictionary: &HashMap<String, String>,
) -> Result<(), String> {
    let now = now_ms();
    let model_name = WD_TAGGER_MODEL_NAME;
    let tx = conn
        .transaction()
        .map_err(|error| format!("Failed to open tag save transaction: {error}"))?;

    tx.execute(
        "DELETE FROM image_auto_tags WHERE image_id = ?1 AND model_name = ?2",
        params![image_id, model_name],
    )
    .map_err(|error| format!("Failed to clear old image tags: {error}"))?;

    for tag in &result.ratings {
        upsert_image_auto_tag(&tx, image_id, "rating", &tag.tag, tag.score, dictionary, model_name, now)?;
    }
    for tag in &result.character_tags {
        upsert_image_auto_tag(
            &tx,
            image_id,
            "character",
            &tag.tag,
            tag.score,
            dictionary,
            model_name,
            now,
        )?;
    }
    for tag in &result.general_tags {
        upsert_image_auto_tag(
            &tx,
            image_id,
            "general",
            &tag.tag,
            tag.score,
            dictionary,
            model_name,
            now,
        )?;
    }

    tx.commit()
        .map_err(|error| format!("Failed to commit image tags: {error}"))?;
    Ok(())
}

fn upsert_image_auto_tag(
    conn: &Connection,
    image_id: &str,
    category: &str,
    tag_en: &str,
    confidence: f32,
    dictionary: &HashMap<String, String>,
    model_name: &str,
    updated_at: i64,
) -> Result<(), String> {
    let tag_zh = lookup_cn_tag(dictionary, tag_en);
    if let Some(zh) = &tag_zh {
        conn.execute(
            "
            INSERT INTO tag_dictionary (tag_en, tag_zh, updated_at)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(tag_en) DO UPDATE SET
              tag_zh = excluded.tag_zh,
              updated_at = excluded.updated_at
            ",
            params![tag_en, zh, updated_at],
        )
        .map_err(|error| format!("Failed to upsert tag dictionary: {error}"))?;
    }

    conn.execute(
        "
        INSERT INTO image_auto_tags (
          image_id, category, tag_en, tag_zh, confidence, model_name, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(image_id, category, tag_en, model_name) DO UPDATE SET
          tag_zh = excluded.tag_zh,
          confidence = excluded.confidence,
          updated_at = excluded.updated_at
        ",
        params![image_id, category, tag_en, tag_zh, confidence, model_name, updated_at],
    )
    .map_err(|error| format!("Failed to save image auto tag: {error}"))?;
    Ok(())
}

fn collect_pending_tag_image_ids(conn: &Connection) -> Result<Vec<String>, String> {
    let model_name = WD_TAGGER_MODEL_NAME;
    conn.prepare(
        "
        SELECT images.id
        FROM images
        WHERE images.source = 'library'
          AND COALESCE(images.trashed, 0) = 0
          AND NOT EXISTS (
            SELECT 1
            FROM image_auto_tags
            WHERE image_auto_tags.image_id = images.id
              AND image_auto_tags.model_name = ?1
          )
        ORDER BY images.imported_at DESC, images.id ASC
        ",
    )
    .and_then(|mut stmt| {
        stmt.query_map(params![model_name], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()
    })
    .map_err(|error| format!("Failed to collect pending tag images: {error}"))
}

fn generate_thumbnails_once(
    database_path: &Path,
    progress: &Arc<Mutex<ThumbnailGenerationProgress>>,
    pause_requested: &Arc<Mutex<bool>>,
    stop_requested: &Arc<Mutex<bool>>,
) -> Result<i64, String> {
    set_thumbnail_progress_phase(progress, "collecting");
    let conn = open_database(database_path)?;
    let mut stmt = conn
        .prepare(
            "
            SELECT
              i.id,
              i.path,
              i.modified_at,
              i.file_size,
              t.thumb_path,
              t.source_modified_at,
              t.source_file_size
            FROM images i
            LEFT JOIN image_thumbnails t ON t.image_id = i.id
            WHERE i.source = 'library'
              AND COALESCE(i.trashed, 0) = 0
            ORDER BY i.modified_at DESC, i.id ASC
            ",
        )
        .map_err(|error| format!("Failed to load thumbnail candidates: {error}"))?;
    let candidates = stmt
        .query_map([], |row| {
            Ok(ThumbnailCandidate {
                image_id: row.get(0)?,
                image_path: row.get(1)?,
                modified_at: row.get(2)?,
                file_size: row.get(3)?,
                current_thumb_path: row.get(4)?,
                current_source_modified_at: row.get(5)?,
                current_source_file_size: row.get(6)?,
            })
        })
        .map_err(|error| format!("Failed to load thumbnail candidates: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to load thumbnail candidates: {error}"))?;
    drop(stmt);

    set_thumbnail_progress_total(progress, candidates.len() as i64);
    set_thumbnail_progress_phase(progress, "generating");

    let thumb_root = ensure_thumbnail_root_dir(database_path)?;
    let mut generated = 0i64;
    let mut skipped = 0i64;
    let mut failed = 0i64;
    let mut processed = 0i64;
    let mut stop_now = false;

    for candidate in candidates {
        if thumbnail_stop_requested(stop_requested) {
            stop_now = true;
            break;
        }
        wait_for_thumbnail_resume(progress, pause_requested, stop_requested);
        if thumbnail_stop_requested(stop_requested) {
            stop_now = true;
            break;
        }

        if !Path::new(&candidate.image_path).is_file() {
            if let Err(error) = clear_thumbnail_for_missing_image(&conn, &candidate.image_id) {
                set_thumbnail_progress_error(progress, &error);
                push_thumbnail_progress_recent_error(progress, &error);
                failed += 1;
            } else {
                skipped += 1;
            }
            processed += 1;
            set_thumbnail_progress_counts(progress, processed, generated, skipped, failed);
            continue;
        }

        if thumbnail_up_to_date(&candidate) {
            skipped += 1;
            processed += 1;
            set_thumbnail_progress_counts(progress, processed, generated, skipped, failed);
            continue;
        }

        match generate_single_thumbnail(&candidate, &thumb_root, THUMBNAIL_LONG_EDGE, THUMBNAIL_WEBP_QUALITY) {
            Ok(next_thumb_path) => {
                if let Err(error) = upsert_thumbnail_record(
                    &conn,
                    &candidate.image_id,
                    &next_thumb_path,
                    candidate.modified_at,
                    candidate.file_size,
                ) {
                    set_thumbnail_progress_error(progress, &error);
                    push_thumbnail_progress_recent_error(progress, &error);
                    failed += 1;
                } else {
                    generated += 1;
                    if let Some(previous_thumb) = candidate.current_thumb_path {
                        if previous_thumb != next_thumb_path {
                            let _ = fs::remove_file(previous_thumb);
                        }
                    }
                }
            }
            Err(error) => {
                set_thumbnail_progress_error(progress, &error);
                push_thumbnail_progress_recent_error(progress, &error);
                failed += 1;
            }
        }

        processed += 1;
        set_thumbnail_progress_counts(progress, processed, generated, skipped, failed);
    }

    cleanup_orphan_thumbnail_files(&conn, &thumb_root)?;
    if stop_now {
        set_thumbnail_progress_phase(progress, "idle");
    }
    Ok(generated)
}

fn generate_natural_language_embeddings_once(
    database_path: &Path,
    progress: &Arc<Mutex<NaturalLanguageScanProgress>>,
) -> Result<i64, String> {
    set_natural_language_scan_progress_phase(progress, "collecting");
    let conn = open_database(database_path)?;
    let mut stmt = conn
        .prepare(
            "
            SELECT
              i.id,
              i.path,
              i.modified_at,
              i.file_size,
              e.source_modified_at,
              e.source_file_size
            FROM images i
            LEFT JOIN image_clip_embeddings e
              ON e.image_id = i.id
             AND e.model_name = ?1
            WHERE i.source = 'library'
              AND COALESCE(i.trashed, 0) = 0
            ORDER BY i.modified_at DESC, i.id ASC
            ",
        )
        .map_err(|error| format!("Failed to load natural language scan candidates: {error}"))?;
    let candidates = stmt
        .query_map(params![CHINESE_CLIP_MODEL_NAME], |row| {
            Ok(NaturalLanguageEmbeddingCandidate {
                image_id: row.get(0)?,
                image_path: row.get(1)?,
                modified_at: row.get(2)?,
                file_size: row.get(3)?,
                current_source_modified_at: row.get(4)?,
                current_source_file_size: row.get(5)?,
            })
        })
        .map_err(|error| format!("Failed to load natural language scan candidates: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to load natural language scan candidates: {error}"))?;
    drop(stmt);

    set_natural_language_scan_progress_total(progress, candidates.len() as i64);
    set_natural_language_scan_progress_phase(progress, "generating");

    if candidates.is_empty() {
        set_natural_language_scan_progress_phase(progress, "idle");
        return Ok(0);
    }

    let model_root = resolve_chinese_clip_model_dir(None)?;
    let script_path = resolve_chinese_clip_embed_script_path()?;

    let mut generated = 0i64;
    let mut skipped = 0i64;
    let mut failed = 0i64;
    let mut processed = 0i64;

    for candidate in candidates {
        if !Path::new(&candidate.image_path).is_file() {
            clear_image_clip_embedding(&conn, &candidate.image_id)?;
            skipped += 1;
            processed += 1;
            set_natural_language_scan_progress_counts(progress, processed, generated, skipped, failed);
            continue;
        }

        if natural_language_embedding_up_to_date(&candidate) {
            skipped += 1;
            processed += 1;
            set_natural_language_scan_progress_counts(progress, processed, generated, skipped, failed);
            continue;
        }

        match run_chinese_clip_image_embedding(&candidate.image_path, &model_root, &script_path) {
            Ok(vector) => {
                if let Err(error) = upsert_image_clip_embedding(
                    &conn,
                    &candidate.image_id,
                    &vector,
                    candidate.modified_at,
                    candidate.file_size,
                ) {
                    failed += 1;
                    set_natural_language_scan_progress_error(progress, &error);
                    push_natural_language_scan_recent_error(progress, &error);
                } else {
                    generated += 1;
                }
            }
            Err(error) => {
                failed += 1;
                set_natural_language_scan_progress_error(progress, &error);
                push_natural_language_scan_recent_error(progress, &error);
                eprintln!("[clip-scan] {}", error);
            }
        }
        processed += 1;
        set_natural_language_scan_progress_counts(progress, processed, generated, skipped, failed);
    }

    set_natural_language_scan_progress_phase(progress, "idle");
    Ok(generated)
}

fn natural_language_embedding_up_to_date(candidate: &NaturalLanguageEmbeddingCandidate) -> bool {
    matches!(
        (
            candidate.current_source_modified_at,
            candidate.current_source_file_size,
        ),
        (Some(source_modified_at), Some(source_file_size))
            if source_modified_at == candidate.modified_at && source_file_size == candidate.file_size
    )
}

fn clear_image_clip_embedding(conn: &Connection, image_id: &str) -> Result<(), String> {
    conn.execute(
        "DELETE FROM image_clip_embeddings WHERE image_id = ?1 AND model_name = ?2",
        params![image_id, CHINESE_CLIP_MODEL_NAME],
    )
    .map_err(|error| format!("Failed to clear stale clip embedding: {error}"))?;
    Ok(())
}

fn upsert_image_clip_embedding(
    conn: &Connection,
    image_id: &str,
    vector: &[f32],
    source_modified_at: i64,
    source_file_size: i64,
) -> Result<(), String> {
    if vector.is_empty() {
        return Err("CLIP embedding is empty".to_string());
    }
    let dim = i64::try_from(vector.len()).map_err(|_| "CLIP embedding dimension overflow".to_string())?;
    let vector_json =
        serde_json::to_string(vector).map_err(|error| format!("Failed to encode CLIP embedding: {error}"))?;
    let now = now_ms();
    conn.execute(
        "
        INSERT INTO image_clip_embeddings (
          image_id, model_name, dim, vector_json, source_modified_at, source_file_size, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(image_id, model_name) DO UPDATE SET
          dim = excluded.dim,
          vector_json = excluded.vector_json,
          source_modified_at = excluded.source_modified_at,
          source_file_size = excluded.source_file_size,
          updated_at = excluded.updated_at
        ",
        params![
            image_id,
            CHINESE_CLIP_MODEL_NAME,
            dim,
            vector_json,
            source_modified_at,
            source_file_size,
            now
        ],
    )
    .map_err(|error| format!("Failed to save CLIP embedding: {error}"))?;
    Ok(())
}

fn run_chinese_clip_image_embedding(
    image_path: &str,
    model_root: &Path,
    script_path: &Path,
) -> Result<Vec<f32>, String> {
    let output = Command::new("python")
        .arg("-X")
        .arg("utf8")
        .arg(script_path)
        .arg("--model-dir")
        .arg(model_root)
        .arg("--mode")
        .arg("image")
        .arg("--image")
        .arg(image_path)
        .arg("--provider")
        .arg("cpu")
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .output()
        .map_err(|error| format!("Failed to run Chinese-CLIP image embedding script: {error}"))?;
    parse_clip_embedding_script_output(output, "image")
}

fn run_chinese_clip_text_embedding(
    text: &str,
    model_root: &Path,
    script_path: &Path,
) -> Result<Vec<f32>, String> {
    let output = Command::new("python")
        .arg("-X")
        .arg("utf8")
        .arg(script_path)
        .arg("--model-dir")
        .arg(model_root)
        .arg("--mode")
        .arg("text")
        .arg("--text")
        .arg(text)
        .arg("--provider")
        .arg("cpu")
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .output()
        .map_err(|error| format!("Failed to run Chinese-CLIP text embedding script: {error}"))?;
    parse_clip_embedding_script_output(output, "text")
}

fn parse_clip_embedding_script_output(
    output: std::process::Output,
    mode: &str,
) -> Result<Vec<f32>, String> {
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let detail = if stderr.is_empty() {
            "unknown error".to_string()
        } else {
            stderr
        };
        return Err(format!(
            "Chinese-CLIP {mode} embedding failed: {detail}. If dependency missing, run: pip install onnxruntime numpy pillow"
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Err(format!("Chinese-CLIP {mode} embedding returned empty output"));
    }

    let value: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|error| format!("Failed to parse CLIP embedding output: {error}"))?;
    let array = value
        .get("embedding")
        .and_then(|item| item.as_array())
        .ok_or_else(|| "CLIP embedding output missing embedding array".to_string())?;
    let mut vector = Vec::<f32>::with_capacity(array.len());
    for item in array {
        let number = item
            .as_f64()
            .ok_or_else(|| "CLIP embedding output contains non-numeric value".to_string())?;
        if !number.is_finite() {
            return Err("CLIP embedding output contains invalid number".to_string());
        }
        vector.push(number as f32);
    }
    if vector.is_empty() {
        return Err("CLIP embedding output is empty".to_string());
    }
    Ok(normalize_vector(&vector))
}

fn normalize_vector(input: &[f32]) -> Vec<f32> {
    let mut sum = 0f64;
    for value in input {
        let v = *value as f64;
        sum += v * v;
    }
    if sum <= 1e-18 {
        return input.to_vec();
    }
    let inv = 1.0f64 / sum.sqrt();
    input.iter().map(|value| (*value as f64 * inv) as f32).collect()
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> Option<f32> {
    if left.len() != right.len() || left.is_empty() {
        return None;
    }
    let mut dot = 0f64;
    let mut left_norm = 0f64;
    let mut right_norm = 0f64;
    for (lv, rv) in left.iter().zip(right.iter()) {
        let l = *lv as f64;
        let r = *rv as f64;
        dot += l * r;
        left_norm += l * l;
        right_norm += r * r;
    }
    if left_norm <= 1e-18 || right_norm <= 1e-18 {
        return None;
    }
    Some((dot / (left_norm.sqrt() * right_norm.sqrt())) as f32)
}

fn ensure_thumbnail_root_dir(database_path: &Path) -> Result<PathBuf, String> {
    let root = database_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("thumbs")
        .join("library");
    fs::create_dir_all(&root)
        .map_err(|error| format!("Failed to create thumbnail cache directory: {error}"))?;
    Ok(root)
}

fn clear_thumbnail_for_missing_image(conn: &Connection, image_id: &str) -> Result<(), String> {
    let previous_thumb: Option<String> = conn
        .query_row(
            "SELECT thumb_path FROM image_thumbnails WHERE image_id = ?1",
            params![image_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("Failed to query stale thumbnail: {error}"))?
        .flatten();

    conn.execute("DELETE FROM image_thumbnails WHERE image_id = ?1", params![image_id])
        .map_err(|error| format!("Failed to clear stale thumbnail: {error}"))?;

    if let Some(path) = previous_thumb {
        let _ = fs::remove_file(path);
    }
    Ok(())
}

fn clear_thumbnail_cache_storage(conn: &Connection, database_path: &Path) -> Result<(), String> {
    conn.execute("DELETE FROM image_thumbnails", [])
        .map_err(|error| format!("Failed to clear thumbnail table: {error}"))?;
    let thumb_root = ensure_thumbnail_root_dir(database_path)?;
    if thumb_root.exists() {
        fs::remove_dir_all(&thumb_root)
            .map_err(|error| format!("Failed to remove thumbnail cache directory: {error}"))?;
    }
    fs::create_dir_all(&thumb_root)
        .map_err(|error| format!("Failed to recreate thumbnail cache directory: {error}"))?;
    Ok(())
}

fn cleanup_orphan_thumbnail_files(conn: &Connection, thumb_root: &Path) -> Result<(), String> {
    let mut stmt = conn
        .prepare("SELECT image_id, thumb_path FROM image_thumbnails")
        .map_err(|error| format!("Failed to query thumbnail paths: {error}"))?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(|error| format!("Failed to query thumbnail paths: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to query thumbnail paths: {error}"))?;
    drop(stmt);

    let mut referenced_set = HashSet::<String>::new();
    let mut stale_image_ids = Vec::<String>::new();
    for (image_id, thumb_path) in rows {
        if Path::new(&thumb_path).is_file() {
            referenced_set.insert(thumb_path);
        } else {
            stale_image_ids.push(image_id);
        }
    }
    for image_id in stale_image_ids {
        conn.execute(
            "DELETE FROM image_thumbnails WHERE image_id = ?1",
            params![image_id],
        )
        .map_err(|error| format!("Failed to remove stale thumbnail record: {error}"))?;
    }

    if !thumb_root.exists() {
        return Ok(());
    }

    for entry in WalkDir::new(thumb_root)
        .min_depth(1)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path_str = entry.path().to_string_lossy().to_string();
        if !referenced_set.contains(&path_str) {
            let _ = fs::remove_file(entry.path());
        }
    }
    Ok(())
}

fn thumbnail_stop_requested(stop_requested: &Arc<Mutex<bool>>) -> bool {
    stop_requested.lock().map(|value| *value).unwrap_or(false)
}

fn thumbnail_pause_requested(pause_requested: &Arc<Mutex<bool>>) -> bool {
    pause_requested.lock().map(|value| *value).unwrap_or(false)
}

fn wait_for_thumbnail_resume(
    progress: &Arc<Mutex<ThumbnailGenerationProgress>>,
    pause_requested: &Arc<Mutex<bool>>,
    stop_requested: &Arc<Mutex<bool>>,
) {
    loop {
        if thumbnail_stop_requested(stop_requested) {
            return;
        }
        if thumbnail_pause_requested(pause_requested) {
            set_thumbnail_progress_phase(progress, "paused");
            thread::sleep(std::time::Duration::from_millis(120));
            continue;
        }
        break;
    }
}

fn thumbnail_up_to_date(candidate: &ThumbnailCandidate) -> bool {
    let Some(thumb_path) = candidate.current_thumb_path.as_ref() else {
        return false;
    };
    if !Path::new(thumb_path).is_file() {
        return false;
    }
    match (
        candidate.current_source_modified_at,
        candidate.current_source_file_size,
    ) {
        (Some(modified_at), Some(file_size)) => {
            modified_at == candidate.modified_at && file_size == candidate.file_size
        }
        _ => false,
    }
}

fn generate_single_thumbnail(
    candidate: &ThumbnailCandidate,
    thumb_root: &Path,
    long_edge: u32,
    quality: f32,
) -> Result<String, String> {
    let source = ImageReader::open(&candidate.image_path)
        .map_err(|error| format!("Failed to open image for thumbnail: {error}"))?
        .decode()
        .map_err(|error| format!("Failed to decode image for thumbnail: {error}"))?;

    let (src_w, src_h) = source.dimensions();
    if src_w == 0 || src_h == 0 {
        return Err("Invalid image dimensions for thumbnail".to_string());
    }
    let (dst_w, dst_h) = fit_long_edge(src_w, src_h, long_edge);

    let resized = if dst_w == src_w && dst_h == src_h {
        source
    } else {
        source.resize(dst_w, dst_h, FilterType::Triangle)
    };
    let rgba = resized.to_rgba8();
    let (encoded_w, encoded_h) = rgba.dimensions();
    let mut encoded = Vec::<u8>::new();
    {
        let mut cursor = Cursor::new(&mut encoded);
        let encoder = webp::Encoder::from_rgba(rgba.as_raw(), encoded_w, encoded_h);
        let webp = encoder.encode(quality);
        cursor
            .write_all(webp.as_ref())
            .map_err(|error| format!("Failed to encode thumbnail webp: {error}"))?;
    }

    let file_name = format!("{}.webp", stable_hash_hex(&candidate.image_id));
    let thumb_path = thumb_root.join(file_name);
    fs::write(&thumb_path, encoded)
        .map_err(|error| format!("Failed to write thumbnail file: {error}"))?;
    Ok(thumb_path.to_string_lossy().to_string())
}

fn fit_long_edge(width: u32, height: u32, max_long_edge: u32) -> (u32, u32) {
    let max_side = width.max(height);
    if max_side <= max_long_edge {
        return (width.max(1), height.max(1));
    }
    let scale = max_long_edge as f64 / max_side as f64;
    let dst_w = ((width as f64 * scale).round() as u32).max(1);
    let dst_h = ((height as f64 * scale).round() as u32).max(1);
    (dst_w, dst_h)
}

fn stable_hash_hex(value: &str) -> String {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for &byte in value.as_bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}

fn upsert_thumbnail_record(
    conn: &Connection,
    image_id: &str,
    thumb_path: &str,
    source_modified_at: i64,
    source_file_size: i64,
) -> Result<(), String> {
    conn.execute(
        "
        INSERT INTO image_thumbnails (
          image_id, thumb_path, source_modified_at, source_file_size, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5)
        ON CONFLICT(image_id) DO UPDATE SET
          thumb_path = excluded.thumb_path,
          source_modified_at = excluded.source_modified_at,
          source_file_size = excluded.source_file_size,
          updated_at = excluded.updated_at
        ",
        params![
            image_id,
            thumb_path,
            source_modified_at,
            source_file_size,
            now_ms()
        ],
    )
    .map_err(|error| format!("Failed to upsert thumbnail record: {error}"))?;
    Ok(())
}

fn load_cn_tag_dictionary_map() -> Result<HashMap<String, String>, String> {
    let dictionary_path = resolve_dictionary_xlsx_path()?;
    if !dictionary_path.is_file() {
        return Ok(HashMap::new());
    }

    let mut workbook = open_workbook_auto(&dictionary_path)
        .map_err(|error| format!("Failed to open dictionary01.xlsx: {error}"))?;
    let sheet_name = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| "dictionary01.xlsx has no sheets".to_string())?;
    let range = workbook
        .worksheet_range(&sheet_name)
        .map_err(|error| format!("Failed to read dictionary sheet: {error}"))?;

    let mut rows = range.rows();
    let header = rows.next().ok_or_else(|| "dictionary sheet is empty".to_string())?;
    let header_names: Vec<String> = header
        .iter()
        .map(|cell| excel_cell_to_string(Some(cell)).unwrap_or_default().to_ascii_lowercase())
        .collect();
    let tag_idx = header_names
        .iter()
        .position(|name| name == "tag")
        .or_else(|| header_names.iter().position(|name| name == "url"))
        .unwrap_or(2);
    let cn_idx = header_names
        .iter()
        .position(|name| name.contains("right_tag_cn") || name.ends_with("_cn") || name == "cn")
        .unwrap_or(3);

    let mut map = HashMap::<String, String>::new();
    let mut count = 0usize;
    for row in rows {
        let tag_en = excel_cell_to_string(row.get(tag_idx)).unwrap_or_default();
        let tag_zh = excel_cell_to_string(row.get(cn_idx)).unwrap_or_default();
        let tag_en = tag_en.trim().to_string();
        let tag_zh = tag_zh.trim().to_string();
        if tag_en.is_empty() || tag_zh.is_empty() {
            continue;
        }
        if tag_en.eq_ignore_ascii_case("tag")
            || tag_en.eq_ignore_ascii_case("url")
            || tag_en.eq_ignore_ascii_case("english")
            || tag_en.eq_ignore_ascii_case("en")
        {
            continue;
        }
        map.insert(tag_en.clone(), tag_zh.clone());
        map.insert(normalize_tag_key(&tag_en), tag_zh);
        count += 1;
    }
    if count == 0 {
        return Ok(HashMap::new());
    }

    Ok(map)
}

fn excel_cell_to_string(cell: Option<&ExcelCell>) -> Option<String> {
    let cell = cell?;
    let text = match cell {
        ExcelCell::String(value) => value.clone(),
        ExcelCell::Float(value) => value.to_string(),
        ExcelCell::Int(value) => value.to_string(),
        ExcelCell::Bool(value) => value.to_string(),
        ExcelCell::DateTime(value) => value.to_string(),
        ExcelCell::DateTimeIso(value) => value.clone(),
        ExcelCell::DurationIso(value) => value.clone(),
        ExcelCell::Error(_) | ExcelCell::Empty => String::new(),
    };
    let normalized = text.trim().to_string();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn normalize_tag_key(tag: &str) -> String {
    tag.trim()
        .to_ascii_lowercase()
        .replace('-', "_")
        .replace(' ', "_")
}

fn escape_like_pattern(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '%' => escaped.push_str("\\%"),
            '_' => escaped.push_str("\\_"),
            '\\' => escaped.push_str("\\\\"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn split_search_tokens(input: &str) -> Vec<String> {
    let mut tokens = input
        .split_whitespace()
        .map(|token| token.trim().to_ascii_lowercase())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    tokens.sort();
    tokens.dedup();
    tokens
}

fn lookup_cn_tag(dictionary: &HashMap<String, String>, tag_en: &str) -> Option<String> {
    if let Some(value) = dictionary.get(tag_en) {
        return Some(value.clone());
    }
    let normalized = normalize_tag_key(tag_en);
    dictionary.get(&normalized).cloned()
}

fn resolve_dictionary_xlsx_path() -> Result<PathBuf, String> {
    let mut candidates = Vec::<PathBuf>::new();
    if let Ok(cwd) = env::current_dir() {
        candidates.push(cwd.join("wd-swinv2-tagger-v3").join("dictionary01.xlsx"));
        candidates.push(cwd.join("..").join("wd-swinv2-tagger-v3").join("dictionary01.xlsx"));
    }
    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.push(exe_dir.join("wd-swinv2-tagger-v3").join("dictionary01.xlsx"));
            candidates.push(
                exe_dir
                    .join("..")
                    .join("wd-swinv2-tagger-v3")
                    .join("dictionary01.xlsx"),
            );
            candidates.push(
                exe_dir
                    .join("..")
                    .join("..")
                    .join("wd-swinv2-tagger-v3")
                    .join("dictionary01.xlsx"),
            );
        }
    }

    for candidate in candidates {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Ok(PathBuf::from("dictionary01.xlsx"))
}

fn set_thumbnail_progress(
    progress: &Arc<Mutex<ThumbnailGenerationProgress>>,
    next: ThumbnailGenerationProgress,
) {
    if let Ok(mut state) = progress.lock() {
        *state = next;
    }
}

fn update_thumbnail_progress<F>(progress: &Arc<Mutex<ThumbnailGenerationProgress>>, update: F)
where
    F: FnOnce(&mut ThumbnailGenerationProgress),
{
    if let Ok(mut state) = progress.lock() {
        update(&mut state);
    }
}

fn set_thumbnail_progress_phase(progress: &Arc<Mutex<ThumbnailGenerationProgress>>, phase: &str) {
    update_thumbnail_progress(progress, |state| {
        state.phase = phase.to_string();
        state.running = phase != "idle";
        state.paused = phase == "paused";
    });
}

fn set_thumbnail_progress_total(progress: &Arc<Mutex<ThumbnailGenerationProgress>>, total: i64) {
    update_thumbnail_progress(progress, |state| {
        state.total_candidates = total.max(0);
    });
}

fn set_thumbnail_progress_counts(
    progress: &Arc<Mutex<ThumbnailGenerationProgress>>,
    processed: i64,
    generated: i64,
    skipped: i64,
    failed: i64,
) {
    update_thumbnail_progress(progress, |state| {
        state.processed_images = processed.max(0);
        state.generated_images = generated.max(0);
        state.skipped_images = skipped.max(0);
        state.failed_images = failed.max(0);
    });
}

fn set_thumbnail_progress_error(progress: &Arc<Mutex<ThumbnailGenerationProgress>>, error: &str) {
    update_thumbnail_progress(progress, |state| {
        state.last_error = Some(error.to_string());
    });
}

fn push_thumbnail_progress_recent_error(
    progress: &Arc<Mutex<ThumbnailGenerationProgress>>,
    error: &str,
) {
    update_thumbnail_progress(progress, |state| {
        let text = error.trim();
        if text.is_empty() {
            return;
        }
        state.recent_errors.push(text.to_string());
        if state.recent_errors.len() > 12 {
            let overflow = state.recent_errors.len() - 12;
            state.recent_errors.drain(0..overflow);
        }
    });
}

fn set_thumbnail_progress_done(progress: &Arc<Mutex<ThumbnailGenerationProgress>>) {
    update_thumbnail_progress(progress, |state| {
        state.running = false;
        state.paused = false;
        state.phase = "idle".to_string();
    });
}

fn set_natural_language_scan_progress(
    progress: &Arc<Mutex<NaturalLanguageScanProgress>>,
    next: NaturalLanguageScanProgress,
) {
    if let Ok(mut state) = progress.lock() {
        *state = next;
    }
}

fn update_natural_language_scan_progress<F>(
    progress: &Arc<Mutex<NaturalLanguageScanProgress>>,
    update: F,
) where
    F: FnOnce(&mut NaturalLanguageScanProgress),
{
    if let Ok(mut state) = progress.lock() {
        update(&mut state);
    }
}

fn set_natural_language_scan_progress_phase(
    progress: &Arc<Mutex<NaturalLanguageScanProgress>>,
    phase: &str,
) {
    update_natural_language_scan_progress(progress, |state| {
        state.phase = phase.to_string();
        state.running = phase != "idle";
    });
}

fn set_natural_language_scan_progress_total(
    progress: &Arc<Mutex<NaturalLanguageScanProgress>>,
    total: i64,
) {
    update_natural_language_scan_progress(progress, |state| {
        state.total_images = total.max(0);
    });
}

fn set_natural_language_scan_progress_counts(
    progress: &Arc<Mutex<NaturalLanguageScanProgress>>,
    processed: i64,
    generated: i64,
    skipped: i64,
    failed: i64,
) {
    update_natural_language_scan_progress(progress, |state| {
        state.processed_images = processed.max(0);
        state.generated_images = generated.max(0);
        state.skipped_images = skipped.max(0);
        state.failed_images = failed.max(0);
    });
}

fn set_natural_language_scan_progress_error(
    progress: &Arc<Mutex<NaturalLanguageScanProgress>>,
    error: &str,
) {
    update_natural_language_scan_progress(progress, |state| {
        state.last_error = Some(error.to_string());
    });
}

fn push_natural_language_scan_recent_error(
    progress: &Arc<Mutex<NaturalLanguageScanProgress>>,
    error: &str,
) {
    update_natural_language_scan_progress(progress, |state| {
        let text = error.trim();
        if text.is_empty() {
            return;
        }
        state.recent_errors.push(text.to_string());
        if state.recent_errors.len() > 12 {
            let overflow = state.recent_errors.len() - 12;
            state.recent_errors.drain(0..overflow);
        }
    });
}

fn set_natural_language_scan_progress_done(progress: &Arc<Mutex<NaturalLanguageScanProgress>>) {
    update_natural_language_scan_progress(progress, |state| {
        state.running = false;
        state.phase = "idle".to_string();
    });
}

fn set_scan_progress(progress: &Arc<Mutex<BackgroundScanProgress>>, next: BackgroundScanProgress) {
    if let Ok(mut state) = progress.lock() {
        *state = next;
    }
}

fn update_scan_progress<F>(progress: &Arc<Mutex<BackgroundScanProgress>>, update: F)
where
    F: FnOnce(&mut BackgroundScanProgress),
{
    if let Ok(mut state) = progress.lock() {
        update(&mut state);
    }
}

fn set_scan_progress_phase(progress: &Arc<Mutex<BackgroundScanProgress>>, phase: &str) {
    update_scan_progress(progress, |state| {
        state.phase = phase.to_string();
    });
}

fn set_scan_progress_total_folders(progress: &Arc<Mutex<BackgroundScanProgress>>, total_folders: i64) {
    update_scan_progress(progress, |state| {
        state.total_folders = total_folders.max(0);
    });
}

fn set_scan_progress_scanned_folders(progress: &Arc<Mutex<BackgroundScanProgress>>, scanned_folders: i64) {
    update_scan_progress(progress, |state| {
        state.scanned_folders = scanned_folders.max(0);
    });
}

fn set_scan_progress_new_images(progress: &Arc<Mutex<BackgroundScanProgress>>, new_images: i64) {
    update_scan_progress(progress, |state| {
        state.new_images = new_images.max(0);
    });
}

fn set_scan_progress_updated_images(progress: &Arc<Mutex<BackgroundScanProgress>>, updated_images: i64) {
    update_scan_progress(progress, |state| {
        state.updated_images = updated_images.max(0);
    });
}

fn set_scan_progress_skipped_images(progress: &Arc<Mutex<BackgroundScanProgress>>, skipped_images: i64) {
    update_scan_progress(progress, |state| {
        state.skipped_images = skipped_images.max(0);
    });
}

fn set_scan_progress_removed_missing_images(
    progress: &Arc<Mutex<BackgroundScanProgress>>,
    removed_missing_images: i64,
) {
    update_scan_progress(progress, |state| {
        state.removed_missing_images = removed_missing_images.max(0);
    });
}

fn set_scan_progress_queued_images(progress: &Arc<Mutex<BackgroundScanProgress>>, queued_images: i64) {
    update_scan_progress(progress, |state| {
        state.queued_images = queued_images.max(0);
    });
}

fn increment_scan_progress_tagged(progress: &Arc<Mutex<BackgroundScanProgress>>) {
    update_scan_progress(progress, |state| {
        state.tagged_images += 1;
    });
}

fn increment_scan_progress_failed(progress: &Arc<Mutex<BackgroundScanProgress>>) {
    update_scan_progress(progress, |state| {
        state.failed_images += 1;
    });
}

fn set_scan_progress_error(progress: &Arc<Mutex<BackgroundScanProgress>>, error: &str) {
    update_scan_progress(progress, |state| {
        state.last_error = Some(error.to_string());
    });
}

fn push_scan_progress_recent_error(progress: &Arc<Mutex<BackgroundScanProgress>>, error: &str) {
    update_scan_progress(progress, |state| {
        let text = error.trim();
        if text.is_empty() {
            return;
        }
        state.recent_errors.push(text.to_string());
        if state.recent_errors.len() > 12 {
            let overflow = state.recent_errors.len() - 12;
            state.recent_errors.drain(0..overflow);
        }
    });
}

fn set_scan_progress_done(progress: &Arc<Mutex<BackgroundScanProgress>>) {
    update_scan_progress(progress, |state| {
        state.running = false;
        state.phase = "idle".to_string();
    });
}

fn resolve_wd_tagger_model_dir(explicit_dir: Option<&str>) -> Result<PathBuf, String> {
    if let Some(path) = explicit_dir {
        let dir = PathBuf::from(path);
        if dir.is_dir() {
            return Ok(dir);
        }
        return Err(format!("Configured model directory does not exist: {}", dir.display()));
    }

    let mut candidates = Vec::<PathBuf>::new();
    if let Ok(cwd) = env::current_dir() {
        candidates.push(cwd.join("wd-swinv2-tagger-v3"));
        candidates.push(cwd.join("..").join("wd-swinv2-tagger-v3"));
    }
    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.push(exe_dir.join("wd-swinv2-tagger-v3"));
            candidates.push(exe_dir.join("..").join("wd-swinv2-tagger-v3"));
            candidates.push(exe_dir.join("..").join("..").join("wd-swinv2-tagger-v3"));
        }
    }

    for candidate in candidates {
        if candidate.join("model.onnx").is_file() && candidate.join("selected_tags.csv").is_file() {
            return Ok(candidate);
        }
    }

    Err("Cannot find wd-swinv2-tagger-v3 (model.onnx + selected_tags.csv). Put it at project root or pass modelDir.".to_string())
}

fn resolve_wd_tagger_script_path() -> Result<PathBuf, String> {
    let mut candidates = Vec::<PathBuf>::new();
    if let Ok(cwd) = env::current_dir() {
        candidates.push(cwd.join("src-tauri").join("scripts").join("wd_tagger_test.py"));
        candidates.push(cwd.join("scripts").join("wd_tagger_test.py"));
    }
    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.push(exe_dir.join("scripts").join("wd_tagger_test.py"));
            candidates.push(exe_dir.join("..").join("scripts").join("wd_tagger_test.py"));
            candidates.push(
                exe_dir
                    .join("..")
                    .join("..")
                    .join("src-tauri")
                    .join("scripts")
                    .join("wd_tagger_test.py"),
            );
        }
    }

    for candidate in candidates {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    Err("Cannot find wd_tagger_test.py under src-tauri/scripts".to_string())
}

fn resolve_chinese_clip_model_dir(explicit_dir: Option<&str>) -> Result<PathBuf, String> {
    if let Some(path) = explicit_dir {
        let dir = PathBuf::from(path);
        if dir.is_dir() {
            let text_onnx = dir.join("onnx").join("chinese_clip_text_encoder.onnx");
            let image_onnx = dir.join("onnx").join("chinese_clip_image_encoder.onnx");
            if text_onnx.is_file() && image_onnx.is_file() {
                return Ok(dir);
            }
            return Err(format!(
                "Configured model directory is missing ONNX files: {} and {}",
                text_onnx.display(),
                image_onnx.display()
            ));
        }
        return Err(format!("Configured model directory does not exist: {}", dir.display()));
    }

    let mut candidates = Vec::<PathBuf>::new();
    if let Ok(cwd) = env::current_dir() {
        candidates.push(cwd.join("model").join("chinese-clip-vit-base-patch16"));
        candidates.push(cwd.join("..").join("model").join("chinese-clip-vit-base-patch16"));
    }
    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.push(exe_dir.join("model").join("chinese-clip-vit-base-patch16"));
            candidates.push(exe_dir.join("..").join("model").join("chinese-clip-vit-base-patch16"));
            candidates.push(exe_dir.join("..").join("..").join("model").join("chinese-clip-vit-base-patch16"));
        }
    }

    for candidate in candidates {
        let text_onnx = candidate.join("onnx").join("chinese_clip_text_encoder.onnx");
        let image_onnx = candidate.join("onnx").join("chinese_clip_image_encoder.onnx");
        if text_onnx.is_file() && image_onnx.is_file() {
            return Ok(candidate);
        }
    }

    Err("Cannot find Chinese-CLIP ONNX model directory (expected model/chinese-clip-vit-base-patch16/onnx)".to_string())
}

fn resolve_chinese_clip_smoke_script_path() -> Result<PathBuf, String> {
    let mut candidates = Vec::<PathBuf>::new();
    if let Ok(cwd) = env::current_dir() {
        candidates.push(
            cwd.join("src-tauri")
                .join("scripts")
                .join("chinese_clip_onnx_smoke_test.py"),
        );
        candidates.push(cwd.join("scripts").join("chinese_clip_onnx_smoke_test.py"));
    }
    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.push(exe_dir.join("scripts").join("chinese_clip_onnx_smoke_test.py"));
            candidates.push(exe_dir.join("..").join("scripts").join("chinese_clip_onnx_smoke_test.py"));
            candidates.push(
                exe_dir
                    .join("..")
                    .join("..")
                    .join("src-tauri")
                    .join("scripts")
                    .join("chinese_clip_onnx_smoke_test.py"),
            );
        }
    }

    for candidate in candidates {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    Err("Cannot find chinese_clip_onnx_smoke_test.py under src-tauri/scripts".to_string())
}

fn resolve_chinese_clip_embed_script_path() -> Result<PathBuf, String> {
    let mut candidates = Vec::<PathBuf>::new();
    if let Ok(cwd) = env::current_dir() {
        candidates.push(
            cwd.join("src-tauri")
                .join("scripts")
                .join("chinese_clip_onnx_embed.py"),
        );
        candidates.push(cwd.join("scripts").join("chinese_clip_onnx_embed.py"));
    }
    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.push(exe_dir.join("scripts").join("chinese_clip_onnx_embed.py"));
            candidates.push(exe_dir.join("..").join("scripts").join("chinese_clip_onnx_embed.py"));
            candidates.push(
                exe_dir
                    .join("..")
                    .join("..")
                    .join("src-tauri")
                    .join("scripts")
                    .join("chinese_clip_onnx_embed.py"),
            );
        }
    }

    for candidate in candidates {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    Err("Cannot find chinese_clip_onnx_embed.py under src-tauri/scripts".to_string())
}

fn upsert_folder(conn: &Connection, folder_path: &str, scanned_at: i64) -> Result<i64, String> {
    conn.execute(
        "
        INSERT INTO folders (path, added_at, last_scanned_at, hidden)
        VALUES (?1, ?2, ?2, 0)
        ON CONFLICT(path) DO UPDATE SET
          last_scanned_at = excluded.last_scanned_at,
          hidden = 0
        ",
        params![folder_path, scanned_at],
    )
    .map_err(|error| format!("保存图库文件夹失败：{error}"))?;

    conn.query_row(
        "SELECT id FROM folders WHERE path = ?1",
        params![folder_path],
        |row| row.get(0),
    )
    .map_err(|error| format!("读取图库文件夹失败：{error}"))
}

fn upsert_hidden_folder(
    conn: &Connection,
    folder_path: &str,
    scanned_at: i64,
) -> Result<i64, String> {
    conn.execute(
        "
        INSERT INTO folders (path, added_at, last_scanned_at, hidden)
        VALUES (?1, ?2, ?2, 1)
        ON CONFLICT(path) DO UPDATE SET
          last_scanned_at = excluded.last_scanned_at,
          hidden = 1
        ",
        params![folder_path, scanned_at],
    )
    .map_err(|error| format!("保存临时图片目录失败：{error}"))?;

    conn.query_row(
        "SELECT id FROM folders WHERE path = ?1",
        params![folder_path],
        |row| row.get(0),
    )
    .map_err(|error| format!("读取临时图片目录失败：{error}"))
}

fn load_known_paths(conn: &Connection) -> Result<HashSet<String>, String> {
    let mut stmt = conn
        .prepare("SELECT path FROM images")
        .map_err(|error| format!("读取已有图片索引失败：{error}"))?;

    let paths = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| format!("读取已有图片索引失败：{error}"))?
        .collect::<Result<HashSet<_>, _>>()
        .map_err(|error| format!("读取已有图片索引失败：{error}"))?;

    Ok(paths)
}

fn load_existing_library_image_meta(conn: &Connection) -> Result<HashMap<String, ExistingImageMeta>, String> {
    let mut stmt = conn
        .prepare(
            "
            SELECT path, width, height, file_size, modified_at
            FROM images
            WHERE source = 'library'
            ",
        )
        .map_err(|error| format!("Failed to load existing library image metadata: {error}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                ExistingImageMeta {
                    width: row.get::<_, u32>(1)?,
                    height: row.get::<_, u32>(2)?,
                    file_size: row.get::<_, i64>(3)?,
                    modified_at: row.get::<_, i64>(4)?,
                },
            ))
        })
        .map_err(|error| format!("Failed to load existing library image metadata: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to load existing library image metadata: {error}"))?;

    Ok(rows.into_iter().collect())
}

fn upsert_image(conn: &Connection, folder_id: i64, image: &ScannedImage) -> Result<(), String> {
    conn.execute(
        "
        INSERT INTO images (
          id, path, file_name, ext, width, height, file_size, modified_at,
          imported_at, folder_id, missing, source
        )
        VALUES (?1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, 'library')
        ON CONFLICT(path) DO UPDATE SET
          file_name = excluded.file_name,
          ext = excluded.ext,
          width = excluded.width,
          height = excluded.height,
          file_size = excluded.file_size,
          modified_at = excluded.modified_at,
          folder_id = excluded.folder_id,
          missing = 0,
          trashed = images.trashed,
          source = 'library'
        ",
        params![
            image.path,
            image.file_name,
            image.ext,
            image.width,
            image.height,
            image.file_size,
            image.modified_at,
            image.imported_at,
            folder_id,
        ],
    )
    .map_err(|error| format!("保存图片索引失败：{error}"))?;

    Ok(())
}

fn default_reference_board_item_size(width: u32, height: u32) -> (f32, f32) {
    if width == 0 || height == 0 {
        return (220.0, 220.0);
    }

    let max_side = 220.0;
    let aspect = width as f32 / height as f32;
    if aspect >= 1.0 {
        (max_side, (max_side / aspect).max(48.0))
    } else {
        ((max_side * aspect).max(48.0), max_side)
    }
}

fn clipboard_image_extension(mime_type: &str) -> &'static str {
    match mime_type {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "png",
    }
}

fn mime_type_for_extension(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        _ => "image/png",
    }
}

fn scan_images(
    folder_path: &Path,
    imported_at: i64,
    seen_paths: &mut HashSet<String>,
    existing_meta: &HashMap<String, ExistingImageMeta>,
) -> Vec<ScannedImage> {
    let mut images = Vec::new();

    for entry in WalkDir::new(folder_path)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
        let path = entry.path();
        if !is_supported_image(path) {
            continue;
        }

        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let modified_at = metadata
            .modified()
            .ok()
            .map(system_time_ms)
            .unwrap_or(imported_at);
        let path_text = path.to_string_lossy().to_string();

        if !seen_paths.insert(path_text.clone()) {
            continue;
        }

        let file_size = metadata.len() as i64;
        let cached = existing_meta.get(&path_text);
        let (width, height) = if let Some(meta) = cached {
            if meta.modified_at == modified_at && meta.file_size == file_size {
                (meta.width, meta.height)
            } else {
                let Ok(reader) = ImageReader::open(path) else {
                    continue;
                };
                let Ok((width, height)) = reader.into_dimensions() else {
                    continue;
                };
                (width, height)
            }
        } else {
            let Ok(reader) = ImageReader::open(path) else {
                continue;
            };
            let Ok((width, height)) = reader.into_dimensions() else {
                continue;
            };
            (width, height)
        };

        images.push(ScannedImage {
            path: path_text,
            file_name: path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default(),
            ext: path
                .extension()
                .map(|extension| extension.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default(),
            width,
            height,
            file_size,
            modified_at,
            imported_at,
        });
    }

    images
}

fn normalize_folder_path(folder_path: &str) -> Result<String, String> {
    let path = PathBuf::from(folder_path);
    if !path.is_dir() {
        return Err("请选择一个有效的文件夹".to_string());
    }

    path.canonicalize()
        .map(|path| path.to_string_lossy().to_string())
        .map_err(|error| format!("读取文件夹失败：{error}"))
}

fn normalize_existing_or_stored_folder_path(folder_path: &str) -> String {
    PathBuf::from(folder_path)
        .canonicalize()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|_| folder_path.to_string())
}

fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "webp" | "gif" | "bmp" | "tif" | "tiff" | "avif"
            )
        })
        .unwrap_or(false)
}

fn now_ms() -> i64 {
    system_time_ms(SystemTime::now())
}

fn system_time_ms(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}
