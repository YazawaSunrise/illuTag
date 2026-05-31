use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use calamine::{Data as ExcelCell, Reader, open_workbook_auto};
use image::ImageReader;
use image::imageops::FilterType;
use image::GenericImageView;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter, types::Value};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    cmp::Ordering,
    collections::{BinaryHeap, HashMap, HashSet},
    env,
    fs,
    io::{BufRead, BufReader, Cursor, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{
        Arc, Mutex, OnceLock,
        mpsc::{self, TrySendError},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use walkdir::WalkDir;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

const WD_TAGGER_MODEL_NAME: &str = "wd-swinv2-tagger-v3";
const CHINESE_CLIP_MODEL_ID: &str = "cn_clip_vit_base_patch16";
const CHINESE_CLIP_MODEL_VERSION: &str = "onnx_v1";
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

fn apply_hidden_child_window(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

fn resolve_python_executable_path() -> PathBuf {
    static PYTHON_EXECUTABLE: OnceLock<PathBuf> = OnceLock::new();
    PYTHON_EXECUTABLE
        .get_or_init(|| {
            let mut candidates = Vec::<PathBuf>::new();
            if let Ok(exe_path) = env::current_exe() {
                if let Some(exe_dir) = exe_path.parent() {
                    candidates.push(exe_dir.join("runtime").join("python").join("python.exe"));
                    candidates.push(
                        exe_dir
                            .join("..")
                            .join("runtime")
                            .join("python")
                            .join("python.exe"),
                    );
                    candidates.push(
                        exe_dir
                            .join("resources")
                            .join("runtime")
                            .join("python")
                            .join("python.exe"),
                    );
                }
            }
            if let Ok(cwd) = env::current_dir() {
                candidates.push(cwd.join("runtime").join("python").join("python.exe"));
                candidates.push(
                    cwd.join("src-tauri")
                        .join("runtime")
                        .join("python")
                        .join("python.exe"),
                );
            }
            for candidate in candidates {
                if candidate.is_file() {
                    eprintln!(
                        "[python-runtime] using embedded python: {}",
                        candidate.display()
                    );
                    return candidate;
                }
            }
            eprintln!("[python-runtime] using system python from PATH");
            PathBuf::from("python")
        })
        .clone()
}

fn python_command() -> Command {
    let mut command = Command::new(resolve_python_executable_path());
    apply_hidden_child_window(&mut command);
    command
}
const THUMBNAIL_LONG_EDGE: u32 = 960;
const THUMBNAIL_WEBP_QUALITY: f32 = 85.0;
const THUMBNAIL_WORKER_COUNT: usize = 3;
const THUMBNAIL_WORKER_QUEUE_CAPACITY: usize = 2;
const NATURAL_LANGUAGE_SEARCH_DEFAULT_TOP_K: usize = 600;
const ATMOSPHERE_SIGNATURE_IMAGE_EDGE: u32 = 48;
const ATMOSPHERE_SIGNATURE_HUE_BINS: usize = 12;
const ATMOSPHERE_SIGNATURE_DIM: usize = 51;
const COLOR_SIGNATURE_HUE_BINS: usize = 24;
const COLOR_SIGNATURE_DIM: usize = COLOR_SIGNATURE_HUE_BINS + 8;
const CLIP_IMAGE_SERVICE_IDLE_RELEASE_MS: i64 = 20 * 60 * 1000;
const CLIP_IMAGE_SERVICE_IDLE_CHECK_INTERVAL_MS: u64 = 30_000;
const USER_FOLDER_SOURCE_KIND_LIBRARY_DIR: &str = "library_dir";
const TAG_DICTIONARY_SOURCE_SCHEMA_VERSION: &str = "csv-col2-col5-v1";
const BATCH_SQL_VARIABLE_LIMIT_SAFE: usize = 900;

pub struct AppState {
    pub database_path: PathBuf,
    pub library: Arc<Mutex<Option<LibraryStore>>>,
    pub background_scan_running: Arc<Mutex<bool>>,
    pub background_scan_pending: Arc<Mutex<bool>>,
    pub background_scan_pause_requested: Arc<Mutex<bool>>,
    pub background_scan_stop_requested: Arc<Mutex<bool>>,
    pub background_scan_progress: Arc<Mutex<BackgroundScanProgress>>,
    pub startup_cleanup_running: Arc<Mutex<bool>>,
    pub startup_cleanup_generation: Arc<Mutex<i64>>,
    pub thumbnail_generation_running: Arc<Mutex<bool>>,
    pub thumbnail_generation_pending: Arc<Mutex<bool>>,
    pub thumbnail_generation_pause_requested: Arc<Mutex<bool>>,
    pub thumbnail_generation_stop_requested: Arc<Mutex<bool>>,
    pub thumbnail_generation_progress: Arc<Mutex<ThumbnailGenerationProgress>>,
    pub atmosphere_generation_running: Arc<Mutex<bool>>,
    pub atmosphere_generation_pending: Arc<Mutex<bool>>,
    pub atmosphere_generation_pause_requested: Arc<Mutex<bool>>,
    pub atmosphere_generation_stop_requested: Arc<Mutex<bool>>,
    pub atmosphere_generation_progress: Arc<Mutex<AtmosphereGenerationProgress>>,
    pub color_signature_generation_running: Arc<Mutex<bool>>,
    pub color_signature_generation_pending: Arc<Mutex<bool>>,
    pub color_signature_generation_pause_requested: Arc<Mutex<bool>>,
    pub color_signature_generation_stop_requested: Arc<Mutex<bool>>,
    pub color_signature_generation_progress: Arc<Mutex<ColorSignatureGenerationProgress>>,
    pub natural_language_scan_running: Arc<Mutex<bool>>,
    pub natural_language_scan_pending: Arc<Mutex<bool>>,
    pub natural_language_scan_pause_requested: Arc<Mutex<bool>>,
    pub natural_language_scan_stop_requested: Arc<Mutex<bool>>,
    pub natural_language_scan_progress: Arc<Mutex<NaturalLanguageScanProgress>>,
    pub clip_vector_cache: Arc<Mutex<Option<ClipImageVectorCache>>>,
    pub atmosphere_signature_cache: Arc<Mutex<Option<SignatureCache>>>,
    pub color_signature_cache: Arc<Mutex<Option<SignatureCache>>>,
    pub clip_text_encoder_service: Arc<Mutex<Option<ClipTextEncoderService>>>,
    pub clip_image_encoder_service: Arc<Mutex<Option<ClipImageEncoderService>>>,
    pub clip_image_encoder_last_used_at: Arc<Mutex<i64>>,
    pub clip_image_encoder_release_worker_running: Arc<Mutex<bool>>,
    pub wd_tagger_service: Arc<Mutex<Option<WdTaggerService>>>,
}

pub struct ClipImageVectorCache {
    model_id: String,
    model_version: String,
    dimension: usize,
    vectors: HashMap<String, Vec<f32>>,
}

pub struct SignatureCache {
    dimension: usize,
    vectors: HashMap<String, Vec<f32>>,
}

pub struct ClipTextEncoderService {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    model_root: PathBuf,
}

pub struct ClipImageEncoderService {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    model_root: PathBuf,
}

pub struct WdTaggerService {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    model_path: PathBuf,
    tags_path: PathBuf,
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
    pub is_favorite: bool,
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
    pub flip_x: bool,
    pub flip_y: bool,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSupplementTagInput {
    pub tag_en: String,
    pub tag_zh: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSystemTrashResult {
    pub store: LibraryStore,
    pub moved_count: usize,
    pub failed_image_ids: Vec<String>,
    pub first_error: Option<String>,
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
    pub is_user_custom: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageUserCustomTag {
    pub tag_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageUserSupplementTag {
    pub tag_en: String,
    pub tag_zh: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageUserTagSummary {
    pub image_id: String,
    pub custom_tags: Vec<ImageUserCustomTag>,
    pub supplement_tags: Vec<ImageUserSupplementTag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserTagFolder {
    pub id: i64,
    pub name: String,
    pub sort_order: i64,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagManagementState {
    pub folders: Vec<UserTagFolder>,
    pub unclassified_tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserFolderRuleCondition {
    pub logic: String,
    pub source: String,
    pub keyword: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserFolderRule {
    pub folder_id: i64,
    pub conditions: Vec<UserFolderRuleCondition>,
    pub updated_at: i64,
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
    pub paused: bool,
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
pub struct AtmosphereGenerationProgress {
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
pub struct ColorSignatureGenerationProgress {
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
    pub paused: bool,
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
            paused: false,
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

impl Default for AtmosphereGenerationProgress {
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
            paused: false,
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

impl Default for ColorSignatureGenerationProgress {
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

#[derive(Clone)]
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

struct ThumbnailWorkerResult {
    candidate: ThumbnailCandidate,
    output: Result<String, String>,
}

struct NaturalLanguageEmbeddingCandidate {
    image_id: String,
    image_path: String,
    modified_at: i64,
    current_source_modified_at: Option<i64>,
}

struct AtmosphereSignatureCandidate {
    image_id: String,
    image_path: String,
    thumbnail_path: Option<String>,
    thumbnail_source_modified_at: Option<i64>,
    thumbnail_source_file_size: Option<i64>,
    modified_at: i64,
    file_size: i64,
}

struct AtmosphereGenerationCandidate {
    image_id: String,
    source_path: String,
    modified_at: i64,
    file_size: i64,
    priority: i32,
}

struct ColorSignatureGenerationCandidate {
    image_id: String,
    thumbnail_path: Option<String>,
    modified_at: i64,
    file_size: i64,
}

#[derive(Debug)]
struct NaturalLanguageSearchHeapEntry {
    image_id: String,
    score: f32,
}

impl PartialEq for NaturalLanguageSearchHeapEntry {
    fn eq(&self, other: &Self) -> bool {
        self.image_id == other.image_id && self.score.to_bits() == other.score.to_bits()
    }
}

impl Eq for NaturalLanguageSearchHeapEntry {}

impl PartialOrd for NaturalLanguageSearchHeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NaturalLanguageSearchHeapEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        match self
            .score
            .partial_cmp(&other.score)
            .unwrap_or(Ordering::Less)
        {
            Ordering::Equal => self.image_id.cmp(&other.image_id),
            ordering => ordering.reverse(),
        }
    }
}

pub fn list_library_from_state(state: &AppState) -> Result<LibraryStore, String> {
    let started = Instant::now();
    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;

    if let Some(cached) = library.as_ref() {
        eprintln!(
            "[startup-prof] list_library_from_state cache_hit=true images={} total_ms={}",
            cached.images.len(),
            started.elapsed().as_millis()
        );
        return Ok(cached.clone());
    }

    let open_started = Instant::now();
    let conn = open_database(&state.database_path)?;
    let open_ms = open_started.elapsed().as_millis();

    let load_started = Instant::now();
    let store = load_store(&conn)?;
    let load_ms = load_started.elapsed().as_millis();
    let image_count = store.images.len();
    *library = Some(store.clone());

    eprintln!(
        "[startup-prof] list_library_from_state cache_hit=false open_db_ms={} load_store_ms={} total_ms={} images={}",
        open_ms,
        load_ms,
        started.elapsed().as_millis(),
        image_count
    );

    Ok(store)
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

    for image in &found {
        upsert_image(&tx, folder_id, image)?;
    }
    sync_user_folder_tree_for_library_directory(&tx, &folder_path, &found, scanned_at)?;

    tx.commit()
        .map_err(|error| format!("保存图库索引失败：{error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    invalidate_all_similarity_caches(state);
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
            params![&folder_path],
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
        remove_synced_user_folder_tree_for_root(&tx, &folder_path)?;
        tx.execute("DELETE FROM folders WHERE id = ?1", params![folder_id])
            .map_err(|error| format!("删除图库文件夹失败：{error}"))?;
    }

    tx.commit()
        .map_err(|error| format!("保存图库索引失败：{error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    invalidate_all_similarity_caches(state);
    Ok(store)
}

fn sync_user_folder_tree_for_library_directory(
    conn: &Connection,
    root_folder_path: &str,
    scanned_images: &[ScannedImage],
    now: i64,
) -> Result<(), String> {
    if scanned_images.is_empty() && !Path::new(root_folder_path).is_dir() {
        return Ok(());
    }

    let root_path = normalize_existing_or_stored_folder_path(root_folder_path);
    let root_prefix = if root_path.ends_with(std::path::MAIN_SEPARATOR) {
        root_path.clone()
    } else {
        format!("{root_path}{}", std::path::MAIN_SEPARATOR)
    };
    let mut directory_paths = collect_directory_tree_paths(&root_path)?;

    for image in scanned_images {
        let image_parent = Path::new(&image.path)
            .parent()
            .map(|path| normalize_existing_or_stored_folder_path(&path.to_string_lossy()))
            .unwrap_or_else(|| root_path.clone());
        if image_parent == root_path || image_parent.starts_with(&root_prefix)
        {
            directory_paths.insert(image_parent);
        }
    }

    if directory_paths.is_empty() {
        return Ok(());
    }

    let mut sorted_paths = directory_paths.into_iter().collect::<Vec<_>>();
    sorted_paths.sort_by(|a, b| {
        path_depth_for_sort(a)
            .cmp(&path_depth_for_sort(b))
            .then_with(|| a.cmp(b))
    });

    let mut folder_id_by_path = HashMap::<String, i64>::new();
    for dir_path in sorted_paths {
        let parent_id = if dir_path == root_path {
            None
        } else {
            let parent_path = Path::new(&dir_path)
                .parent()
                .map(|path| normalize_existing_or_stored_folder_path(&path.to_string_lossy()));
            parent_path.and_then(|path| folder_id_by_path.get(&path).copied())
        };
        let name = Path::new(&dir_path)
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| dir_path.clone());
        let folder_id = upsert_synced_user_folder_for_path(conn, &dir_path, parent_id, &name, now)?;
        folder_id_by_path.insert(dir_path, folder_id);
    }

    if scanned_images.is_empty() {
        return Ok(());
    }

    for image in scanned_images {
        remove_synced_folder_assignments_for_image(conn, &image.path)?;
        let parent_path = Path::new(&image.path)
            .parent()
            .map(|path| normalize_existing_or_stored_folder_path(&path.to_string_lossy()))
            .unwrap_or_else(|| root_path.clone());
        let Some(target_folder_id) = folder_id_by_path.get(&parent_path).copied() else {
            continue;
        };
        conn.execute(
            "
            INSERT OR IGNORE INTO image_user_folders (image_id, folder_id, assigned_at)
            VALUES (?1, ?2, ?3)
            ",
            params![image.path, target_folder_id, now],
        )
        .map_err(|error| format!("Failed to sync image folder assignment: {error}"))?;
    }

    Ok(())
}

fn assign_scanned_images_to_nearest_synced_parent_folder(
    conn: &Connection,
    root_folder_path: &str,
    scanned_images: &[ScannedImage],
    now: i64,
) -> Result<(), String> {
    if scanned_images.is_empty() {
        return Ok(());
    }

    let root_path = normalize_existing_or_stored_folder_path(root_folder_path);
    let root_prefix = if root_path.ends_with(std::path::MAIN_SEPARATOR) {
        root_path.clone()
    } else {
        format!("{root_path}{}", std::path::MAIN_SEPARATOR)
    };

    let mut stmt = conn
        .prepare(
            "
            SELECT id, source_path
            FROM user_folders
            WHERE source_kind = ?1
              AND source_path IS NOT NULL
            ",
        )
        .map_err(|error| format!("Failed to load synced folders for scan assignment: {error}"))?;
    let synced_rows = stmt
        .query_map(params![USER_FOLDER_SOURCE_KIND_LIBRARY_DIR], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| format!("Failed to load synced folders for scan assignment: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to load synced folders for scan assignment: {error}"))?;
    drop(stmt);

    let mut folder_id_by_path = HashMap::<String, i64>::new();
    for (id, path) in synced_rows {
        folder_id_by_path.insert(normalize_existing_or_stored_folder_path(&path), id);
    }

    for image in scanned_images {
        remove_synced_folder_assignments_for_image(conn, &image.path)?;

        let mut current = Path::new(&image.path)
            .parent()
            .map(|path| normalize_existing_or_stored_folder_path(&path.to_string_lossy()))
            .unwrap_or_else(|| root_path.clone());

        if current != root_path && !current.starts_with(&root_prefix) {
            continue;
        }

        let mut target_folder_id = folder_id_by_path.get(&current).copied();
        while target_folder_id.is_none() && current != root_path {
            let Some(parent) = Path::new(&current)
                .parent()
                .map(|path| normalize_existing_or_stored_folder_path(&path.to_string_lossy()))
            else {
                break;
            };
            current = parent;
            target_folder_id = folder_id_by_path.get(&current).copied();
        }

        let Some(folder_id) = target_folder_id else {
            continue;
        };

        conn.execute(
            "
            INSERT OR IGNORE INTO image_user_folders (image_id, folder_id, assigned_at)
            VALUES (?1, ?2, ?3)
            ",
            params![image.path, folder_id, now],
        )
        .map_err(|error| format!("Failed to sync scanned image folder assignment: {error}"))?;
    }

    Ok(())
}

fn collect_directory_tree_paths(root_folder_path: &str) -> Result<HashSet<String>, String> {
    let mut paths = HashSet::<String>::new();
    let root = Path::new(root_folder_path);
    let root_prefix = if root_folder_path.ends_with(std::path::MAIN_SEPARATOR) {
        root_folder_path.to_string()
    } else {
        format!("{root_folder_path}{}", std::path::MAIN_SEPARATOR)
    };
    if !root.is_dir() {
        return Ok(paths);
    }

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_dir() {
            continue;
        }
        let path = normalize_existing_or_stored_folder_path(&entry.path().to_string_lossy());
        if path == root_folder_path || path.starts_with(&root_prefix) {
            paths.insert(path);
        }
    }

    if paths.is_empty() {
        paths.insert(root_folder_path.to_string());
    }
    Ok(paths)
}

fn path_depth_for_sort(path_text: &str) -> usize {
    Path::new(path_text).components().count()
}

fn upsert_synced_user_folder_for_path(
    conn: &Connection,
    source_path: &str,
    parent_id: Option<i64>,
    name: &str,
    now: i64,
) -> Result<i64, String> {
    if let Some(existing_id) = conn
        .query_row(
            "
            SELECT id
            FROM user_folders
            WHERE source_kind = ?1 AND source_path = ?2
            ",
            params![USER_FOLDER_SOURCE_KIND_LIBRARY_DIR, source_path],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| format!("Failed to query synced user folder: {error}"))?
    {
        conn.execute(
            "
            UPDATE user_folders
            SET parent_id = ?1, name = ?2, updated_at = ?3
            WHERE id = ?4
            ",
            params![parent_id, name, now, existing_id],
        )
        .map_err(|error| format!("Failed to update synced user folder: {error}"))?;
        return Ok(existing_id);
    }

    let sort_order = next_user_folder_sort_order(conn, parent_id)?;
    conn.execute(
        "
        INSERT INTO user_folders (
          parent_id, name, sort_order, source_kind, source_path, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
        ",
        params![
            parent_id,
            name,
            sort_order,
            USER_FOLDER_SOURCE_KIND_LIBRARY_DIR,
            source_path,
            now,
        ],
    )
    .map_err(|error| format!("Failed to create synced user folder: {error}"))?;

    conn.query_row(
        "SELECT id FROM user_folders WHERE source_path = ?1",
        params![source_path],
        |row| row.get::<_, i64>(0),
    )
    .map_err(|error| format!("Failed to load synced user folder id: {error}"))
}

fn remove_synced_folder_assignments_for_image(conn: &Connection, image_id: &str) -> Result<(), String> {
    conn.execute(
        "
        DELETE FROM image_user_folders
        WHERE image_id = ?1
          AND folder_id IN (
            SELECT id FROM user_folders WHERE source_kind = ?2
          )
        ",
        params![image_id, USER_FOLDER_SOURCE_KIND_LIBRARY_DIR],
    )
    .map_err(|error| format!("Failed to clear synced folder assignment: {error}"))?;
    Ok(())
}

fn remove_synced_user_folder_tree_for_root(conn: &Connection, root_path: &str) -> Result<(), String> {
    let root = normalize_existing_or_stored_folder_path(root_path);
    let mut ids = Vec::<i64>::new();

    let mut stmt = conn
        .prepare(
            "
            SELECT id, source_path
            FROM user_folders
            WHERE source_kind = ?1
              AND source_path IS NOT NULL
            ",
        )
        .map_err(|error| format!("Failed to load synced folders for delete: {error}"))?;
    let rows = stmt
        .query_map(params![USER_FOLDER_SOURCE_KIND_LIBRARY_DIR], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| format!("Failed to load synced folders for delete: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to load synced folders for delete: {error}"))?;
    drop(stmt);

    let prefix = if root.ends_with(std::path::MAIN_SEPARATOR) {
        root.clone()
    } else {
        format!("{root}{}", std::path::MAIN_SEPARATOR)
    };
    for (id, path) in rows {
        if path == root || path.starts_with(&prefix) {
            ids.push(id);
        }
    }

    for id in ids {
        conn.execute("DELETE FROM user_folders WHERE id = ?1", params![id])
            .map_err(|error| format!("Failed to delete synced user folder: {error}"))?;
    }
    Ok(())
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
    remove_signature_cache_entry(&state.atmosphere_signature_cache, &image_id)?;
    remove_signature_cache_entry(&state.color_signature_cache, &image_id)?;
    remove_clip_vector_cache_entry(&state.clip_vector_cache, &image_id)?;
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
    invalidate_all_similarity_caches(state);
    Ok(store)
}

fn move_file_to_system_recycle_bin(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let normalized_path = normalize_windows_path_for_recycle_bin(path);
        let move_result = trash::delete(&normalized_path);
        if let Err(error) = move_result {
            let error_text = error.to_string();
            eprintln!(
                "[system-trash] move failed path={} status={} stdout={} stderr={} error={}",
                normalized_path.display(),
                "N/A",
                "",
                "",
                error_text
            );
            if is_permission_denied_error_text(&error_text) {
                return Err("无权限，无法移入系统回收站。".to_string());
            }
            return Err(format!("移动到系统回收站失败：{error_text}"));
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        Err("Moving file to system recycle bin is only supported on Windows".to_string())
    }
}

fn normalize_windows_path_for_recycle_bin(path: &Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let raw = path.to_string_lossy();
        if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = raw.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
        return path.to_path_buf();
    }
    #[cfg(not(target_os = "windows"))]
    {
        path.to_path_buf()
    }
}

fn is_permission_denied_error_text(error_text: &str) -> bool {
    let lower = error_text.to_lowercase();
    lower.contains("permission denied")
        || lower.contains("access is denied")
        || lower.contains("operation not permitted")
        || error_text.contains("拒绝访问")
        || error_text.contains("无权限")
}

pub fn move_image_to_system_trash(image_id: String, state: &AppState) -> Result<LibraryStore, String> {
    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let conn = open_database(&state.database_path)?;
    conn.execute("PRAGMA foreign_keys = ON", [])
        .map_err(|error| format!("启用数据库外键失败：{error}"))?;

    let image_row = conn
        .query_row(
            "
            SELECT path, source, COALESCE(trashed, 0)
            FROM images
            WHERE id = ?1
            ",
            params![image_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|error| format!("Failed to load image before recycle-bin move: {error}"))?;
    let Some((path, source, trashed)) = image_row else {
        return Err("Image not found".to_string());
    };
    if source != "library" {
        return Err("Only library images can be moved to system recycle bin".to_string());
    }
    if trashed == 0 {
        return Err("Image must be in app recycle bin before moving to system recycle bin".to_string());
    }

    let image_path = PathBuf::from(&path);
    if !image_path.exists() {
        return Err("源文件不存在，无法移入系统回收站。".to_string());
    }
    move_file_to_system_recycle_bin(&image_path)?;

    conn.execute("DELETE FROM images WHERE id = ?1", params![image_id])
        .map_err(|error| format!("Failed to remove image index after recycle-bin move: {error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    remove_signature_cache_entry(&state.atmosphere_signature_cache, &image_id)?;
    remove_signature_cache_entry(&state.color_signature_cache, &image_id)?;
    remove_clip_vector_cache_entry(&state.clip_vector_cache, &image_id)?;
    Ok(store)
}

pub fn move_images_to_system_trash(
    image_ids: Vec<String>,
    state: &AppState,
) -> Result<BatchSystemTrashResult, String> {
    let image_ids = normalize_batch_image_ids(image_ids);
    if image_ids.is_empty() {
        return Ok(BatchSystemTrashResult {
            store: list_library_from_state(state)?,
            moved_count: 0,
            failed_image_ids: Vec::new(),
            first_error: None,
        });
    }

    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let conn = open_database(&state.database_path)?;
    conn.execute("PRAGMA foreign_keys = ON", [])
        .map_err(|error| format!("启用数据库外键失败：{error}"))?;

    let mut moved_count = 0usize;
    let mut failed_image_ids = Vec::<String>::new();
    let mut first_error = None::<String>;
    let mut removed_image_ids = Vec::<String>::new();

    for image_id in &image_ids {
        let image_row = conn
            .query_row(
                "
                SELECT path, source, COALESCE(trashed, 0)
                FROM images
                WHERE id = ?1
                ",
                params![image_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| format!("Failed to load image before recycle-bin move: {error}"))?;

        let Some((path, source, trashed)) = image_row else {
            failed_image_ids.push(image_id.clone());
            if first_error.is_none() {
                first_error = Some("Image not found".to_string());
            }
            continue;
        };
        if source != "library" {
            failed_image_ids.push(image_id.clone());
            if first_error.is_none() {
                first_error = Some("Only library images can be moved to system recycle bin".to_string());
            }
            continue;
        }
        if trashed == 0 {
            failed_image_ids.push(image_id.clone());
            if first_error.is_none() {
                first_error = Some("Image must be in app recycle bin before moving to system recycle bin".to_string());
            }
            continue;
        }

        let image_path = PathBuf::from(&path);
        if !image_path.exists() {
            failed_image_ids.push(image_id.clone());
            if first_error.is_none() {
                first_error = Some("源文件不存在，无法移入系统回收站。".to_string());
            }
            continue;
        }

        if let Err(error) = move_file_to_system_recycle_bin(&image_path) {
            failed_image_ids.push(image_id.clone());
            if first_error.is_none() {
                first_error = Some(error);
            }
            continue;
        }

        conn.execute("DELETE FROM images WHERE id = ?1", params![image_id])
            .map_err(|error| format!("Failed to remove image index after recycle-bin move: {error}"))?;
        moved_count += 1;
        removed_image_ids.push(image_id.clone());
    }

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    for image_id in &removed_image_ids {
        remove_signature_cache_entry(&state.atmosphere_signature_cache, image_id)?;
        remove_signature_cache_entry(&state.color_signature_cache, image_id)?;
        remove_clip_vector_cache_entry(&state.clip_vector_cache, image_id)?;
    }

    Ok(BatchSystemTrashResult {
        store,
        moved_count,
        failed_image_ids,
        first_error,
    })
}

pub fn toggle_image_favorite(
    image_id: String,
    favorite: bool,
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
        "UPDATE images SET is_favorite = ?1 WHERE id = ?2 AND source = 'library'",
        params![if favorite { 1 } else { 0 }, image_id],
    )
    .map_err(|error| format!("Failed to update image favorite state: {error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

fn normalize_batch_image_ids(image_ids: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::<String>::new();
    let mut normalized = Vec::<String>::new();
    for image_id in image_ids {
        let trimmed = image_id.trim();
        if trimmed.is_empty() {
            continue;
        }
        let owned = trimmed.to_string();
        if seen.insert(owned.clone()) {
            normalized.push(owned);
        }
    }
    normalized
}

fn for_each_image_id_chunk<F>(image_ids: &[String], mut handler: F) -> Result<(), String>
where
    F: FnMut(&[String]) -> Result<(), String>,
{
    for chunk in image_ids.chunks(BATCH_SQL_VARIABLE_LIMIT_SAFE) {
        handler(chunk)?;
    }
    Ok(())
}

fn build_in_placeholders(count: usize) -> String {
    std::iter::repeat("?")
        .take(count)
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn set_images_favorite(
    image_ids: Vec<String>,
    favorite: bool,
    state: &AppState,
) -> Result<LibraryStore, String> {
    let image_ids = normalize_batch_image_ids(image_ids);
    if image_ids.is_empty() {
        return list_library_from_state(state);
    }

    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let mut conn = open_database(&state.database_path)?;
    conn.execute("PRAGMA foreign_keys = ON", [])
        .map_err(|error| format!("启用数据库外键失败：{error}"))?;
    let tx = conn
        .transaction()
        .map_err(|error| format!("Failed to start batch favorite transaction: {error}"))?;

    for_each_image_id_chunk(&image_ids, |chunk| {
        let placeholders = build_in_placeholders(chunk.len());
        let sql = format!(
            "
            UPDATE images
            SET is_favorite = ?1
            WHERE source = 'library'
              AND id IN ({placeholders})
            "
        );
        let mut params_values = Vec::<Value>::with_capacity(1 + chunk.len());
        params_values.push(Value::Integer(if favorite { 1 } else { 0 }));
        for image_id in chunk {
            params_values.push(Value::Text(image_id.clone()));
        }
        tx.execute(&sql, params_from_iter(params_values.iter()))
            .map_err(|error| format!("Failed to update image favorite state in batch: {error}"))?;
        Ok(())
    })?;
    tx.commit()
        .map_err(|error| format!("Failed to commit batch favorite transaction: {error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn remove_images_from_index(
    image_ids: Vec<String>,
    state: &AppState,
) -> Result<LibraryStore, String> {
    let image_ids = normalize_batch_image_ids(image_ids);
    if image_ids.is_empty() {
        return list_library_from_state(state);
    }

    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let mut conn = open_database(&state.database_path)?;
    conn.execute("PRAGMA foreign_keys = ON", [])
        .map_err(|error| format!("启用数据库外键失败：{error}"))?;
    let tx = conn
        .transaction()
        .map_err(|error| format!("Failed to start batch trash transaction: {error}"))?;

    for_each_image_id_chunk(&image_ids, |chunk| {
        let placeholders = build_in_placeholders(chunk.len());
        let sql = format!(
            "
            UPDATE images
            SET trashed = 1
            WHERE source = 'library'
              AND id IN ({placeholders})
            "
        );
        let params_values: Vec<Value> = chunk
            .iter()
            .map(|image_id| Value::Text(image_id.clone()))
            .collect();
        tx.execute(&sql, params_from_iter(params_values.iter()))
            .map_err(|error| format!("Failed to move images to trash in batch: {error}"))?;
        Ok(())
    })?;
    tx.commit()
        .map_err(|error| format!("Failed to commit batch trash transaction: {error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    for image_id in &image_ids {
        remove_signature_cache_entry(&state.atmosphere_signature_cache, image_id)?;
        remove_signature_cache_entry(&state.color_signature_cache, image_id)?;
        remove_clip_vector_cache_entry(&state.clip_vector_cache, image_id)?;
    }
    Ok(store)
}

pub fn restore_images_from_trash(
    image_ids: Vec<String>,
    state: &AppState,
) -> Result<LibraryStore, String> {
    let image_ids = normalize_batch_image_ids(image_ids);
    if image_ids.is_empty() {
        return list_library_from_state(state);
    }

    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let mut conn = open_database(&state.database_path)?;
    conn.execute("PRAGMA foreign_keys = ON", [])
        .map_err(|error| format!("启用数据库外键失败：{error}"))?;
    let tx = conn
        .transaction()
        .map_err(|error| format!("Failed to start batch restore transaction: {error}"))?;

    for_each_image_id_chunk(&image_ids, |chunk| {
        let placeholders = build_in_placeholders(chunk.len());
        let sql = format!(
            "
            UPDATE images
            SET trashed = 0
            WHERE source = 'library'
              AND id IN ({placeholders})
            "
        );
        let params_values: Vec<Value> = chunk
            .iter()
            .map(|image_id| Value::Text(image_id.clone()))
            .collect();
        tx.execute(&sql, params_from_iter(params_values.iter()))
            .map_err(|error| format!("Failed to restore images from trash in batch: {error}"))?;
        Ok(())
    })?;
    tx.commit()
        .map_err(|error| format!("Failed to commit batch restore transaction: {error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    invalidate_all_similarity_caches(state);
    Ok(store)
}

pub fn assign_images_to_user_folder(
    image_ids: Vec<String>,
    folder_id: i64,
    state: &AppState,
) -> Result<LibraryStore, String> {
    let image_ids = normalize_batch_image_ids(image_ids);
    if image_ids.is_empty() {
        return list_library_from_state(state);
    }

    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let mut conn = open_database(&state.database_path)?;
    conn.execute("PRAGMA foreign_keys = ON", [])
        .map_err(|error| format!("启用数据库外键失败：{error}"))?;

    if !user_folder_is_leaf(&conn, folder_id)? {
        return Err("只能将图片放入最小层级文件夹".to_string());
    }

    let tx = conn
        .transaction()
        .map_err(|error| format!("Failed to start batch assign transaction: {error}"))?;
    let now = now_ms();
    for image_id in &image_ids {
        tx.execute(
            "
            INSERT OR IGNORE INTO image_user_folders (image_id, folder_id, assigned_at)
            VALUES (?1, ?2, ?3)
            ",
            params![image_id, folder_id, now],
        )
        .map_err(|error| format!("Failed to assign image to folder in batch: {error}"))?;
    }
    tx.commit()
        .map_err(|error| format!("Failed to commit batch assign transaction: {error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn move_images_to_user_folder(
    image_ids: Vec<String>,
    from_folder_id: i64,
    target_folder_id: i64,
    state: &AppState,
) -> Result<LibraryStore, String> {
    let image_ids = normalize_batch_image_ids(image_ids);
    if image_ids.is_empty() {
        return list_library_from_state(state);
    }

    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let mut conn = open_database(&state.database_path)?;
    conn.execute("PRAGMA foreign_keys = ON", [])
        .map_err(|error| format!("启用数据库外键失败：{error}"))?;

    if !user_folder_is_leaf(&conn, target_folder_id)? {
        return Err("只能将图片放入最小层级文件夹".to_string());
    }

    let tx = conn
        .transaction()
        .map_err(|error| format!("Failed to start batch move transaction: {error}"))?;
    let now = now_ms();
    for image_id in &image_ids {
        tx.execute(
            "
            INSERT OR IGNORE INTO image_user_folders (image_id, folder_id, assigned_at)
            VALUES (?1, ?2, ?3)
            ",
            params![image_id, target_folder_id, now],
        )
        .map_err(|error| format!("Failed to assign image during batch move: {error}"))?;
    }

    if from_folder_id != target_folder_id {
        for image_id in &image_ids {
            tx.execute(
                "
                DELETE FROM image_user_folders
                WHERE image_id = ?1 AND folder_id = ?2
                ",
                params![image_id, from_folder_id],
            )
            .map_err(|error| format!("Failed to remove image from source folder in batch move: {error}"))?;
        }
    }

    tx.commit()
        .map_err(|error| format!("Failed to commit batch move transaction: {error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn remove_images_from_user_folder(
    image_ids: Vec<String>,
    folder_id: i64,
    state: &AppState,
) -> Result<LibraryStore, String> {
    let image_ids = normalize_batch_image_ids(image_ids);
    if image_ids.is_empty() {
        return list_library_from_state(state);
    }

    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let mut conn = open_database(&state.database_path)?;
    conn.execute("PRAGMA foreign_keys = ON", [])
        .map_err(|error| format!("启用数据库外键失败：{error}"))?;
    let tx = conn
        .transaction()
        .map_err(|error| format!("Failed to start batch remove-folder transaction: {error}"))?;

    for_each_image_id_chunk(&image_ids, |chunk| {
        let placeholders = build_in_placeholders(chunk.len());
        let sql = format!(
            "
            DELETE FROM image_user_folders
            WHERE folder_id = ?1
              AND image_id IN ({placeholders})
            "
        );
        let mut params_values = Vec::<Value>::with_capacity(1 + chunk.len());
        params_values.push(Value::Integer(folder_id));
        for image_id in chunk {
            params_values.push(Value::Text(image_id.clone()));
        }
        tx.execute(&sql, params_from_iter(params_values.iter()))
            .map_err(|error| format!("Failed to remove images from folder in batch: {error}"))?;
        Ok(())
    })?;
    tx.commit()
        .map_err(|error| format!("Failed to commit batch remove-folder transaction: {error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
}

pub fn add_images_user_tags(
    image_ids: Vec<String>,
    custom_tags: Vec<String>,
    supplement_tags: Vec<BatchSupplementTagInput>,
    state: &AppState,
) -> Result<LibraryStore, String> {
    let image_ids = normalize_batch_image_ids(image_ids);
    if image_ids.is_empty() {
        return list_library_from_state(state);
    }

    let mut normalized_custom_tags = Vec::<String>::new();
    let mut custom_seen = HashSet::<String>::new();
    for tag in custom_tags {
        let normalized = normalize_tag_text(&tag);
        if normalized.is_empty() {
            continue;
        }
        if custom_seen.insert(normalized.clone()) {
            normalized_custom_tags.push(normalized);
        }
    }

    let mut normalized_supplement_tags = Vec::<(String, Option<String>)>::new();
    let mut supplement_seen = HashSet::<String>::new();
    for tag in supplement_tags {
        let normalized_tag_en = normalize_tag_text(&tag.tag_en);
        if normalized_tag_en.is_empty() {
            continue;
        }
        let dedupe_key = normalized_tag_en.to_lowercase();
        if !supplement_seen.insert(dedupe_key) {
            continue;
        }
        let normalized_tag_zh = normalize_optional_tag_text(tag.tag_zh);
        normalized_supplement_tags.push((normalized_tag_en, normalized_tag_zh));
    }

    if normalized_custom_tags.is_empty() && normalized_supplement_tags.is_empty() {
        return list_library_from_state(state);
    }

    let mut library = state
        .library
        .lock()
        .map_err(|_| "图库状态被占用，请稍后再试".to_string())?;
    let mut conn = open_database(&state.database_path)?;
    conn.execute("PRAGMA foreign_keys = ON", [])
        .map_err(|error| format!("启用数据库外键失败：{error}"))?;

    let tx = conn
        .transaction()
        .map_err(|error| format!("Failed to start batch add tags transaction: {error}"))?;
    let now = now_ms();

    for tag in &normalized_custom_tags {
        upsert_user_custom_tag(&tx, tag)?;
    }

    for image_id in &image_ids {
        for tag_text in &normalized_custom_tags {
            tx.execute(
                "
                INSERT INTO image_user_custom_tags (
                  image_id, tag_text, created_at, updated_at
                )
                VALUES (?1, ?2, ?3, ?3)
                ON CONFLICT(image_id, tag_text) DO UPDATE SET
                  updated_at = excluded.updated_at
                ",
                params![image_id, tag_text, now],
            )
            .map_err(|error| format!("Failed to add custom user tag in batch: {error}"))?;
        }

        for (tag_en, tag_zh) in &normalized_supplement_tags {
            tx.execute(
                "
                INSERT INTO image_user_supplement_tags (
                  image_id, tag_en, tag_zh, created_at, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?4)
                ON CONFLICT(image_id, tag_en) DO UPDATE SET
                  tag_zh = COALESCE(excluded.tag_zh, image_user_supplement_tags.tag_zh),
                  updated_at = excluded.updated_at
                ",
                params![image_id, tag_en, tag_zh, now],
            )
            .map_err(|error| format!("Failed to add supplement user tag in batch: {error}"))?;
        }

        apply_matching_user_folder_rules_for_image(&tx, image_id)?;
    }

    tx.commit()
        .map_err(|error| format!("Failed to commit batch add tags transaction: {error}"))?;

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

    let mut command = python_command();
    let output = command
        .arg("-X")
        .arg("utf8")
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackgroundScanJobMode {
    CollectOnly,
    CollectAndTag,
    TagPendingOnly,
}

fn start_scan_all_folders_worker(state: &AppState, mode: BackgroundScanJobMode) -> Result<bool, String> {
    let mut running = state
        .background_scan_running
        .lock()
        .map_err(|_| "Background scan state is locked".to_string())?;
    if *running {
        if matches!(mode, BackgroundScanJobMode::CollectOnly) {
            return Ok(false);
        }
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
    if let Ok(mut pause_requested) = state.background_scan_pause_requested.lock() {
        *pause_requested = false;
    }
    if let Ok(mut stop_requested) = state.background_scan_stop_requested.lock() {
        *stop_requested = false;
    }

    let database_path = state.database_path.clone();
    let library_cache = Arc::clone(&state.library);
    let background_scan_running = Arc::clone(&state.background_scan_running);
    let background_scan_pending = Arc::clone(&state.background_scan_pending);
    let background_scan_pause_requested_flag = Arc::clone(&state.background_scan_pause_requested);
    let background_scan_stop_requested_flag = Arc::clone(&state.background_scan_stop_requested);
    let background_scan_progress = Arc::clone(&state.background_scan_progress);
    let startup_cleanup_running = Arc::clone(&state.startup_cleanup_running);
    let wd_tagger_service = Arc::clone(&state.wd_tagger_service);
    let clip_vector_cache = Arc::clone(&state.clip_vector_cache);
    let atmosphere_signature_cache = Arc::clone(&state.atmosphere_signature_cache);
    let color_signature_cache = Arc::clone(&state.color_signature_cache);
    thread::spawn(move || {
        wait_until_startup_cleanup_finished(&startup_cleanup_running);
        eprintln!("[wd-scan] worker started");
        loop {
            let tag_pending_only = matches!(mode, BackgroundScanJobMode::TagPendingOnly);
            set_scan_progress(
                &background_scan_progress,
                BackgroundScanProgress {
                    running: true,
                    phase: if tag_pending_only {
                        "tagging".to_string()
                    } else {
                        "collecting".to_string()
                    },
                    ..BackgroundScanProgress::default()
                },
            );

            let tag_queue_result = if tag_pending_only {
                let conn = match open_database(&database_path) {
                    Ok(value) => value,
                    Err(error) => {
                        set_scan_progress_error(&background_scan_progress, &error);
                        push_scan_progress_recent_error(&background_scan_progress, &error);
                        eprintln!("[wd-tag] {error}");
                        break;
                    }
                };
                let mut tag_queue_image_ids = match collect_pending_tag_image_ids(&conn) {
                    Ok(value) => value,
                    Err(error) => {
                        set_scan_progress_error(&background_scan_progress, &error);
                        push_scan_progress_recent_error(&background_scan_progress, &error);
                        eprintln!("[wd-tag] {error}");
                        break;
                    }
                };
                tag_queue_image_ids.sort();
                tag_queue_image_ids.dedup();
                set_scan_progress_phase(&background_scan_progress, "tagging");
                set_scan_progress_queued_images(
                    &background_scan_progress,
                    tag_queue_image_ids.len() as i64,
                );
                Ok(ScanCollectResult { tag_queue_image_ids })
            } else {
                scan_all_folders_and_collect_new_images(
                    &database_path,
                    &background_scan_progress,
                    &background_scan_pause_requested_flag,
                    &background_scan_stop_requested_flag,
                    matches!(mode, BackgroundScanJobMode::CollectAndTag),
                )
            };

            match tag_queue_result {
                Ok(scan_result) => {
                    if !tag_pending_only {
                        if let Ok(mut cache) = library_cache.lock() {
                            *cache = None;
                        }
                        clear_optional_cache(&clip_vector_cache);
                        clear_optional_cache(&atmosphere_signature_cache);
                        clear_optional_cache(&color_signature_cache);
                    }
                    if matches!(mode, BackgroundScanJobMode::CollectAndTag | BackgroundScanJobMode::TagPendingOnly) {
                        if let Err(error) = tag_images_with_wd_model(
                            &database_path,
                            &scan_result.tag_queue_image_ids,
                            &background_scan_progress,
                            &background_scan_pause_requested_flag,
                            &background_scan_stop_requested_flag,
                            &wd_tagger_service,
                        ) {
                            set_scan_progress_error(&background_scan_progress, &error);
                            push_scan_progress_recent_error(&background_scan_progress, &error);
                            eprintln!("[wd-tag] {error}");
                        }
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
            clear_optional_cache(&clip_vector_cache);
            clear_optional_cache(&atmosphere_signature_cache);
            clear_optional_cache(&color_signature_cache);

            if background_scan_stop_requested(&background_scan_stop_requested_flag) {
                if let Ok(mut pending) = background_scan_pending.lock() {
                    *pending = false;
                }
                break;
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
        clear_optional_cache(&clip_vector_cache);
        clear_optional_cache(&atmosphere_signature_cache);
        clear_optional_cache(&color_signature_cache);
        release_wd_tagger_service(&wd_tagger_service);
        if let Ok(mut running) = background_scan_running.lock() {
            *running = false;
        }
        if let Ok(mut pending) = background_scan_pending.lock() {
            *pending = false;
        }
        if let Ok(mut pause_requested) = background_scan_pause_requested_flag.lock() {
            *pause_requested = false;
        }
        if let Ok(mut stop_requested) = background_scan_stop_requested_flag.lock() {
            *stop_requested = false;
        }
        eprintln!("[wd-scan] worker finished");
    });

    Ok(true)
}

pub fn start_scan_all_folders_with_tagging(state: &AppState) -> Result<bool, String> {
    start_scan_all_folders_worker(state, BackgroundScanJobMode::CollectAndTag)
}

pub fn start_scan_all_folders_collect_only(state: &AppState) -> Result<bool, String> {
    start_scan_all_folders_worker(state, BackgroundScanJobMode::CollectOnly)
}

pub fn start_tag_pending_images_only(state: &AppState) -> Result<bool, String> {
    start_scan_all_folders_worker(state, BackgroundScanJobMode::TagPendingOnly)
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

pub fn pause_background_scan(state: &AppState) -> Result<bool, String> {
    let running = state
        .background_scan_running
        .lock()
        .map_err(|_| "Background scan state is locked".to_string())
        .map(|value| *value)?;
    if !running {
        return Ok(false);
    }
    if let Ok(mut pause_requested) = state.background_scan_pause_requested.lock() {
        *pause_requested = true;
    }
    update_scan_progress(&state.background_scan_progress, |progress| {
        progress.paused = true;
        progress.phase = "paused".to_string();
    });
    Ok(true)
}

pub fn resume_background_scan(state: &AppState) -> Result<bool, String> {
    let running = state
        .background_scan_running
        .lock()
        .map_err(|_| "Background scan state is locked".to_string())
        .map(|value| *value)?;
    if !running {
        return Ok(false);
    }
    if let Ok(mut pause_requested) = state.background_scan_pause_requested.lock() {
        *pause_requested = false;
    }
    update_scan_progress(&state.background_scan_progress, |progress| {
        progress.paused = false;
        if progress.phase == "paused" {
            progress.phase = if progress.queued_images > 0 {
                "tagging".to_string()
            } else {
                "collecting".to_string()
            };
        }
    });
    Ok(true)
}

pub fn stop_background_scan(state: &AppState) -> Result<bool, String> {
    let running = state
        .background_scan_running
        .lock()
        .map_err(|_| "Background scan state is locked".to_string())
        .map(|value| *value)?;
    if !running {
        return Ok(false);
    }
    if let Ok(mut stop_requested) = state.background_scan_stop_requested.lock() {
        *stop_requested = true;
    }
    if let Ok(mut pause_requested) = state.background_scan_pause_requested.lock() {
        *pause_requested = false;
    }
    update_scan_progress(&state.background_scan_progress, |progress| {
        progress.paused = false;
        progress.phase = "stopping".to_string();
    });
    Ok(true)
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
    let clip_vector_cache = Arc::clone(&state.clip_vector_cache);
    let atmosphere_signature_cache = Arc::clone(&state.atmosphere_signature_cache);
    let color_signature_cache = Arc::clone(&state.color_signature_cache);

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
        if matches!(result, Ok(removed) if removed > 0) {
            clear_optional_cache(&clip_vector_cache);
            clear_optional_cache(&atmosphere_signature_cache);
            clear_optional_cache(&color_signature_cache);
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

pub fn start_atmosphere_generation(state: &AppState) -> Result<bool, String> {
    let mut running = state
        .atmosphere_generation_running
        .lock()
        .map_err(|_| "Atmosphere generation state is locked".to_string())?;
    if *running {
        if let Ok(mut pending) = state.atmosphere_generation_pending.lock() {
            *pending = true;
        }
        return Ok(false);
    }
    *running = true;
    drop(running);

    if let Ok(mut pending) = state.atmosphere_generation_pending.lock() {
        *pending = false;
    }
    if let Ok(mut pause_requested) = state.atmosphere_generation_pause_requested.lock() {
        *pause_requested = false;
    }
    if let Ok(mut stop_requested) = state.atmosphere_generation_stop_requested.lock() {
        *stop_requested = false;
    }
    set_atmosphere_progress(
        &state.atmosphere_generation_progress,
        AtmosphereGenerationProgress {
            running: true,
            paused: false,
            phase: "queueing".to_string(),
            ..AtmosphereGenerationProgress::default()
        },
    );

    let database_path = state.database_path.clone();
    let library_cache = Arc::clone(&state.library);
    let task_running = Arc::clone(&state.atmosphere_generation_running);
    let task_pending = Arc::clone(&state.atmosphere_generation_pending);
    let task_pause_requested = Arc::clone(&state.atmosphere_generation_pause_requested);
    let task_stop_requested = Arc::clone(&state.atmosphere_generation_stop_requested);
    let task_progress = Arc::clone(&state.atmosphere_generation_progress);
    let atmosphere_signature_cache = Arc::clone(&state.atmosphere_signature_cache);

    thread::spawn(move || {
        loop {
            set_atmosphere_progress_phase(&task_progress, "queueing");
            match generate_atmosphere_signatures_once(
                &database_path,
                &task_progress,
                &task_pause_requested,
                &task_stop_requested,
                &atmosphere_signature_cache,
            ) {
                Ok(generated) => {
                    if generated > 0 {
                        if let Ok(mut cache) = library_cache.lock() {
                            *cache = None;
                        }
                    }
                }
                Err(error) => {
                    set_atmosphere_progress_error(&task_progress, &error);
                    push_atmosphere_progress_recent_error(&task_progress, &error);
                }
            }

            let rerun = if let Ok(mut pending) = task_pending.lock() {
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

        set_atmosphere_progress_done(&task_progress);
        if let Ok(mut running) = task_running.lock() {
            *running = false;
        }
        if let Ok(mut pending) = task_pending.lock() {
            *pending = false;
        }
        if let Ok(mut pause_requested) = task_pause_requested.lock() {
            *pause_requested = false;
        }
        if let Ok(mut stop_requested) = task_stop_requested.lock() {
            *stop_requested = false;
        }
    });

    Ok(true)
}

pub fn atmosphere_generation_status(state: &AppState) -> Result<AtmosphereGenerationProgress, String> {
    state
        .atmosphere_generation_progress
        .lock()
        .map_err(|_| "Atmosphere generation progress state is locked".to_string())
        .map(|value| value.clone())
}

pub fn pause_atmosphere_generation(state: &AppState) -> Result<bool, String> {
    let running = state
        .atmosphere_generation_running
        .lock()
        .map_err(|_| "Atmosphere generation state is locked".to_string())
        .map(|value| *value)?;
    if !running {
        return Ok(false);
    }
    if let Ok(mut pause_requested) = state.atmosphere_generation_pause_requested.lock() {
        *pause_requested = true;
    }
    update_atmosphere_progress(&state.atmosphere_generation_progress, |progress| {
        progress.paused = true;
        progress.phase = "paused".to_string();
    });
    Ok(true)
}

pub fn resume_atmosphere_generation(state: &AppState) -> Result<bool, String> {
    let running = state
        .atmosphere_generation_running
        .lock()
        .map_err(|_| "Atmosphere generation state is locked".to_string())
        .map(|value| *value)?;
    if !running {
        return Ok(false);
    }
    if let Ok(mut pause_requested) = state.atmosphere_generation_pause_requested.lock() {
        *pause_requested = false;
    }
    update_atmosphere_progress(&state.atmosphere_generation_progress, |progress| {
        progress.paused = false;
        if progress.phase == "paused" {
            progress.phase = "generating".to_string();
        }
    });
    Ok(true)
}

pub fn stop_atmosphere_generation(state: &AppState) -> Result<bool, String> {
    let running = state
        .atmosphere_generation_running
        .lock()
        .map_err(|_| "Atmosphere generation state is locked".to_string())
        .map(|value| *value)?;
    if !running {
        return Ok(false);
    }
    if let Ok(mut stop_requested) = state.atmosphere_generation_stop_requested.lock() {
        *stop_requested = true;
    }
    if let Ok(mut pause_requested) = state.atmosphere_generation_pause_requested.lock() {
        *pause_requested = false;
    }
    update_atmosphere_progress(&state.atmosphere_generation_progress, |progress| {
        progress.paused = false;
        progress.phase = "stopping".to_string();
    });
    Ok(true)
}

pub fn rebuild_atmosphere_signature_cache(state: &AppState) -> Result<bool, String> {
    let running = state
        .atmosphere_generation_running
        .lock()
        .map_err(|_| "Atmosphere generation state is locked".to_string())
        .map(|value| *value)?;
    if running {
        return Err("氛围特征任务正在运行，请先停止后再重建".to_string());
    }

    let conn = open_database(&state.database_path)?;
    clear_atmosphere_signature_cache_storage(&conn)?;
    clear_optional_cache(&state.atmosphere_signature_cache);
    if let Ok(mut cache) = state.library.lock() {
        *cache = None;
    }
    set_atmosphere_progress(
        &state.atmosphere_generation_progress,
        AtmosphereGenerationProgress::default(),
    );
    start_atmosphere_generation(state)
}

pub fn start_color_signature_generation(state: &AppState) -> Result<bool, String> {
    let mut running = state
        .color_signature_generation_running
        .lock()
        .map_err(|_| "Color signature generation state is locked".to_string())?;
    if *running {
        if let Ok(mut pending) = state.color_signature_generation_pending.lock() {
            *pending = true;
        }
        return Ok(false);
    }
    *running = true;
    drop(running);

    if let Ok(mut pending) = state.color_signature_generation_pending.lock() {
        *pending = false;
    }
    if let Ok(mut pause_requested) = state.color_signature_generation_pause_requested.lock() {
        *pause_requested = false;
    }
    if let Ok(mut stop_requested) = state.color_signature_generation_stop_requested.lock() {
        *stop_requested = false;
    }
    set_color_signature_progress(
        &state.color_signature_generation_progress,
        ColorSignatureGenerationProgress {
            running: true,
            paused: false,
            phase: "queueing".to_string(),
            ..ColorSignatureGenerationProgress::default()
        },
    );

    let database_path = state.database_path.clone();
    let library_cache = Arc::clone(&state.library);
    let task_running = Arc::clone(&state.color_signature_generation_running);
    let task_pending = Arc::clone(&state.color_signature_generation_pending);
    let task_pause_requested = Arc::clone(&state.color_signature_generation_pause_requested);
    let task_stop_requested = Arc::clone(&state.color_signature_generation_stop_requested);
    let task_progress = Arc::clone(&state.color_signature_generation_progress);
    let color_signature_cache = Arc::clone(&state.color_signature_cache);

    thread::spawn(move || {
        loop {
            set_color_signature_progress_phase(&task_progress, "queueing");
            match generate_color_signatures_once(
                &database_path,
                &task_progress,
                &task_pause_requested,
                &task_stop_requested,
                &color_signature_cache,
            ) {
                Ok(generated) => {
                    if generated > 0 {
                        if let Ok(mut cache) = library_cache.lock() {
                            *cache = None;
                        }
                    }
                }
                Err(error) => {
                    set_color_signature_progress_error(&task_progress, &error);
                    push_color_signature_progress_recent_error(&task_progress, &error);
                }
            }

            let rerun = if let Ok(mut pending) = task_pending.lock() {
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

        set_color_signature_progress_done(&task_progress);
        if let Ok(mut running) = task_running.lock() {
            *running = false;
        }
        if let Ok(mut pending) = task_pending.lock() {
            *pending = false;
        }
        if let Ok(mut pause_requested) = task_pause_requested.lock() {
            *pause_requested = false;
        }
        if let Ok(mut stop_requested) = task_stop_requested.lock() {
            *stop_requested = false;
        }
    });

    Ok(true)
}

pub fn color_signature_generation_status(
    state: &AppState,
) -> Result<ColorSignatureGenerationProgress, String> {
    state
        .color_signature_generation_progress
        .lock()
        .map_err(|_| "Color signature generation progress state is locked".to_string())
        .map(|value| value.clone())
}

pub fn pause_color_signature_generation(state: &AppState) -> Result<bool, String> {
    let running = state
        .color_signature_generation_running
        .lock()
        .map_err(|_| "Color signature generation state is locked".to_string())
        .map(|value| *value)?;
    if !running {
        return Ok(false);
    }
    if let Ok(mut pause_requested) = state.color_signature_generation_pause_requested.lock() {
        *pause_requested = true;
    }
    update_color_signature_progress(&state.color_signature_generation_progress, |progress| {
        progress.paused = true;
        progress.phase = "paused".to_string();
    });
    Ok(true)
}

pub fn resume_color_signature_generation(state: &AppState) -> Result<bool, String> {
    let running = state
        .color_signature_generation_running
        .lock()
        .map_err(|_| "Color signature generation state is locked".to_string())
        .map(|value| *value)?;
    if !running {
        return Ok(false);
    }
    if let Ok(mut pause_requested) = state.color_signature_generation_pause_requested.lock() {
        *pause_requested = false;
    }
    update_color_signature_progress(&state.color_signature_generation_progress, |progress| {
        progress.paused = false;
        if progress.phase == "paused" {
            progress.phase = "generating".to_string();
        }
    });
    Ok(true)
}

pub fn stop_color_signature_generation(state: &AppState) -> Result<bool, String> {
    let running = state
        .color_signature_generation_running
        .lock()
        .map_err(|_| "Color signature generation state is locked".to_string())
        .map(|value| *value)?;
    if !running {
        return Ok(false);
    }
    if let Ok(mut stop_requested) = state.color_signature_generation_stop_requested.lock() {
        *stop_requested = true;
    }
    if let Ok(mut pause_requested) = state.color_signature_generation_pause_requested.lock() {
        *pause_requested = false;
    }
    update_color_signature_progress(&state.color_signature_generation_progress, |progress| {
        progress.paused = false;
        progress.phase = "stopping".to_string();
    });
    Ok(true)
}

pub fn rebuild_color_signature_cache(state: &AppState) -> Result<bool, String> {
    let running = state
        .color_signature_generation_running
        .lock()
        .map_err(|_| "Color signature generation state is locked".to_string())
        .map(|value| *value)?;
    if running {
        return Err("配色特征任务正在运行，请先停止后再重建".to_string());
    }

    let conn = open_database(&state.database_path)?;
    clear_color_signature_cache_storage(&conn)?;
    clear_optional_cache(&state.color_signature_cache);
    if let Ok(mut cache) = state.library.lock() {
        *cache = None;
    }
    set_color_signature_progress(
        &state.color_signature_generation_progress,
        ColorSignatureGenerationProgress::default(),
    );
    start_color_signature_generation(state)
}

pub fn start_natural_language_scan(state: &AppState) -> Result<bool, String> {
    ensure_clip_vector_cache_loaded(state)?;
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
    if let Ok(mut pause_requested) = state.natural_language_scan_pause_requested.lock() {
        *pause_requested = false;
    }
    if let Ok(mut stop_requested) = state.natural_language_scan_stop_requested.lock() {
        *stop_requested = false;
    }

    let database_path = state.database_path.clone();
    let library_cache = Arc::clone(&state.library);
    let scan_running = Arc::clone(&state.natural_language_scan_running);
    let scan_pending = Arc::clone(&state.natural_language_scan_pending);
    let scan_pause_requested = Arc::clone(&state.natural_language_scan_pause_requested);
    let scan_stop_requested = Arc::clone(&state.natural_language_scan_stop_requested);
    let scan_progress = Arc::clone(&state.natural_language_scan_progress);
    let clip_vector_cache = Arc::clone(&state.clip_vector_cache);
    let clip_image_encoder_service = Arc::clone(&state.clip_image_encoder_service);
    let clip_image_encoder_last_used_at = Arc::clone(&state.clip_image_encoder_last_used_at);
    let clip_image_encoder_release_worker_running =
        Arc::clone(&state.clip_image_encoder_release_worker_running);

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

            if let Err(error) = generate_natural_language_embeddings_once(
                &database_path,
                &scan_progress,
                &clip_vector_cache,
                &clip_image_encoder_service,
                &clip_image_encoder_last_used_at,
                &clip_image_encoder_release_worker_running,
                &scan_pause_requested,
                &scan_stop_requested,
            ) {
                set_natural_language_scan_progress_error(&scan_progress, &error);
                push_natural_language_scan_recent_error(&scan_progress, &error);
                eprintln!("[clip-scan] {error}");
            }

            if let Ok(mut cache) = library_cache.lock() {
                *cache = None;
            }

            if natural_language_scan_stop_requested(&scan_stop_requested) {
                if let Ok(mut pending) = scan_pending.lock() {
                    *pending = false;
                }
                break;
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
        if let Ok(mut pause_requested) = scan_pause_requested.lock() {
            *pause_requested = false;
        }
        if let Ok(mut stop_requested) = scan_stop_requested.lock() {
            *stop_requested = false;
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

pub fn pause_natural_language_scan(state: &AppState) -> Result<bool, String> {
    let running = state
        .natural_language_scan_running
        .lock()
        .map_err(|_| "Natural language scan state is locked".to_string())
        .map(|value| *value)?;
    if !running {
        return Ok(false);
    }
    if let Ok(mut pause_requested) = state.natural_language_scan_pause_requested.lock() {
        *pause_requested = true;
    }
    update_natural_language_scan_progress(&state.natural_language_scan_progress, |progress| {
        progress.paused = true;
        progress.phase = "paused".to_string();
    });
    Ok(true)
}

pub fn resume_natural_language_scan(state: &AppState) -> Result<bool, String> {
    let running = state
        .natural_language_scan_running
        .lock()
        .map_err(|_| "Natural language scan state is locked".to_string())
        .map(|value| *value)?;
    if !running {
        return Ok(false);
    }
    if let Ok(mut pause_requested) = state.natural_language_scan_pause_requested.lock() {
        *pause_requested = false;
    }
    update_natural_language_scan_progress(&state.natural_language_scan_progress, |progress| {
        progress.paused = false;
        if progress.phase == "paused" {
            progress.phase = "generating".to_string();
        }
    });
    Ok(true)
}

pub fn stop_natural_language_scan(state: &AppState) -> Result<bool, String> {
    let running = state
        .natural_language_scan_running
        .lock()
        .map_err(|_| "Natural language scan state is locked".to_string())
        .map(|value| *value)?;
    if !running {
        return Ok(false);
    }
    if let Ok(mut stop_requested) = state.natural_language_scan_stop_requested.lock() {
        *stop_requested = true;
    }
    if let Ok(mut pause_requested) = state.natural_language_scan_pause_requested.lock() {
        *pause_requested = false;
    }
    update_natural_language_scan_progress(&state.natural_language_scan_progress, |progress| {
        progress.paused = false;
        progress.phase = "stopping".to_string();
    });
    Ok(true)
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

    ensure_clip_vector_cache_loaded(state)?;
    let text_embedding = run_chinese_clip_text_embedding_via_service(trimmed_query, state)?;
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

    let cache_guard = state
        .clip_vector_cache
        .lock()
        .map_err(|_| "Clip vector cache state is locked".to_string())?;
    let cache = cache_guard
        .as_ref()
        .ok_or_else(|| "Clip vector cache not loaded".to_string())?;
    if cache.model_id != CHINESE_CLIP_MODEL_ID || cache.model_version != CHINESE_CLIP_MODEL_VERSION {
        return Ok(Vec::new());
    }
    if cache.dimension != text_embedding.len() {
        return Ok(Vec::new());
    }

    let top_k = NATURAL_LANGUAGE_SEARCH_DEFAULT_TOP_K.max(1);
    let mut heap = BinaryHeap::<NaturalLanguageSearchHeapEntry>::new();
    for (image_id, vector) in &cache.vectors {
        if let Some(filter) = &candidate_filter {
            if !filter.contains(image_id) {
                continue;
            }
        }
        if vector.len() != text_embedding.len() {
            continue;
        }
        let score = dot_product(&text_embedding, vector);
        if !score.is_finite() {
            continue;
        }
        heap.push(NaturalLanguageSearchHeapEntry {
            image_id: image_id.clone(),
            score,
        });
        if heap.len() > top_k {
            let _ = heap.pop();
        }
    }

    let mut ranked = Vec::<NaturalLanguageSearchHeapEntry>::with_capacity(heap.len());
    while let Some(entry) = heap.pop() {
        ranked.push(entry);
    }
    ranked.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.image_id.cmp(&b.image_id))
    });
    Ok(ranked.into_iter().map(|entry| entry.image_id).collect())
}

pub fn search_gallery_image_ids_by_external_image(
    image_path: Option<String>,
    image_url: Option<String>,
    image_bytes: Option<Vec<u8>>,
    image_base64: Option<String>,
    search_type: Option<String>,
    candidate_image_ids: Option<Vec<String>>,
    limit: Option<usize>,
    state: &AppState,
) -> Result<Vec<String>, String> {
    let mode = search_type
        .as_deref()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "default".to_string());

    let query_path = image_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let mut temp_query_path: Option<PathBuf> = None;
    let query_image_path = if let Some(path) = query_path {
        if !Path::new(&path).is_file() {
            return Err(format!("External image not found: {path}"));
        }
        path
    } else {
        let payload = decode_external_image_payload(image_bytes, image_base64)?;
        let bytes = if let Some(bytes) = payload {
            bytes
        } else {
            let url = image_url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    "Missing external image payload: provide imagePath, imageUrl or imageBytes/imageBase64"
                        .to_string()
                })?;
            download_external_image_bytes_from_url(url)?
        };
        let temp_path = create_external_image_temp_path(state, &bytes)?;
        fs::write(&temp_path, &bytes).map_err(|error| {
            format!(
                "Failed to write external image temp file {}: {error}",
                temp_path.display()
            )
        })?;
        temp_query_path = Some(temp_path.clone());
        temp_path.to_string_lossy().to_string()
    };

    let result = (|| -> Result<Vec<String>, String> {
        if mode == "color" {
            return search_gallery_image_ids_by_external_image_color(
                &query_image_path,
                candidate_image_ids.clone(),
                limit,
                state,
            );
        }
        if mode == "atmosphere" {
            return search_gallery_image_ids_by_external_image_atmosphere(
                &query_image_path,
                candidate_image_ids.clone(),
                limit,
                state,
            );
        }

        ensure_clip_vector_cache_loaded(state)?;

        let model_root = resolve_chinese_clip_model_dir(None)?;
        let script_path = resolve_chinese_clip_image_service_script_path()?;
        touch_clip_image_service_last_used(&state.clip_image_encoder_last_used_at);
        let query_embedding = {
            let mut service_guard = state
                .clip_image_encoder_service
                .lock()
                .map_err(|_| "Clip image encoder service is locked".to_string())?;
            run_chinese_clip_image_embedding_via_service_with_recovery(
                &mut service_guard,
                &model_root,
                &script_path,
                &query_image_path,
            )?
        };
        touch_clip_image_service_last_used(&state.clip_image_encoder_last_used_at);
        ensure_clip_image_service_idle_reaper_started(
            &state.clip_image_encoder_service,
            &state.clip_image_encoder_last_used_at,
            &state.clip_image_encoder_release_worker_running,
        );
        if query_embedding.is_empty() {
            return Err("External image embedding is empty".to_string());
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

        let cache_guard = state
            .clip_vector_cache
            .lock()
            .map_err(|_| "Clip vector cache state is locked".to_string())?;
        let cache = cache_guard
            .as_ref()
            .ok_or_else(|| "Clip vector cache not loaded".to_string())?;
        if cache.vectors.is_empty() {
            return Err("请先运行自然语言扫描生成图片向量。".to_string());
        }
        if cache.model_id != CHINESE_CLIP_MODEL_ID || cache.model_version != CHINESE_CLIP_MODEL_VERSION {
            return Ok(Vec::new());
        }
        if cache.dimension != query_embedding.len() {
            return Ok(Vec::new());
        }

        let top_k = limit.unwrap_or(NATURAL_LANGUAGE_SEARCH_DEFAULT_TOP_K).max(1);
        let mut heap = BinaryHeap::<NaturalLanguageSearchHeapEntry>::new();
        for (image_id, vector) in &cache.vectors {
            if let Some(filter) = &candidate_filter {
                if !filter.contains(image_id) {
                    continue;
                }
            }
            if vector.len() != query_embedding.len() {
                continue;
            }
            let score = dot_product(&query_embedding, vector);
            if !score.is_finite() {
                continue;
            }
            heap.push(NaturalLanguageSearchHeapEntry {
                image_id: image_id.clone(),
                score,
            });
            if heap.len() > top_k {
                let _ = heap.pop();
            }
        }

        let mut ranked = Vec::<NaturalLanguageSearchHeapEntry>::with_capacity(heap.len());
        while let Some(entry) = heap.pop() {
            ranked.push(entry);
        }
        ranked.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.image_id.cmp(&b.image_id))
        });
        Ok(ranked.into_iter().map(|entry| entry.image_id).collect())
    })();

    if let Some(path) = temp_query_path {
        let _ = fs::remove_file(path);
    }
    result
}

fn search_gallery_image_ids_by_external_image_atmosphere(
    query_image_path: &str,
    candidate_image_ids: Option<Vec<String>>,
    limit: Option<usize>,
    state: &AppState,
) -> Result<Vec<String>, String> {
    if let Some(ids) = candidate_image_ids.as_ref() {
        let has_candidate = ids.iter().any(|id| !id.trim().is_empty());
        if !has_candidate {
            return Ok(Vec::new());
        }
    }
    ensure_atmosphere_signature_cache_loaded(state)?;
    let query_signature = compute_atmosphere_signature_from_path(query_image_path)?;
    let candidate_filter = candidate_image_ids.map(|list| {
        list.into_iter()
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty())
            .collect::<HashSet<_>>()
    });
    if matches!(candidate_filter, Some(ref set) if set.is_empty()) {
        return Ok(Vec::new());
    }

    let cache_guard = state
        .atmosphere_signature_cache
        .lock()
        .map_err(|_| "Atmosphere signature cache state is locked".to_string())?;
    let cache = cache_guard
        .as_ref()
        .ok_or_else(|| "Atmosphere signature cache not loaded".to_string())?;
    if cache.dimension != ATMOSPHERE_SIGNATURE_DIM {
        return Err("Atmosphere signature cache dimension mismatch".to_string());
    }
    if cache.vectors.is_empty() {
        return Err("请先生成氛围特征。".to_string());
    }

    let top_k = limit.unwrap_or(NATURAL_LANGUAGE_SEARCH_DEFAULT_TOP_K).max(1);
    let mut heap = BinaryHeap::<NaturalLanguageSearchHeapEntry>::new();
    for (image_id, signature) in &cache.vectors {
        if let Some(filter) = &candidate_filter {
            if !filter.contains(image_id) {
                continue;
            }
        }
        let distance = atmosphere_signature_weighted_distance(&query_signature, signature);
        if !distance.is_finite() {
            continue;
        }
        heap.push(NaturalLanguageSearchHeapEntry {
            image_id: image_id.clone(),
            score: -distance,
        });
        if heap.len() > top_k {
            let _ = heap.pop();
        }
    }

    let mut ranked = Vec::<NaturalLanguageSearchHeapEntry>::with_capacity(heap.len());
    while let Some(entry) = heap.pop() {
        ranked.push(entry);
    }
    ranked.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.image_id.cmp(&b.image_id))
    });
    Ok(ranked.into_iter().map(|entry| entry.image_id).collect())
}

fn search_gallery_image_ids_by_external_image_color(
    query_image_path: &str,
    candidate_image_ids: Option<Vec<String>>,
    limit: Option<usize>,
    state: &AppState,
) -> Result<Vec<String>, String> {
    if let Some(ids) = candidate_image_ids.as_ref() {
        let has_candidate = ids.iter().any(|id| !id.trim().is_empty());
        if !has_candidate {
            return Ok(Vec::new());
        }
    }

    ensure_color_signature_cache_loaded(state)?;
    let query_signature = compute_color_signature_from_path(query_image_path)?;
    let candidate_filter = candidate_image_ids.map(|list| {
        list.into_iter()
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty())
            .collect::<HashSet<_>>()
    });
    if matches!(candidate_filter, Some(ref set) if set.is_empty()) {
        return Ok(Vec::new());
    }

    let cache_guard = state
        .color_signature_cache
        .lock()
        .map_err(|_| "Color signature cache state is locked".to_string())?;
    let cache = cache_guard
        .as_ref()
        .ok_or_else(|| "Color signature cache not loaded".to_string())?;
    if cache.dimension != COLOR_SIGNATURE_DIM {
        return Err("Color signature cache dimension mismatch".to_string());
    }
    if cache.vectors.is_empty() {
        return Err("请先在设置中生成配色特征。".to_string());
    }

    let top_k = limit.unwrap_or(NATURAL_LANGUAGE_SEARCH_DEFAULT_TOP_K).max(1);
    let mut heap = BinaryHeap::<NaturalLanguageSearchHeapEntry>::new();
    for (image_id, signature) in &cache.vectors {
        if let Some(filter) = &candidate_filter {
            if !filter.contains(image_id) {
                continue;
            }
        }
        let distance = color_signature_weighted_distance(&query_signature, signature);
        if !distance.is_finite() {
            continue;
        }
        heap.push(NaturalLanguageSearchHeapEntry {
            image_id: image_id.clone(),
            score: -distance,
        });
        if heap.len() > top_k {
            let _ = heap.pop();
        }
    }

    let mut ranked = Vec::<NaturalLanguageSearchHeapEntry>::with_capacity(heap.len());
    while let Some(entry) = heap.pop() {
        ranked.push(entry);
    }
    ranked.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.image_id.cmp(&b.image_id))
    });
    Ok(ranked.into_iter().map(|entry| entry.image_id).collect())
}

fn load_existing_atmosphere_signatures(
    conn: &Connection,
    candidate_image_ids: Option<Vec<String>>,
) -> Result<HashMap<String, Vec<f32>>, String> {
    let mut sql = String::from(
        "
        SELECT s.image_id, s.signature_blob
        FROM image_atmosphere_signatures s
        INNER JOIN images i ON i.id = s.image_id
        WHERE i.source = 'library'
          AND COALESCE(i.trashed, 0) = 0
          AND COALESCE(i.missing, 0) = 0
        ",
    );
    let mut params_list = Vec::<Value>::new();
    if let Some(ids) = candidate_image_ids {
        let normalized = ids
            .into_iter()
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty())
            .collect::<Vec<_>>();
        if normalized.is_empty() {
            return Ok(HashMap::new());
        }
        let placeholders = std::iter::repeat("?")
            .take(normalized.len())
            .collect::<Vec<_>>()
            .join(", ");
        sql.push_str(&format!(" AND s.image_id IN ({placeholders})"));
        for id in normalized {
            params_list.push(Value::from(id));
        }
    }
    sql.push_str(" ORDER BY i.modified_at DESC, s.image_id ASC");

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|error| format!("Failed to load atmosphere signatures: {error}"))?;
    let rows = stmt
        .query_map(params_from_iter(params_list.iter()), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
        })
        .map_err(|error| format!("Failed to load atmosphere signatures: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to load atmosphere signatures: {error}"))?;

    let mut result = HashMap::<String, Vec<f32>>::with_capacity(rows.len());
    for (image_id, blob) in rows {
        if let Ok(vector) = decode_f32_blob(&blob) {
            if vector.len() == ATMOSPHERE_SIGNATURE_DIM {
                result.insert(image_id, vector);
            }
        }
    }
    Ok(result)
}

fn load_existing_color_signatures(
    conn: &Connection,
    candidate_image_ids: Option<Vec<String>>,
) -> Result<HashMap<String, Vec<f32>>, String> {
    let mut sql = String::from(
        "
        SELECT s.image_id, s.signature_blob
        FROM image_color_signatures s
        INNER JOIN images i ON i.id = s.image_id
        WHERE i.source = 'library'
          AND COALESCE(i.trashed, 0) = 0
          AND COALESCE(i.missing, 0) = 0
        ",
    );
    let mut params_list = Vec::<Value>::new();
    if let Some(ids) = candidate_image_ids {
        let normalized = ids
            .into_iter()
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty())
            .collect::<Vec<_>>();
        if normalized.is_empty() {
            return Ok(HashMap::new());
        }
        let placeholders = std::iter::repeat("?")
            .take(normalized.len())
            .collect::<Vec<_>>()
            .join(", ");
        sql.push_str(&format!(" AND s.image_id IN ({placeholders})"));
        for id in normalized {
            params_list.push(Value::from(id));
        }
    }
    sql.push_str(" ORDER BY i.modified_at DESC, s.image_id ASC");

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|error| format!("Failed to load color signatures: {error}"))?;
    let rows = stmt
        .query_map(params_from_iter(params_list.iter()), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
        })
        .map_err(|error| format!("Failed to load color signatures: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to load color signatures: {error}"))?;

    let mut result = HashMap::<String, Vec<f32>>::with_capacity(rows.len());
    for (image_id, blob) in rows {
        if let Ok(vector) = decode_f32_blob(&blob) {
            if vector.len() == COLOR_SIGNATURE_DIM {
                result.insert(image_id, vector);
            }
        }
    }
    Ok(result)
}

fn load_atmosphere_signature_candidates(
    conn: &Connection,
    candidate_image_ids: Option<Vec<String>>,
) -> Result<Vec<AtmosphereSignatureCandidate>, String> {
    let mut sql = String::from(
        "
        SELECT
          i.id,
          i.path,
          t.thumb_path,
          t.source_modified_at,
          t.source_file_size,
          i.modified_at,
          i.file_size
        FROM images i
        LEFT JOIN image_thumbnails t ON t.image_id = i.id
        WHERE i.source = 'library'
          AND COALESCE(i.trashed, 0) = 0
          AND COALESCE(i.missing, 0) = 0
        ",
    );

    let mut params_list = Vec::<Value>::new();
    if let Some(ids) = candidate_image_ids {
        let normalized = ids
            .into_iter()
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty())
            .collect::<Vec<_>>();
        if normalized.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = std::iter::repeat("?")
            .take(normalized.len())
            .collect::<Vec<_>>()
            .join(", ");
        sql.push_str(&format!(" AND i.id IN ({placeholders})"));
        for id in normalized {
            params_list.push(Value::from(id));
        }
    }
    sql.push_str(" ORDER BY i.modified_at DESC, i.id ASC");

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|error| format!("Failed to load atmosphere candidates: {error}"))?;
    let rows = stmt
        .query_map(params_from_iter(params_list.iter()), |row| {
            Ok(AtmosphereSignatureCandidate {
                image_id: row.get(0)?,
                image_path: row.get(1)?,
                thumbnail_path: row.get(2)?,
                thumbnail_source_modified_at: row.get(3)?,
                thumbnail_source_file_size: row.get(4)?,
                modified_at: row.get(5)?,
                file_size: row.get(6)?,
            })
        })
        .map_err(|error| format!("Failed to load atmosphere candidates: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to load atmosphere candidates: {error}"))?;
    Ok(rows)
}

fn compute_atmosphere_signature_from_path(path: &str) -> Result<Vec<f32>, String> {
    let image = ImageReader::open(path)
        .map_err(|error| format!("Failed to open image for atmosphere signature: {error}"))?
        .with_guessed_format()
        .map_err(|error| format!("Failed to detect image format for atmosphere signature: {error}"))?
        .decode()
        .map_err(|error| format!("Failed to decode image for atmosphere signature: {error}"))?;
    let resized = image
        .resize_exact(
            ATMOSPHERE_SIGNATURE_IMAGE_EDGE,
            ATMOSPHERE_SIGNATURE_IMAGE_EDGE,
            FilterType::Triangle,
        )
        .blur(0.7)
        .to_rgb8();

    let edge = ATMOSPHERE_SIGNATURE_IMAGE_EDGE as usize;
    let total = (edge * edge) as f32;
    let mut hue_hist = vec![0f32; ATMOSPHERE_SIGNATURE_HUE_BINS];
    let mut sat_sum = 0f32;
    let mut sat_sq_sum = 0f32;
    let mut val_sum = 0f32;
    let mut val_sq_sum = 0f32;
    let mut dark_count = 0f32;
    let mut highlight_count = 0f32;
    let mut warm_count = 0f32;
    let mut cool_count = 0f32;
    let mut brightness_grid = vec![0f32; 16];
    let mut saturation_grid = vec![0f32; 16];
    let mut cell_counts = vec![0f32; 16];

    for (x, y, pixel) in resized.enumerate_pixels() {
        let r = pixel[0] as f32 / 255.0;
        let g = pixel[1] as f32 / 255.0;
        let b = pixel[2] as f32 / 255.0;

        let max = r.max(g.max(b));
        let min = r.min(g.min(b));
        let delta = max - min;
        let value = max;
        let saturation = if max <= 1e-6 { 0.0 } else { delta / max };
        let hue = if delta <= 1e-6 {
            0.0
        } else if max == r {
            60.0 * (((g - b) / delta).rem_euclid(6.0))
        } else if max == g {
            60.0 * (((b - r) / delta) + 2.0)
        } else {
            60.0 * (((r - g) / delta) + 4.0)
        };

        let mut hue_bin = ((hue / 360.0) * ATMOSPHERE_SIGNATURE_HUE_BINS as f32).floor() as usize;
        if hue_bin >= ATMOSPHERE_SIGNATURE_HUE_BINS {
            hue_bin = ATMOSPHERE_SIGNATURE_HUE_BINS - 1;
        }
        hue_hist[hue_bin] += 1.0;

        sat_sum += saturation;
        sat_sq_sum += saturation * saturation;
        val_sum += value;
        val_sq_sum += value * value;

        if value < 0.22 {
            dark_count += 1.0;
        }
        if value > 0.85 {
            highlight_count += 1.0;
        }
        if (hue <= 70.0 || hue >= 290.0) && saturation > 0.15 {
            warm_count += 1.0;
        } else if hue >= 140.0 && hue <= 260.0 && saturation > 0.15 {
            cool_count += 1.0;
        }

        let cell_x = ((x as usize) * 4) / edge;
        let cell_y = ((y as usize) * 4) / edge;
        let cell_index = cell_y.min(3) * 4 + cell_x.min(3);
        brightness_grid[cell_index] += value;
        saturation_grid[cell_index] += saturation;
        cell_counts[cell_index] += 1.0;
    }

    for count in &mut hue_hist {
        *count /= total.max(1.0);
    }
    for idx in 0..16 {
        let denom = cell_counts[idx].max(1.0);
        brightness_grid[idx] /= denom;
        saturation_grid[idx] /= denom;
    }

    let sat_mean = sat_sum / total.max(1.0);
    let sat_var = (sat_sq_sum / total.max(1.0) - sat_mean * sat_mean).max(0.0);
    let sat_std = sat_var.sqrt();
    let val_mean = val_sum / total.max(1.0);
    let val_var = (val_sq_sum / total.max(1.0) - val_mean * val_mean).max(0.0);
    let val_std = val_var.sqrt();
    let warm_cool_ratio = warm_count / (warm_count + cool_count + 1e-6);

    let mut signature = Vec::<f32>::with_capacity(ATMOSPHERE_SIGNATURE_DIM);
    signature.extend_from_slice(&hue_hist);
    signature.push(sat_mean);
    signature.push(sat_std);
    signature.push(val_mean);
    signature.push(val_std);
    signature.push(dark_count / total.max(1.0));
    signature.push(highlight_count / total.max(1.0));
    signature.push(warm_cool_ratio);
    signature.extend_from_slice(&brightness_grid);
    signature.extend_from_slice(&saturation_grid);
    Ok(signature)
}

fn atmosphere_signature_weighted_distance(query: &[f32], candidate: &[f32]) -> f32 {
    if query.len() != ATMOSPHERE_SIGNATURE_DIM || candidate.len() != ATMOSPHERE_SIGNATURE_DIM {
        return f32::INFINITY;
    }
    let mut sum = 0f32;
    for i in 0..ATMOSPHERE_SIGNATURE_DIM {
        let weight = if i < ATMOSPHERE_SIGNATURE_HUE_BINS {
            2.0
        } else if i < ATMOSPHERE_SIGNATURE_HUE_BINS + 7 {
            1.4
        } else if i < ATMOSPHERE_SIGNATURE_HUE_BINS + 7 + 16 {
            1.1
        } else {
            1.0
        };
        let diff = query[i] - candidate[i];
        sum += weight * diff * diff;
    }
    sum.sqrt()
}

fn compute_color_signature_from_path(path: &str) -> Result<Vec<f32>, String> {
    let image = ImageReader::open(path)
        .map_err(|error| format!("Failed to open image for color signature: {error}"))?
        .with_guessed_format()
        .map_err(|error| format!("Failed to detect image format for color signature: {error}"))?
        .decode()
        .map_err(|error| format!("Failed to decode image for color signature: {error}"))?;
    let resized = image
        .resize_exact(
            ATMOSPHERE_SIGNATURE_IMAGE_EDGE,
            ATMOSPHERE_SIGNATURE_IMAGE_EDGE,
            FilterType::Triangle,
        )
        .blur(0.35)
        .to_rgb8();

    let edge = ATMOSPHERE_SIGNATURE_IMAGE_EDGE as usize;
    let total = (edge * edge) as f32;
    let mut hue_hist = vec![0f32; COLOR_SIGNATURE_HUE_BINS];
    let mut sat_sum = 0f32;
    let mut sat_sq_sum = 0f32;
    let mut val_sum = 0f32;
    let mut val_sq_sum = 0f32;
    let mut dark_count = 0f32;
    let mut highlight_count = 0f32;
    let mut gray_count = 0f32;
    let mut warm_count = 0f32;

    for (_, _, pixel) in resized.enumerate_pixels() {
        let r = pixel[0] as f32 / 255.0;
        let g = pixel[1] as f32 / 255.0;
        let b = pixel[2] as f32 / 255.0;

        let max = r.max(g.max(b));
        let min = r.min(g.min(b));
        let delta = max - min;
        let value = max;
        let saturation = if max <= 1e-6 { 0.0 } else { delta / max };
        let hue = if delta <= 1e-6 {
            0.0
        } else if max == r {
            60.0 * (((g - b) / delta).rem_euclid(6.0))
        } else if max == g {
            60.0 * (((b - r) / delta) + 2.0)
        } else {
            60.0 * (((r - g) / delta) + 4.0)
        };

        let mut hue_bin = ((hue / 360.0) * COLOR_SIGNATURE_HUE_BINS as f32).floor() as usize;
        if hue_bin >= COLOR_SIGNATURE_HUE_BINS {
            hue_bin = COLOR_SIGNATURE_HUE_BINS - 1;
        }
        let hue_weight = 0.35 + saturation * 0.65;
        hue_hist[hue_bin] += hue_weight;

        sat_sum += saturation;
        sat_sq_sum += saturation * saturation;
        val_sum += value;
        val_sq_sum += value * value;

        if value < 0.22 {
            dark_count += 1.0;
        }
        if value > 0.85 {
            highlight_count += 1.0;
        }
        if saturation < 0.15 {
            gray_count += 1.0;
        }
        if (hue <= 70.0 || hue >= 290.0) && saturation > 0.15 {
            warm_count += 1.0;
        }
    }

    let hue_norm: f32 = hue_hist.iter().sum::<f32>().max(1e-6);
    for value in &mut hue_hist {
        *value /= hue_norm;
    }

    let sat_mean = sat_sum / total.max(1.0);
    let sat_var = (sat_sq_sum / total.max(1.0) - sat_mean * sat_mean).max(0.0);
    let sat_std = sat_var.sqrt();
    let val_mean = val_sum / total.max(1.0);
    let val_var = (val_sq_sum / total.max(1.0) - val_mean * val_mean).max(0.0);
    let val_std = val_var.sqrt();

    let mut signature = Vec::<f32>::with_capacity(COLOR_SIGNATURE_DIM);
    signature.extend_from_slice(&hue_hist);
    signature.push(sat_mean);
    signature.push(sat_std);
    signature.push(val_mean);
    signature.push(val_std);
    signature.push(dark_count / total.max(1.0));
    signature.push(highlight_count / total.max(1.0));
    signature.push(gray_count / total.max(1.0));
    signature.push(warm_count / total.max(1.0));
    Ok(signature)
}

fn color_signature_weighted_distance(query: &[f32], candidate: &[f32]) -> f32 {
    if query.len() != COLOR_SIGNATURE_DIM || candidate.len() != COLOR_SIGNATURE_DIM {
        return f32::INFINITY;
    }

    let hue_len = COLOR_SIGNATURE_HUE_BINS;
    let mut chi_sum = 0f32;
    for i in 0..hue_len {
        let q = query[i].max(0.0);
        let c = candidate[i].max(0.0);
        let denom = (q + c).max(1e-6);
        let diff = q - c;
        chi_sum += (diff * diff) / denom;
    }
    let hue_distance = 0.5 * chi_sum;

    let stats_start = hue_len;
    let sat_mean_diff = query[stats_start] - candidate[stats_start];
    let sat_std_diff = query[stats_start + 1] - candidate[stats_start + 1];
    let val_mean_diff = query[stats_start + 2] - candidate[stats_start + 2];
    let val_std_diff = query[stats_start + 3] - candidate[stats_start + 3];
    let dark_diff = query[stats_start + 4] - candidate[stats_start + 4];
    let highlight_diff = query[stats_start + 5] - candidate[stats_start + 5];
    let gray_diff = query[stats_start + 6] - candidate[stats_start + 6];
    let warm_diff = query[stats_start + 7] - candidate[stats_start + 7];

    let stats_distance = (
        1.8 * sat_mean_diff * sat_mean_diff
            + 1.2 * sat_std_diff * sat_std_diff
            + 1.8 * val_mean_diff * val_mean_diff
            + 1.1 * val_std_diff * val_std_diff
            + 1.0 * dark_diff * dark_diff
            + 1.0 * highlight_diff * highlight_diff
            + 1.2 * gray_diff * gray_diff
            + 1.4 * warm_diff * warm_diff
    )
        .sqrt();

    2.4 * hue_distance + 1.0 * stats_distance
}

fn decode_external_image_payload(
    image_bytes: Option<Vec<u8>>,
    image_base64: Option<String>,
) -> Result<Option<Vec<u8>>, String> {
    if let Some(bytes) = image_bytes {
        if !bytes.is_empty() {
            return Ok(Some(bytes));
        }
    }
    let Some(raw_base64) = image_base64 else {
        return Ok(None);
    };
    let trimmed = raw_base64.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let payload = trimmed
        .split_once(',')
        .map(|(_, value)| value)
        .unwrap_or(trimmed);
    BASE64_STANDARD
        .decode(payload)
        .map(Some)
        .map_err(|error| format!("Failed to decode imageBase64: {error}"))
}

fn create_external_image_temp_path(state: &AppState, bytes: &[u8]) -> Result<PathBuf, String> {
    let parent = state
        .database_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let dir = parent.join("tmp").join("external-image-search");
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Failed to create external image temp directory: {error}"))?;
    let seed = format!("{}-{}", now_ms(), bytes.len());
    Ok(dir.join(format!("query-{}.img", stable_hash_hex(&seed))))
}

fn download_external_image_bytes_from_url(url: &str) -> Result<Vec<u8>, String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("Only http/https image URLs are supported".to_string());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|error| format!("Failed to create HTTP client: {error}"))?;
    let response = client
        .get(url)
        .header("User-Agent", "illuTag/0.1")
        .send()
        .map_err(|error| format!("Failed to download image URL: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("Image URL request failed with status {}", response.status()));
    }
    let bytes = response
        .bytes()
        .map_err(|error| format!("Failed to read image URL response: {error}"))?;
    if bytes.is_empty() {
        return Err("Downloaded image URL is empty".to_string());
    }
    Ok(bytes.to_vec())
}

pub fn list_image_auto_tags(
    image_id: String,
    state: &AppState,
) -> Result<ImageAutoTagSummary, String> {
    sync_tag_dictionary_from_source_if_changed(state)?;
    let conn = open_database(&state.database_path)?;
    let mut stmt = conn
        .prepare(
            "
            SELECT
              t.category,
              t.tag_en,
              COALESCE(NULLIF(d.tag_zh, ''), NULLIF(t.tag_zh, ''), t.tag_en) AS tag_zh,
              t.confidence
            FROM image_auto_tags t
            LEFT JOIN tag_dictionary d ON d.tag_en = t.tag_en
            WHERE image_id = ?1
            ORDER BY
              CASE t.category WHEN 'character' THEN 0 WHEN 'general' THEN 1 ELSE 2 END,
              t.confidence DESC,
              t.tag_en COLLATE NOCASE
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

fn normalize_tag_text(value: &str) -> String {
    value.trim().to_string()
}

fn normalize_optional_tag_text(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn upsert_user_custom_tag(conn: &Connection, tag_text: &str) -> Result<(), String> {
    let normalized = tag_text.trim();
    if normalized.is_empty() {
        return Ok(());
    }
    let now = now_ms();
    conn.execute(
        "
        INSERT INTO user_custom_tags (tag_text, created_at, updated_at)
        VALUES (?1, ?2, ?2)
        ON CONFLICT(tag_text) DO UPDATE SET
          updated_at = excluded.updated_at
        ",
        params![normalized, now],
    )
    .map_err(|error| format!("Failed to upsert user custom tag: {error}"))?;
    Ok(())
}

fn load_image_user_tag_summary(conn: &Connection, image_id: &str) -> Result<ImageUserTagSummary, String> {
    let mut custom_stmt = conn
        .prepare(
            "
            SELECT tag_text
            FROM image_user_custom_tags
            WHERE image_id = ?1
            ORDER BY updated_at DESC, tag_text COLLATE NOCASE
            ",
        )
        .map_err(|error| format!("Failed to load custom user tags: {error}"))?;
    let custom_tags = custom_stmt
        .query_map(params![image_id], |row| {
            Ok(ImageUserCustomTag {
                tag_text: row.get(0)?,
            })
        })
        .map_err(|error| format!("Failed to load custom user tags: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to load custom user tags: {error}"))?;

    let mut supplement_stmt = conn
        .prepare(
            "
            SELECT
              s.tag_en,
              COALESCE(NULLIF(d.tag_zh, ''), NULLIF(s.tag_zh, ''), s.tag_en) AS tag_zh
            FROM image_user_supplement_tags s
            LEFT JOIN tag_dictionary d ON d.tag_en = s.tag_en
            WHERE s.image_id = ?1
            ORDER BY s.updated_at DESC, s.tag_en COLLATE NOCASE
            ",
        )
        .map_err(|error| format!("Failed to load supplement user tags: {error}"))?;
    let supplement_tags = supplement_stmt
        .query_map(params![image_id], |row| {
            Ok(ImageUserSupplementTag {
                tag_en: row.get(0)?,
                tag_zh: row.get(1)?,
            })
        })
        .map_err(|error| format!("Failed to load supplement user tags: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to load supplement user tags: {error}"))?;

    Ok(ImageUserTagSummary {
        image_id: image_id.to_string(),
        custom_tags,
        supplement_tags,
    })
}

fn refresh_library_cache_best_effort(state: &AppState, conn: &Connection) {
    let Ok(store) = load_store(conn) else {
        return;
    };
    if let Ok(mut cache) = state.library.lock() {
        *cache = Some(store);
    }
}

pub fn list_image_user_tags(image_id: String, state: &AppState) -> Result<ImageUserTagSummary, String> {
    sync_tag_dictionary_from_source_if_changed(state)?;
    let conn = open_database(&state.database_path)?;
    load_image_user_tag_summary(&conn, &image_id)
}

pub fn add_image_user_custom_tag(
    image_id: String,
    tag_text: String,
    state: &AppState,
) -> Result<ImageUserTagSummary, String> {
    let normalized = normalize_tag_text(&tag_text);
    if normalized.is_empty() {
        return Err("Tag text cannot be empty".to_string());
    }
    let conn = open_database(&state.database_path)?;
    let now = now_ms();
    conn.execute(
        "
        INSERT INTO image_user_custom_tags (
          image_id, tag_text, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?3)
        ON CONFLICT(image_id, tag_text) DO UPDATE SET
          updated_at = excluded.updated_at
        ",
        params![image_id, normalized, now],
    )
    .map_err(|error| format!("Failed to add custom user tag: {error}"))?;
    upsert_user_custom_tag(&conn, &normalized)?;
    apply_matching_user_folder_rules_for_image(&conn, &image_id)?;
    refresh_library_cache_best_effort(state, &conn);
    load_image_user_tag_summary(&conn, &image_id)
}

pub fn remove_image_user_custom_tag(
    image_id: String,
    tag_text: String,
    state: &AppState,
) -> Result<ImageUserTagSummary, String> {
    let normalized = normalize_tag_text(&tag_text);
    if normalized.is_empty() {
        return Err("Tag text cannot be empty".to_string());
    }
    let conn = open_database(&state.database_path)?;
    conn.execute(
        "DELETE FROM image_user_custom_tags WHERE image_id = ?1 AND tag_text = ?2",
        params![image_id, normalized],
    )
    .map_err(|error| format!("Failed to remove custom user tag: {error}"))?;
    load_image_user_tag_summary(&conn, &image_id)
}

pub fn add_image_user_supplement_tag(
    image_id: String,
    tag_en: String,
    tag_zh: Option<String>,
    state: &AppState,
) -> Result<ImageUserTagSummary, String> {
    let normalized_tag_en = normalize_tag_text(&tag_en);
    if normalized_tag_en.is_empty() {
        return Err("Tag text cannot be empty".to_string());
    }
    let normalized_tag_zh = normalize_optional_tag_text(tag_zh);
    let conn = open_database(&state.database_path)?;
    let now = now_ms();
    conn.execute(
        "
        INSERT INTO image_user_supplement_tags (
          image_id, tag_en, tag_zh, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?4)
        ON CONFLICT(image_id, tag_en) DO UPDATE SET
          tag_zh = COALESCE(excluded.tag_zh, image_user_supplement_tags.tag_zh),
          updated_at = excluded.updated_at
        ",
        params![image_id, normalized_tag_en, normalized_tag_zh, now],
    )
    .map_err(|error| format!("Failed to add supplement user tag: {error}"))?;
    apply_matching_user_folder_rules_for_image(&conn, &image_id)?;
    refresh_library_cache_best_effort(state, &conn);
    load_image_user_tag_summary(&conn, &image_id)
}

pub fn remove_image_user_supplement_tag(
    image_id: String,
    tag_en: String,
    state: &AppState,
) -> Result<ImageUserTagSummary, String> {
    let normalized_tag_en = normalize_tag_text(&tag_en);
    if normalized_tag_en.is_empty() {
        return Err("Tag text cannot be empty".to_string());
    }
    let conn = open_database(&state.database_path)?;
    conn.execute(
        "DELETE FROM image_user_supplement_tags WHERE image_id = ?1 AND tag_en = ?2",
        params![image_id, normalized_tag_en],
    )
    .map_err(|error| format!("Failed to remove supplement user tag: {error}"))?;
    load_image_user_tag_summary(&conn, &image_id)
}

fn cleanup_orphan_user_tag_folder_members(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "
        DELETE FROM user_tag_folder_members
        WHERE tag_text NOT IN (
          SELECT tag_text FROM user_custom_tags
          UNION
          SELECT DISTINCT tag_text FROM image_user_custom_tags
        )
        ",
        [],
    )
    .map_err(|error| format!("Failed to cleanup orphan user tag members: {error}"))?;
    Ok(())
}

fn load_tag_management_state(conn: &Connection) -> Result<TagManagementState, String> {
    cleanup_orphan_user_tag_folder_members(conn)?;

    let mut folder_stmt = conn
        .prepare(
            "
            SELECT id, name, sort_order
            FROM user_tag_folders
            ORDER BY sort_order ASC, id ASC
            ",
        )
        .map_err(|error| format!("Failed to prepare user tag folders query: {error}"))?;

    let folder_rows = folder_stmt
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?))
        })
        .map_err(|error| format!("Failed to query user tag folders: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to query user tag folders: {error}"))?;

    let mut tag_stmt = conn
        .prepare(
            "
            SELECT m.tag_text
            FROM user_tag_folder_members m
            JOIN (
              SELECT tag_text FROM user_custom_tags
              UNION
              SELECT DISTINCT tag_text FROM image_user_custom_tags
            ) c ON c.tag_text = m.tag_text
            WHERE m.folder_id = ?1
            ORDER BY m.tag_text COLLATE NOCASE
            ",
        )
        .map_err(|error| format!("Failed to prepare user tag folder tags query: {error}"))?;

    let mut folders = Vec::<UserTagFolder>::with_capacity(folder_rows.len());
    for (id, name, sort_order) in folder_rows {
        let tags = tag_stmt
            .query_map(params![id], |row| row.get::<_, String>(0))
            .map_err(|error| format!("Failed to query user tag folder tags: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Failed to query user tag folder tags: {error}"))?;
        folders.push(UserTagFolder {
            id,
            name,
            sort_order,
            tags,
        });
    }

    let mut unclassified_stmt = conn
        .prepare(
            "
            SELECT c.tag_text
            FROM (
              SELECT tag_text FROM user_custom_tags
              UNION
              SELECT DISTINCT tag_text FROM image_user_custom_tags
            ) c
            LEFT JOIN user_tag_folder_members m ON m.tag_text = c.tag_text
            WHERE m.tag_text IS NULL
            ORDER BY c.tag_text COLLATE NOCASE
            ",
        )
        .map_err(|error| format!("Failed to prepare unclassified user tags query: {error}"))?;
    let unclassified_tags = unclassified_stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| format!("Failed to query unclassified user tags: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to query unclassified user tags: {error}"))?;

    Ok(TagManagementState {
        folders,
        unclassified_tags,
    })
}

pub fn list_tag_management_state(state: &AppState) -> Result<TagManagementState, String> {
    let conn = open_database(&state.database_path)?;
    load_tag_management_state(&conn)
}

pub fn create_user_tag_folder(name: String, state: &AppState) -> Result<TagManagementState, String> {
    let normalized_name = name.trim().to_string();
    if normalized_name.is_empty() {
        return Err("Folder name cannot be empty".to_string());
    }
    let conn = open_database(&state.database_path)?;
    let now = now_ms();
    let sort_order = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM user_tag_folders",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("Failed to query user tag folder sort order: {error}"))?;
    conn.execute(
        "
        INSERT INTO user_tag_folders (name, sort_order, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?3)
        ",
        params![normalized_name, sort_order, now],
    )
    .map_err(|error| format!("Failed to create user tag folder: {error}"))?;
    load_tag_management_state(&conn)
}

pub fn create_user_custom_tag(tag_text: String, state: &AppState) -> Result<TagManagementState, String> {
    let normalized_tag = tag_text.trim().to_string();
    if normalized_tag.is_empty() {
        return Err("Tag cannot be empty".to_string());
    }
    let conn = open_database(&state.database_path)?;
    upsert_user_custom_tag(&conn, &normalized_tag)?;
    load_tag_management_state(&conn)
}

pub fn delete_user_custom_tag(tag_text: String, state: &AppState) -> Result<TagManagementState, String> {
    let normalized_tag = tag_text.trim().to_string();
    if normalized_tag.is_empty() {
        return Err("Tag cannot be empty".to_string());
    }
    let mut conn = open_database(&state.database_path)?;
    let tx = conn
        .transaction()
        .map_err(|error| format!("Failed to open user custom tag delete transaction: {error}"))?;
    tx.execute(
        "DELETE FROM image_user_custom_tags WHERE tag_text = ?1",
        params![normalized_tag.clone()],
    )
    .map_err(|error| format!("Failed to delete image custom tag rows: {error}"))?;
    tx.execute(
        "DELETE FROM user_tag_folder_members WHERE tag_text = ?1",
        params![normalized_tag.clone()],
    )
    .map_err(|error| format!("Failed to delete tag folder member rows: {error}"))?;
    tx.execute(
        "DELETE FROM user_custom_tags WHERE tag_text = ?1",
        params![normalized_tag],
    )
    .map_err(|error| format!("Failed to delete user custom tag row: {error}"))?;
    tx.commit()
        .map_err(|error| format!("Failed to commit user custom tag delete transaction: {error}"))?;
    load_tag_management_state(&conn)
}

pub fn rename_user_tag_folder(
    folder_id: i64,
    name: String,
    state: &AppState,
) -> Result<TagManagementState, String> {
    let normalized_name = name.trim().to_string();
    if normalized_name.is_empty() {
        return Err("Folder name cannot be empty".to_string());
    }
    let conn = open_database(&state.database_path)?;
    let now = now_ms();
    let updated = conn
        .execute(
            "
            UPDATE user_tag_folders
            SET name = ?1, updated_at = ?2
            WHERE id = ?3
            ",
            params![normalized_name, now, folder_id],
        )
        .map_err(|error| format!("Failed to rename user tag folder: {error}"))?;
    if updated == 0 {
        return Err("User tag folder not found".to_string());
    }
    load_tag_management_state(&conn)
}

pub fn delete_user_tag_folder(folder_id: i64, state: &AppState) -> Result<TagManagementState, String> {
    let conn = open_database(&state.database_path)?;
    let deleted = conn
        .execute("DELETE FROM user_tag_folders WHERE id = ?1", params![folder_id])
        .map_err(|error| format!("Failed to delete user tag folder: {error}"))?;
    if deleted == 0 {
        return Err("User tag folder not found".to_string());
    }
    load_tag_management_state(&conn)
}

pub fn assign_user_tag_to_folder(
    folder_id: i64,
    tag_text: String,
    state: &AppState,
) -> Result<TagManagementState, String> {
    let normalized_tag = tag_text.trim().to_string();
    if normalized_tag.is_empty() {
        return Err("Tag cannot be empty".to_string());
    }
    let conn = open_database(&state.database_path)?;
    let folder_exists = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM user_tag_folders WHERE id = ?1)",
            params![folder_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("Failed to verify user tag folder: {error}"))?;
    if folder_exists == 0 {
        return Err("User tag folder not found".to_string());
    }
    let tag_exists = conn
        .query_row(
            "
            SELECT EXISTS(
              SELECT 1 FROM user_custom_tags WHERE tag_text = ?1
              UNION
              SELECT 1 FROM image_user_custom_tags WHERE tag_text = ?1
            )
            ",
            params![normalized_tag.clone()],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("Failed to verify user tag source: {error}"))?;
    if tag_exists == 0 {
        return Err("Tag not found in custom tags".to_string());
    }
    let now = now_ms();
    conn.execute(
        "
        INSERT INTO user_tag_folder_members (folder_id, tag_text, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?3)
        ON CONFLICT(tag_text) DO UPDATE SET
          folder_id = excluded.folder_id,
          updated_at = excluded.updated_at
        ",
        params![folder_id, normalized_tag, now],
    )
    .map_err(|error| format!("Failed to assign user tag to folder: {error}"))?;
    load_tag_management_state(&conn)
}

pub fn unassign_user_tag_from_folder(tag_text: String, state: &AppState) -> Result<TagManagementState, String> {
    let normalized_tag = tag_text.trim().to_string();
    if normalized_tag.is_empty() {
        return Err("Tag cannot be empty".to_string());
    }
    let conn = open_database(&state.database_path)?;
    conn.execute(
        "DELETE FROM user_tag_folder_members WHERE tag_text = ?1",
        params![normalized_tag],
    )
    .map_err(|error| format!("Failed to unassign user tag from folder: {error}"))?;
    load_tag_management_state(&conn)
}

pub fn suggest_known_auto_tags(
    query: String,
    limit: Option<i64>,
    include_user_custom: Option<bool>,
    include_dictionary: Option<bool>,
    state: &AppState,
) -> Result<Vec<KnownAutoTagSuggestion>, String> {
    sync_tag_dictionary_from_source_if_changed(state)?;
    let keyword = query.trim();
    if keyword.is_empty() {
        return Ok(Vec::new());
    }
    let conn = open_database(&state.database_path)?;
    let keyword_lower = keyword.to_lowercase();
    let like = format!("%{}%", escape_like_pattern(&keyword_lower));
    let like_prefix = format!("{}%", escape_like_pattern(&keyword_lower));
    let limit = limit.unwrap_or(20).clamp(1, 80);
    let include_user_custom = include_user_custom.unwrap_or(false);
    let include_dictionary = include_dictionary.unwrap_or(false);
    let mut stmt = conn
        .prepare(
            "
            WITH custom_tag_candidates AS (
              SELECT tag_text FROM user_custom_tags
              UNION
              SELECT tag_text FROM image_user_custom_tags
            ),
            custom_tag_counts AS (
              SELECT
                iuct.tag_text,
                COUNT(DISTINCT iuct.image_id) AS image_count
              FROM image_user_custom_tags iuct
              JOIN images i ON i.id = iuct.image_id
              WHERE i.source = 'library'
                AND COALESCE(i.trashed, 0) = 0
              GROUP BY iuct.tag_text
            ),
            dictionary_candidates AS (
              SELECT
                d.tag_en AS tag_en,
                d.tag_zh AS tag_zh
              FROM tag_dictionary d
              WHERE ?6 = 1
            ),
            candidates AS (
            SELECT
              k.tag_en AS tag_en,
              COALESCE(NULLIF(d.tag_zh, ''), NULLIF(k.tag_zh, ''), k.tag_en) AS tag_zh,
              k.image_count AS image_count,
              0 AS is_user_custom
            FROM known_image_tags k
            LEFT JOIN tag_dictionary d ON d.tag_en = k.tag_en
            WHERE k.model_name = ?1
              AND k.image_count > 0
            UNION ALL
            SELECT
              ct.tag_text AS tag_en,
              ct.tag_text AS tag_zh,
              COALESCE(cc.image_count, 0) AS image_count,
              1 AS is_user_custom
            FROM custom_tag_candidates ct
            LEFT JOIN custom_tag_counts cc ON cc.tag_text = ct.tag_text
            WHERE ?5 = 1
            UNION ALL
            SELECT
              dc.tag_en AS tag_en,
              dc.tag_zh AS tag_zh,
              0 AS image_count,
              0 AS is_user_custom
            FROM dictionary_candidates dc
            ),
            filtered AS (
              SELECT * FROM candidates
              WHERE LOWER(tag_en) LIKE ?2 ESCAPE '\\'
                 OR LOWER(COALESCE(tag_zh, '')) LIKE ?2 ESCAPE '\\'
            ),
            deduped AS (
              SELECT
                tag_en,
                MAX(COALESCE(tag_zh, '')) AS tag_zh,
                MAX(image_count) AS image_count,
                MAX(is_user_custom) AS is_user_custom
              FROM filtered
              GROUP BY tag_en
            )
            SELECT
              tag_en,
              NULLIF(tag_zh, '') AS tag_zh,
              image_count,
              is_user_custom
            FROM deduped
            ORDER BY
              CASE WHEN is_user_custom = 1 THEN 0 ELSE 1 END,
              CASE
                WHEN LOWER(COALESCE(tag_zh, '')) LIKE ?3 ESCAPE '\\' THEN 0
                WHEN LOWER(tag_en) LIKE ?3 ESCAPE '\\' THEN 1
                ELSE 2
              END,
              image_count DESC,
              tag_en COLLATE NOCASE
            LIMIT ?4
            ",
        )
        .map_err(|error| format!("Failed to prepare known tag suggestion query: {error}"))?;

    let rows = stmt
        .query_map(
            params![
                WD_TAGGER_MODEL_NAME,
                like,
                like_prefix,
                limit,
                if include_user_custom { 1 } else { 0 },
                if include_dictionary { 1 } else { 0 }
            ],
            |row| {
            Ok(KnownAutoTagSuggestion {
                tag_en: row.get(0)?,
                tag_zh: row.get(1)?,
                image_count: row.get(2)?,
                is_user_custom: row.get::<_, i64>(3)? != 0,
            })
        },
        )
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
              UNION
              SELECT 1
              FROM image_user_supplement_tags ust
              WHERE ust.image_id = images.id
                AND ust.tag_en = ?
              UNION
              SELECT 1
              FROM image_user_custom_tags uct
              WHERE uct.image_id = images.id
                AND uct.tag_text = ?
            )
            ",
        );
        params_values.push(Value::Text(WD_TAGGER_MODEL_NAME.to_string()));
        params_values.push(Value::Text(tag_en.clone()));
        params_values.push(Value::Real(confidence_min as f64));
        params_values.push(Value::Real(confidence_max as f64));
        params_values.push(Value::Text(tag_en.clone()));
        params_values.push(Value::Text(tag_en));
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
              UNION
              SELECT 1
              FROM image_user_supplement_tags ust
              WHERE ust.image_id = images.id
                AND LOWER(REPLACE(ust.tag_en, '_', ' ')) LIKE ? ESCAPE '\\'
            )
            ",
        );
        params_values.push(Value::Text(WD_TAGGER_MODEL_NAME.to_string()));
        let token_like = format!("%{}%", escape_like_pattern(&token));
        params_values.push(Value::Text(token_like.clone()));
        params_values.push(Value::Real(confidence_min as f64));
        params_values.push(Value::Real(confidence_max as f64));
        params_values.push(Value::Text(token_like));
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

fn normalize_user_folder_rule_logic(logic: &str) -> Option<&'static str> {
    match logic.trim().to_uppercase().as_str() {
        "AND" => Some("AND"),
        "OR" => Some("OR"),
        "NOT" => Some("NOT"),
        _ => None,
    }
}

fn normalize_user_folder_rule_source(source: &str) -> Option<&'static str> {
    match source.trim().to_lowercase().as_str() {
        "danbooru" => Some("danbooru"),
        "custom" => Some("custom"),
        "filename" => Some("filename"),
        _ => None,
    }
}

fn normalize_user_folder_rule_conditions(
    conditions: Vec<UserFolderRuleCondition>,
) -> Result<Vec<UserFolderRuleCondition>, String> {
    let mut normalized = Vec::<UserFolderRuleCondition>::with_capacity(conditions.len());
    for condition in conditions {
        let logic = normalize_user_folder_rule_logic(&condition.logic)
            .ok_or_else(|| format!("Unsupported rule logic: {}", condition.logic))?;
        let source = normalize_user_folder_rule_source(&condition.source)
            .ok_or_else(|| format!("Unsupported rule source: {}", condition.source))?;
        let keyword = condition.keyword.trim().to_string();
        if keyword.is_empty() {
            return Err("Rule keyword cannot be empty".to_string());
        }
        normalized.push(UserFolderRuleCondition {
            logic: logic.to_string(),
            source: source.to_string(),
            keyword,
        });
    }
    Ok(normalized)
}

fn user_folder_exists(conn: &Connection, folder_id: i64) -> Result<bool, String> {
    let exists: i64 = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM user_folders WHERE id = ?1)",
            params![folder_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("Failed to query user folder: {error}"))?;
    Ok(exists != 0)
}

fn user_folder_is_leaf(conn: &Connection, folder_id: i64) -> Result<bool, String> {
    let has_children: i64 = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM user_folders WHERE parent_id = ?1)",
            params![folder_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("Failed to query user folder children: {error}"))?;
    Ok(has_children == 0)
}

fn load_user_folder_rule(
    conn: &Connection,
    folder_id: i64,
) -> Result<Option<UserFolderRule>, String> {
    let row = conn
        .query_row(
            "
            SELECT rule_json, updated_at
            FROM user_folder_rules
            WHERE folder_id = ?1
            ",
            params![folder_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(|error| format!("Failed to load user folder rule: {error}"))?;
    let Some((rule_json, updated_at)) = row else {
        return Ok(None);
    };
    let parsed: Vec<UserFolderRuleCondition> = serde_json::from_str(&rule_json)
        .map_err(|error| format!("Failed to parse stored user folder rule: {error}"))?;
    Ok(Some(UserFolderRule {
        folder_id,
        conditions: normalize_user_folder_rule_conditions(parsed)?,
        updated_at,
    }))
}

#[derive(Debug, Clone)]
struct ImageFolderRuleMatchContext {
    file_name_lower: String,
    danbooru_terms: HashSet<String>,
    custom_terms: HashSet<String>,
}

fn load_image_folder_rule_match_context(
    conn: &Connection,
    image_id: &str,
) -> Result<Option<ImageFolderRuleMatchContext>, String> {
    let image_row = conn
        .query_row(
            "
            SELECT file_name, source, COALESCE(trashed, 0)
            FROM images
            WHERE id = ?1
            ",
            params![image_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|error| format!("Failed to load image info for folder rule: {error}"))?;
    let Some((file_name, source, trashed)) = image_row else {
        return Ok(None);
    };
    if source != "library" || trashed != 0 {
        return Ok(None);
    }

    let mut danbooru_terms = HashSet::<String>::new();
    {
        let mut stmt = conn
            .prepare(
                "
                SELECT tag_en, COALESCE(tag_zh, '')
                FROM image_auto_tags
                WHERE image_id = ?1
                ",
            )
            .map_err(|error| format!("Failed to prepare auto tag query for folder rule: {error}"))?;
        let rows = stmt
            .query_map(params![image_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| format!("Failed to query auto tags for folder rule: {error}"))?;
        for row in rows {
            let (tag_en, tag_zh) =
                row.map_err(|error| format!("Failed to query auto tags for folder rule: {error}"))?;
            let normalized_en = tag_en.trim().to_lowercase();
            if !normalized_en.is_empty() {
                danbooru_terms.insert(normalized_en);
            }
            let normalized_zh = tag_zh.trim().to_lowercase();
            if !normalized_zh.is_empty() {
                danbooru_terms.insert(normalized_zh);
            }
        }
    }
    {
        let mut stmt = conn
            .prepare(
                "
                SELECT tag_en, COALESCE(tag_zh, '')
                FROM image_user_supplement_tags
                WHERE image_id = ?1
                ",
            )
            .map_err(|error| format!("Failed to prepare supplement tag query for folder rule: {error}"))?;
        let rows = stmt
            .query_map(params![image_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| format!("Failed to query supplement tags for folder rule: {error}"))?;
        for row in rows {
            let (tag_en, tag_zh) = row
                .map_err(|error| format!("Failed to query supplement tags for folder rule: {error}"))?;
            let normalized_en = tag_en.trim().to_lowercase();
            if !normalized_en.is_empty() {
                danbooru_terms.insert(normalized_en);
            }
            let normalized_zh = tag_zh.trim().to_lowercase();
            if !normalized_zh.is_empty() {
                danbooru_terms.insert(normalized_zh);
            }
        }
    }

    let mut custom_terms = HashSet::<String>::new();
    {
        let mut stmt = conn
            .prepare(
                "
                SELECT tag_text
                FROM image_user_custom_tags
                WHERE image_id = ?1
                ",
            )
            .map_err(|error| format!("Failed to prepare custom tag query for folder rule: {error}"))?;
        let rows = stmt
            .query_map(params![image_id], |row| row.get::<_, String>(0))
            .map_err(|error| format!("Failed to query custom tags for folder rule: {error}"))?;
        for row in rows {
            let tag_text =
                row.map_err(|error| format!("Failed to query custom tags for folder rule: {error}"))?;
            let normalized = tag_text.trim().to_lowercase();
            if !normalized.is_empty() {
                custom_terms.insert(normalized);
            }
        }
    }

    Ok(Some(ImageFolderRuleMatchContext {
        file_name_lower: file_name.to_lowercase(),
        danbooru_terms,
        custom_terms,
    }))
}

fn evaluate_user_folder_rule_condition(
    context: &ImageFolderRuleMatchContext,
    condition: &UserFolderRuleCondition,
) -> bool {
    let keyword = condition.keyword.trim().to_lowercase();
    if keyword.is_empty() {
        return false;
    }
    match condition.source.as_str() {
        "danbooru" => context.danbooru_terms.contains(&keyword),
        "custom" => context.custom_terms.contains(&keyword),
        "filename" => context.file_name_lower.contains(&keyword),
        _ => false,
    }
}

fn evaluate_user_folder_rule_conditions(
    context: &ImageFolderRuleMatchContext,
    conditions: &[UserFolderRuleCondition],
) -> bool {
    if conditions.is_empty() {
        return false;
    }
    let first = evaluate_user_folder_rule_condition(context, &conditions[0]);
    let mut result = if conditions[0].logic == "NOT" {
        !first
    } else {
        first
    };
    for condition in conditions.iter().skip(1) {
        let matched = evaluate_user_folder_rule_condition(context, condition);
        match condition.logic.as_str() {
            "AND" => result = result && matched,
            "OR" => result = result || matched,
            "NOT" => result = result && !matched,
            _ => {}
        }
    }
    result
}

fn load_active_leaf_user_folder_rules(
    conn: &Connection,
) -> Result<Vec<(i64, Vec<UserFolderRuleCondition>)>, String> {
    let mut stmt = conn
        .prepare(
            "
            SELECT r.folder_id, r.rule_json
            FROM user_folder_rules r
            JOIN user_folders f ON f.id = r.folder_id
            WHERE NOT EXISTS (
              SELECT 1
              FROM user_folders c
              WHERE c.parent_id = f.id
            )
            ",
        )
        .map_err(|error| format!("Failed to prepare user folder rules query: {error}"))?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))
        .map_err(|error| format!("Failed to query user folder rules: {error}"))?;
    let mut rules = Vec::<(i64, Vec<UserFolderRuleCondition>)>::new();
    for row in rows {
        let (folder_id, rule_json) =
            row.map_err(|error| format!("Failed to query user folder rules: {error}"))?;
        let parsed = match serde_json::from_str::<Vec<UserFolderRuleCondition>>(&rule_json) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("[folder-rule] skip invalid rule json for folder {folder_id}: {error}");
                continue;
            }
        };
        let normalized = match normalize_user_folder_rule_conditions(parsed) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("[folder-rule] skip invalid rule for folder {folder_id}: {error}");
                continue;
            }
        };
        if normalized.is_empty() {
            continue;
        }
        rules.push((folder_id, normalized));
    }
    Ok(rules)
}

fn apply_matching_user_folder_rules_for_image(
    conn: &Connection,
    image_id: &str,
) -> Result<usize, String> {
    let Some(context) = load_image_folder_rule_match_context(conn, image_id)? else {
        return Ok(0);
    };
    let rules = load_active_leaf_user_folder_rules(conn)?;
    if rules.is_empty() {
        return Ok(0);
    }
    let now = now_ms();
    let mut assigned = 0usize;
    for (folder_id, conditions) in rules {
        if !evaluate_user_folder_rule_conditions(&context, &conditions) {
            continue;
        }
        let affected = conn
            .execute(
                "
                INSERT OR IGNORE INTO image_user_folders (image_id, folder_id, assigned_at)
                VALUES (?1, ?2, ?3)
                ",
                params![image_id, folder_id, now],
            )
            .map_err(|error| format!("Failed to apply folder rule assignment: {error}"))?;
        assigned += affected;
    }
    Ok(assigned)
}

fn apply_user_folder_rule_to_library_images(
    conn: &Connection,
    folder_id: i64,
    conditions: &[UserFolderRuleCondition],
) -> Result<usize, String> {
    if conditions.is_empty() {
        return Ok(0);
    }
    let mut stmt = conn
        .prepare(
            "
            SELECT id
            FROM images
            WHERE source = 'library'
              AND COALESCE(trashed, 0) = 0
            ",
        )
        .map_err(|error| format!("Failed to prepare folder rule apply candidates: {error}"))?;
    let image_ids = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| format!("Failed to query folder rule apply candidates: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to query folder rule apply candidates: {error}"))?;
    let now = now_ms();
    let mut assigned = 0usize;
    for image_id in image_ids {
        let Some(context) = load_image_folder_rule_match_context(conn, &image_id)? else {
            continue;
        };
        if !evaluate_user_folder_rule_conditions(&context, conditions) {
            continue;
        }
        let affected = conn
            .execute(
                "
                INSERT OR IGNORE INTO image_user_folders (image_id, folder_id, assigned_at)
                VALUES (?1, ?2, ?3)
                ",
                params![image_id, folder_id, now],
            )
            .map_err(|error| format!("Failed to apply folder rule assignment: {error}"))?;
        assigned += affected;
    }
    Ok(assigned)
}

pub fn get_user_folder_rule(folder_id: i64, state: &AppState) -> Result<Option<UserFolderRule>, String> {
    let conn = open_database(&state.database_path)?;
    if !user_folder_exists(&conn, folder_id)? {
        return Err("User folder not found".to_string());
    }
    load_user_folder_rule(&conn, folder_id)
}

pub fn save_user_folder_rule(
    folder_id: i64,
    conditions: Vec<UserFolderRuleCondition>,
    apply_now: bool,
    state: &AppState,
) -> Result<LibraryStore, String> {
    let mut library = state
        .library
        .lock()
        .map_err(|_| "Library state is locked".to_string())?;
    let mut conn = open_database(&state.database_path)?;
    if !user_folder_exists(&conn, folder_id)? {
        return Err("User folder not found".to_string());
    }
    if !user_folder_is_leaf(&conn, folder_id)? {
        return Err("Only leaf folders can have rules".to_string());
    }

    let normalized = normalize_user_folder_rule_conditions(conditions)?;
    let now = now_ms();
    let tx = conn
        .transaction()
        .map_err(|error| format!("Failed to open folder rule transaction: {error}"))?;
    if normalized.is_empty() {
        tx.execute(
            "DELETE FROM user_folder_rules WHERE folder_id = ?1",
            params![folder_id],
        )
        .map_err(|error| format!("Failed to clear folder rule: {error}"))?;
    } else {
        let rule_json = serde_json::to_string(&normalized)
            .map_err(|error| format!("Failed to serialize folder rule: {error}"))?;
        tx.execute(
            "
            INSERT INTO user_folder_rules (folder_id, rule_json, updated_at)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(folder_id) DO UPDATE SET
              rule_json = excluded.rule_json,
              updated_at = excluded.updated_at
            ",
            params![folder_id, rule_json, now],
        )
        .map_err(|error| format!("Failed to save folder rule: {error}"))?;
        if apply_now {
            apply_user_folder_rule_to_library_images(&tx, folder_id, &normalized)?;
        }
    }
    tx.commit()
        .map_err(|error| format!("Failed to commit folder rule transaction: {error}"))?;

    let store = load_store(&conn)?;
    *library = Some(store.clone());
    Ok(store)
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
          board_id, image_id, x, y, width, height, rotation, flip_x, flip_y, z_index, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 0, 0, ?7, ?8)
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
          board_id, image_id, x, y, width, height, rotation, flip_x, flip_y, z_index, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 0, 0, ?7, ?8)
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
    flip_x: bool,
    flip_y: bool,
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
        SET x = ?1, y = ?2, width = ?3, height = ?4, rotation = ?5, flip_x = ?6, flip_y = ?7
        WHERE id = ?8
        ",
        params![
            x,
            y,
            width.max(48.0),
            height.max(48.0),
            rotation,
            if flip_x { 1 } else { 0 },
            if flip_y { 1 } else { 0 },
            item_id
        ],
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
          board_id, image_id, x, y, width, height, rotation, flip_x, flip_y, z_index, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        ",
        params![
            item.board_id,
            item.image_id,
            x.unwrap_or(item.x + 28.0),
            y.unwrap_or(item.y + 28.0),
            item.width,
            item.height,
            item.rotation,
            if item.flip_x { 1 } else { 0 },
            if item.flip_y { 1 } else { 0 },
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
    flip_x: bool,
    flip_y: bool,
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
          board_id, image_id, x, y, width, height, rotation, flip_x, flip_y, z_index, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        ",
        params![
            board_id,
            image_id,
            x,
            y,
            width.max(48.0),
            height.max(48.0),
            rotation,
            if flip_x { 1 } else { 0 },
            if flip_y { 1 } else { 0 },
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
    conn.busy_timeout(std::time::Duration::from_millis(5000))
        .map_err(|error| format!("Failed to set SQLite busy_timeout: {error}"))?;
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
          is_favorite INTEGER NOT NULL DEFAULT 0,
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
          source_kind TEXT NOT NULL DEFAULT 'manual',
          source_path TEXT,
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

        CREATE TABLE IF NOT EXISTS user_folder_rules (
          folder_id INTEGER PRIMARY KEY,
          rule_json TEXT NOT NULL,
          updated_at INTEGER NOT NULL,
          FOREIGN KEY(folder_id) REFERENCES user_folders(id) ON DELETE CASCADE
        );

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

        CREATE TABLE IF NOT EXISTS app_meta (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL,
          updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS image_user_custom_tags (
          image_id TEXT NOT NULL,
          tag_text TEXT NOT NULL,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL,
          PRIMARY KEY(image_id, tag_text),
          FOREIGN KEY(image_id) REFERENCES images(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_image_user_custom_tags_image_id
          ON image_user_custom_tags(image_id, updated_at DESC);

        CREATE TABLE IF NOT EXISTS image_user_supplement_tags (
          image_id TEXT NOT NULL,
          tag_en TEXT NOT NULL,
          tag_zh TEXT,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL,
          PRIMARY KEY(image_id, tag_en),
          FOREIGN KEY(image_id) REFERENCES images(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_image_user_supplement_tags_image_id
          ON image_user_supplement_tags(image_id, updated_at DESC);

        CREATE TABLE IF NOT EXISTS user_custom_tags (
          tag_text TEXT PRIMARY KEY,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS user_tag_folders (
          id INTEGER PRIMARY KEY,
          name TEXT NOT NULL,
          sort_order INTEGER NOT NULL DEFAULT 0,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS user_tag_folder_members (
          folder_id INTEGER NOT NULL,
          tag_text TEXT NOT NULL,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL,
          PRIMARY KEY(folder_id, tag_text),
          UNIQUE(tag_text),
          FOREIGN KEY(folder_id) REFERENCES user_tag_folders(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_user_tag_folder_members_folder_id
          ON user_tag_folder_members(folder_id, updated_at DESC);

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
          model_id TEXT NOT NULL,
          model_version TEXT NOT NULL,
          dimension INTEGER NOT NULL,
          normalized INTEGER NOT NULL DEFAULT 1,
          vector_blob BLOB NOT NULL,
          source_modified_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL,
          PRIMARY KEY(image_id, model_id, model_version),
          FOREIGN KEY(image_id) REFERENCES images(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_image_clip_embeddings_model_updated
          ON image_clip_embeddings(model_id, model_version, updated_at DESC);

        CREATE TABLE IF NOT EXISTS image_atmosphere_signatures (
          image_id TEXT PRIMARY KEY,
          source_modified_at INTEGER NOT NULL,
          source_file_size INTEGER NOT NULL,
          signature_blob BLOB NOT NULL,
          updated_at INTEGER NOT NULL,
          FOREIGN KEY(image_id) REFERENCES images(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_image_atmosphere_signatures_updated
          ON image_atmosphere_signatures(updated_at DESC);

        CREATE TABLE IF NOT EXISTS image_color_signatures (
          image_id TEXT PRIMARY KEY,
          source_modified_at INTEGER NOT NULL,
          source_file_size INTEGER NOT NULL,
          signature_blob BLOB NOT NULL,
          updated_at INTEGER NOT NULL,
          FOREIGN KEY(image_id) REFERENCES images(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_image_color_signatures_updated
          ON image_color_signatures(updated_at DESC);

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
          flip_x INTEGER NOT NULL DEFAULT 0,
          flip_y INTEGER NOT NULL DEFAULT 0,
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
    ensure_clip_embedding_table(conn)?;
    ensure_user_folder_sort_order(conn)?;
    ensure_user_folder_source_metadata(conn)?;
    ensure_reference_board_items_allow_duplicates(conn)?;
    ensure_reference_board_item_flip_columns(conn)?;
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
    if !table_has_column(conn, "images", "is_favorite")? {
        conn.execute(
            "ALTER TABLE images ADD COLUMN is_favorite INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|error| format!("Failed to upgrade images.is_favorite column: {error}"))?;
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

fn ensure_clip_embedding_table(conn: &Connection) -> Result<(), String> {
    if !table_exists(conn, "image_clip_embeddings")? {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS image_clip_embeddings (
              image_id TEXT NOT NULL,
              model_id TEXT NOT NULL,
              model_version TEXT NOT NULL,
              dimension INTEGER NOT NULL,
              normalized INTEGER NOT NULL DEFAULT 1,
              vector_blob BLOB NOT NULL,
              source_modified_at INTEGER NOT NULL,
              updated_at INTEGER NOT NULL,
              PRIMARY KEY(image_id, model_id, model_version),
              FOREIGN KEY(image_id) REFERENCES images(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_image_clip_embeddings_model_updated
              ON image_clip_embeddings(model_id, model_version, updated_at DESC);
            ",
        )
        .map_err(|error| format!("Failed to ensure image_clip_embeddings table: {error}"))?;
        return Ok(());
    }

    if table_has_column(conn, "image_clip_embeddings", "vector_blob")?
        && table_has_column(conn, "image_clip_embeddings", "model_id")?
        && table_has_column(conn, "image_clip_embeddings", "model_version")?
        && table_has_column(conn, "image_clip_embeddings", "dimension")?
        && table_has_column(conn, "image_clip_embeddings", "normalized")?
    {
        return Ok(());
    }

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS image_clip_embeddings_v2 (
          image_id TEXT NOT NULL,
          model_id TEXT NOT NULL,
          model_version TEXT NOT NULL,
          dimension INTEGER NOT NULL,
          normalized INTEGER NOT NULL DEFAULT 1,
          vector_blob BLOB NOT NULL,
          source_modified_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL,
          PRIMARY KEY(image_id, model_id, model_version),
          FOREIGN KEY(image_id) REFERENCES images(id) ON DELETE CASCADE
        );
        ",
    )
    .map_err(|error| format!("Failed to create image_clip_embeddings_v2: {error}"))?;

    let has_vector_json = table_has_column(conn, "image_clip_embeddings", "vector_json")?;
    if has_vector_json {
        let mut stmt = conn
            .prepare(
                "
                SELECT image_id, model_name, dim, vector_json, source_modified_at, updated_at
                FROM image_clip_embeddings
                ",
            )
            .map_err(|error| format!("Failed to read legacy clip embeddings: {error}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            })
            .map_err(|error| format!("Failed to read legacy clip embeddings: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Failed to read legacy clip embeddings: {error}"))?;
        drop(stmt);

        for (image_id, legacy_model_name, legacy_dim, vector_json, source_modified_at, updated_at) in rows {
            let parsed: Vec<f32> = match serde_json::from_str(&vector_json) {
                Ok(values) => values,
                Err(_) => continue,
            };
            if parsed.is_empty() {
                continue;
            }
            let normalized = normalize_vector(&parsed);
            let vector_blob = encode_f32_blob(&normalized);
            let dimension = legacy_dim
                .and_then(|value| usize::try_from(value).ok())
                .filter(|value| *value > 0)
                .unwrap_or(normalized.len());
            let model_id = legacy_model_name
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .map(legacy_model_name_to_model_id)
                .unwrap_or(CHINESE_CLIP_MODEL_ID)
                .to_string();

            conn.execute(
                "
                INSERT OR REPLACE INTO image_clip_embeddings_v2 (
                  image_id, model_id, model_version, dimension, normalized, vector_blob, source_modified_at, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?7)
                ",
                params![
                    image_id,
                    model_id,
                    CHINESE_CLIP_MODEL_VERSION,
                    i64::try_from(dimension).unwrap_or(0),
                    vector_blob,
                    source_modified_at,
                    updated_at,
                ],
            )
            .map_err(|error| format!("Failed to migrate legacy clip embedding: {error}"))?;
        }
    }

    conn.execute_batch(
        "
        DROP TABLE IF EXISTS image_clip_embeddings;
        ALTER TABLE image_clip_embeddings_v2 RENAME TO image_clip_embeddings;
        CREATE INDEX IF NOT EXISTS idx_image_clip_embeddings_model_updated
          ON image_clip_embeddings(model_id, model_version, updated_at DESC);
        ",
    )
    .map_err(|error| format!("Failed to finalize clip embedding migration: {error}"))?;
    Ok(())
}

fn table_exists(conn: &Connection, table_name: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        params![table_name],
        |row| row.get::<_, i64>(0),
    )
    .map(|value| value != 0)
    .map_err(|error| format!("Failed to inspect table existence: {error}"))
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
          flip_x INTEGER NOT NULL DEFAULT 0,
          flip_y INTEGER NOT NULL DEFAULT 0,
          z_index INTEGER NOT NULL DEFAULT 0,
          created_at INTEGER NOT NULL,
          FOREIGN KEY(board_id) REFERENCES reference_boards(id) ON DELETE CASCADE,
          FOREIGN KEY(image_id) REFERENCES images(id) ON DELETE CASCADE
        );
        INSERT INTO reference_board_items (
          id, board_id, image_id, x, y, width, height, rotation, flip_x, flip_y, z_index, created_at
        )
        SELECT id, board_id, image_id, x, y, width, height, rotation, 0, 0, z_index, created_at
        FROM reference_board_items_old;
        DROP TABLE reference_board_items_old;
        CREATE INDEX IF NOT EXISTS idx_reference_board_items_board_id
          ON reference_board_items(board_id, z_index);
        ",
    )
    .map_err(|error| format!("升级参考板图片表失败：{error}"))?;
    Ok(())
}

fn ensure_reference_board_item_flip_columns(conn: &Connection) -> Result<(), String> {
    if !table_has_column(conn, "reference_board_items", "flip_x")? {
        conn.execute(
            "ALTER TABLE reference_board_items ADD COLUMN flip_x INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|error| format!("Failed to add reference_board_items.flip_x: {error}"))?;
    }
    if !table_has_column(conn, "reference_board_items", "flip_y")? {
        conn.execute(
            "ALTER TABLE reference_board_items ADD COLUMN flip_y INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|error| format!("Failed to add reference_board_items.flip_y: {error}"))?;
    }
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

fn ensure_user_folder_source_metadata(conn: &Connection) -> Result<(), String> {
    if !table_has_column(conn, "user_folders", "source_kind")? {
        conn.execute(
            "ALTER TABLE user_folders ADD COLUMN source_kind TEXT NOT NULL DEFAULT 'manual'",
            [],
        )
        .map_err(|error| format!("Failed to add user_folders.source_kind: {error}"))?;
    }
    if !table_has_column(conn, "user_folders", "source_path")? {
        conn.execute(
            "ALTER TABLE user_folders ADD COLUMN source_path TEXT",
            [],
        )
        .map_err(|error| format!("Failed to add user_folders.source_path: {error}"))?;
    }
    conn.execute_batch(
        "
        UPDATE user_folders
        SET source_kind = 'manual'
        WHERE source_kind IS NULL OR TRIM(source_kind) = '';

        CREATE UNIQUE INDEX IF NOT EXISTS idx_user_folders_source_path
          ON user_folders(source_path)
          WHERE source_path IS NOT NULL;
        ",
    )
    .map_err(|error| format!("Failed to finalize user folder source metadata: {error}"))?;
    Ok(())
}

fn load_store(conn: &Connection) -> Result<LibraryStore, String> {
    let started = Instant::now();
    let t0 = Instant::now();
    let folders = load_folders(conn)?;
    let folders_ms = t0.elapsed().as_millis();

    let t1 = Instant::now();
    let images = load_images(conn)?;
    let images_ms = t1.elapsed().as_millis();

    let t2 = Instant::now();
    let user_folders = load_user_folders(conn)?;
    let user_folders_ms = t2.elapsed().as_millis();

    let t3 = Instant::now();
    let image_folders = load_image_folder_assignments(conn)?;
    let image_folders_ms = t3.elapsed().as_millis();

    let t4 = Instant::now();
    let reference_board_folders = load_reference_board_folders(conn)?;
    let reference_board_folders_ms = t4.elapsed().as_millis();

    let t5 = Instant::now();
    let reference_boards = load_reference_boards(conn)?;
    let reference_boards_ms = t5.elapsed().as_millis();

    let t6 = Instant::now();
    let reference_board_items = load_reference_board_items(conn)?;
    let reference_board_items_ms = t6.elapsed().as_millis();

    eprintln!(
        "[startup-prof] load_store folders_ms={} images_ms={} user_folders_ms={} image_folders_ms={} ref_folders_ms={} ref_boards_ms={} ref_items_ms={} total_ms={} counts=folders:{} images:{} user_folders:{} image_folders:{} ref_folders:{} ref_boards:{} ref_items:{}",
        folders_ms,
        images_ms,
        user_folders_ms,
        image_folders_ms,
        reference_board_folders_ms,
        reference_boards_ms,
        reference_board_items_ms,
        started.elapsed().as_millis(),
        folders.len(),
        images.len(),
        user_folders.len(),
        image_folders.len(),
        reference_board_folders.len(),
        reference_boards.len(),
        reference_board_items.len(),
    );

    Ok(LibraryStore {
        folders,
        images,
        user_folders,
        image_folders,
        reference_board_folders,
        reference_boards,
        reference_board_items,
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
              i.imported_at, i.folder_id, i.missing, i.trashed, i.is_favorite, i.source
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
                is_favorite: row.get::<_, i64>(13)? != 0,
                source: row.get(14)?,
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
            SELECT id, board_id, image_id, x, y, width, height, rotation, flip_x, flip_y, z_index, created_at
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
                flip_x: row.get::<_, i64>(8)? != 0,
                flip_y: row.get::<_, i64>(9)? != 0,
                z_index: row.get(10)?,
                created_at: row.get(11)?,
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
        SELECT id, board_id, image_id, x, y, width, height, rotation, flip_x, flip_y, z_index, created_at
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
                flip_x: row.get::<_, i64>(8)? != 0,
                flip_y: row.get::<_, i64>(9)? != 0,
                z_index: row.get(10)?,
                created_at: row.get(11)?,
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
            SELECT id, board_id, image_id, x, y, width, height, rotation, flip_x, flip_y, z_index, created_at
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
                flip_x: row.get::<_, i64>(8)? != 0,
                flip_y: row.get::<_, i64>(9)? != 0,
                z_index: row.get(10)?,
                created_at: row.get(11)?,
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
          i.imported_at, i.folder_id, i.missing, i.trashed, i.is_favorite, i.source
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
                is_favorite: row.get::<_, i64>(13)? != 0,
                source: row.get(14)?,
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
    pause_requested: &Arc<Mutex<bool>>,
    stop_requested: &Arc<Mutex<bool>>,
    collect_tag_queue: bool,
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
        if background_scan_stop_requested(stop_requested) {
            break;
        }
        wait_for_background_scan_resume(progress, pause_requested, stop_requested, "collecting");
        if background_scan_stop_requested(stop_requested) {
            break;
        }
        let folder_path = normalize_existing_or_stored_folder_path(&folder_path);
        if !Path::new(&folder_path).is_dir() {
            scanned_folders += 1;
            set_scan_progress_scanned_folders(progress, scanned_folders);
            continue;
        }
        let folder_id = upsert_folder(&conn, &folder_path, scanned_at)?;
        let found = scan_images(Path::new(&folder_path), scanned_at, &mut seen_paths, &existing_meta);
        let found_count = found.len() as i64;
        let mut newly_found_images = Vec::<ScannedImage>::new();
        for image in &found {
            let previous = existing_meta.get(&image.path).copied();
            let is_new = !known_paths.contains(&image.path);
            upsert_image(&conn, folder_id, image)?;
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
                new_image_ids.push(image.path.clone());
                newly_found_images.push(image.clone());
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
        assign_scanned_images_to_nearest_synced_parent_folder(
            &conn,
            &folder_path,
            &newly_found_images,
            scanned_at,
        )?;
        scanned_folders += 1;
        set_scan_progress_new_images(progress, new_image_ids.len() as i64);
        set_scan_progress_updated_images(progress, updated_images);
        set_scan_progress_skipped_images(progress, skipped_images);
        set_scan_progress_scanned_folders(progress, scanned_folders);
        if found_count > 0 {
            eprintln!("[wd-scan] folder scanned, new images: {found_count}");
        }
    }

    if background_scan_stop_requested(stop_requested) {
        return Ok(ScanCollectResult {
            tag_queue_image_ids: Vec::new(),
        });
    }

    if !collect_tag_queue {
        set_scan_progress_queued_images(progress, 0);
        return Ok(ScanCollectResult {
            tag_queue_image_ids: Vec::new(),
        });
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
    pause_requested: &Arc<Mutex<bool>>,
    stop_requested: &Arc<Mutex<bool>>,
    wd_tagger_service: &Arc<Mutex<Option<WdTaggerService>>>,
) -> Result<(), String> {
    if image_ids.is_empty() {
        return Ok(());
    }

    let mut conn = open_database(database_path)?;
    let dictionary = load_cn_tag_dictionary_map()?;
    let model_root = resolve_wd_tagger_model_dir(None)?;
    let model_path = model_root.join("model.onnx");
    let tags_path = model_root.join("selected_tags.csv");
    let script_path = resolve_wd_tagger_service_script_path()?;

    if !model_path.is_file() || !tags_path.is_file() || !script_path.is_file() {
        let err = "Model files or wd_tagger_service.py not found; skip tagging".to_string();
        set_scan_progress_error(progress, &err);
        push_scan_progress_recent_error(progress, &err);
        eprintln!("[wd-tag] {err}");
        return Ok(());
    }

    let mut service_guard = wd_tagger_service
        .lock()
        .map_err(|_| "WD tagger service state is locked".to_string())?;
    ensure_wd_tagger_service_started(
        &mut service_guard,
        &model_path,
        &tags_path,
        &script_path,
    )?;

    eprintln!("[wd-tag] queue size: {}", image_ids.len());
    for image_id in image_ids {
        if background_scan_stop_requested(stop_requested) {
            break;
        }
        wait_for_background_scan_resume(progress, pause_requested, stop_requested, "tagging");
        if background_scan_stop_requested(stop_requested) {
            break;
        }
        if !Path::new(image_id).is_file() {
            increment_scan_progress_failed(progress);
            continue;
        }
        match run_wd_tagger_via_service_with_recovery(
            &mut service_guard,
            &model_path,
            &tags_path,
            &script_path,
            image_id,
            image_id,
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

fn run_wd_tagger_via_service_with_recovery(
    service: &mut Option<WdTaggerService>,
    model_path: &Path,
    tags_path: &Path,
    script_path: &Path,
    image_id: &str,
    image_path: &str,
    general_threshold: f32,
    character_threshold: f32,
) -> Result<WdTaggerTestResult, String> {
    ensure_wd_tagger_service_started(service, model_path, tags_path, script_path)?;
    let primary = {
        let running = service
            .as_mut()
            .ok_or_else(|| "WD tagger service unavailable".to_string())?;
        run_wd_tagger_via_service(running, image_id, image_path, general_threshold, character_threshold)
    };
    match primary {
        Ok(result) => Ok(result),
        Err(first_error) => {
            stop_python_child_service(service, |running| &mut running.child);
            ensure_wd_tagger_service_started(service, model_path, tags_path, script_path)?;
            let running = service
                .as_mut()
                .ok_or_else(|| "WD tagger service unavailable after restart".to_string())?;
            run_wd_tagger_via_service(
                running,
                image_id,
                image_path,
                general_threshold,
                character_threshold,
            )
            .map_err(|second_error| {
                format!(
                    "WD tagger service failed and restart retry also failed. first: {first_error}; second: {second_error}"
                )
            })
        }
    }
}

fn ensure_wd_tagger_service_started(
    service: &mut Option<WdTaggerService>,
    model_path: &Path,
    tags_path: &Path,
    script_path: &Path,
) -> Result<(), String> {
    let need_restart = match service.as_ref() {
        None => true,
        Some(existing) => existing.model_path != model_path || existing.tags_path != tags_path,
    };
    if !need_restart {
        return Ok(());
    }
    stop_python_child_service(service, |running| &mut running.child);
    *service = Some(spawn_wd_tagger_service(model_path, tags_path, script_path)?);
    Ok(())
}

fn spawn_wd_tagger_service(
    model_path: &Path,
    tags_path: &Path,
    script_path: &Path,
) -> Result<WdTaggerService, String> {
    let mut command = python_command();
    let mut child = command
        .arg("-X")
        .arg("utf8")
        .arg(script_path)
        .arg("--model")
        .arg(model_path)
        .arg("--tags")
        .arg(tags_path)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("Failed to start WD tagger service: {error}"))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "WD tagger service stdin unavailable".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "WD tagger service stdout unavailable".to_string())?;

    Ok(WdTaggerService {
        child,
        stdin,
        stdout: BufReader::new(stdout),
        model_path: model_path.to_path_buf(),
        tags_path: tags_path.to_path_buf(),
    })
}

fn run_wd_tagger_via_service(
    service: &mut WdTaggerService,
    image_id: &str,
    image_path: &str,
    general_threshold: f32,
    character_threshold: f32,
) -> Result<WdTaggerTestResult, String> {
    let request = serde_json::json!({
        "image_id": image_id,
        "image_path": image_path,
        "general_threshold": general_threshold,
        "character_threshold": character_threshold,
    });
    service
        .stdin
        .write_all(request.to_string().as_bytes())
        .and_then(|_| service.stdin.write_all(b"\n"))
        .and_then(|_| service.stdin.flush())
        .map_err(|error| format!("Failed to write WD tagger service request: {error}"))?;

    let mut response_line = String::new();
    service
        .stdout
        .read_line(&mut response_line)
        .map_err(|error| format!("Failed to read WD tagger service response: {error}"))?;
    if response_line.trim().is_empty() {
        return Err("WD tagger service returned empty response".to_string());
    }
    let value: serde_json::Value = serde_json::from_str(response_line.trim())
        .map_err(|error| format!("Failed to parse WD tagger service response: {error}"))?;
    if let Some(error_text) = value.get("error").and_then(|item| item.as_str()) {
        return Err(format!("WD tagger service error: {error_text}"));
    }
    serde_json::from_value(value)
        .map_err(|error| format!("Failed to parse WD tagger service output: {error}"))
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
    apply_matching_user_folder_rules_for_image(conn, image_id)?;
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

fn generate_atmosphere_signatures_once(
    database_path: &Path,
    progress: &Arc<Mutex<AtmosphereGenerationProgress>>,
    pause_requested: &Arc<Mutex<bool>>,
    stop_requested: &Arc<Mutex<bool>>,
    signature_cache: &Arc<Mutex<Option<SignatureCache>>>,
) -> Result<i64, String> {
    set_atmosphere_progress_phase(progress, "collecting");
    let conn = open_database(database_path)?;
    let candidates = load_atmosphere_generation_candidates(&conn)?;
    set_atmosphere_progress_total(progress, candidates.len() as i64);
    set_atmosphere_progress_phase(progress, "generating");

    let mut generated = 0i64;
    let mut skipped = 0i64;
    let mut failed = 0i64;
    let mut processed = 0i64;

    for candidate in &candidates {
        if atmosphere_stop_requested(stop_requested) {
            break;
        }
        wait_for_atmosphere_resume(progress, pause_requested, stop_requested);
        if atmosphere_stop_requested(stop_requested) {
            break;
        }

        let existing = conn
            .query_row(
                "
                SELECT source_modified_at, source_file_size, signature_blob
                FROM image_atmosphere_signatures
                WHERE image_id = ?1
                ",
                params![candidate.image_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| format!("Failed to read atmosphere signature: {error}"))?;
        if let Some((source_modified_at, source_file_size, blob)) = existing {
            if source_modified_at == candidate.modified_at
                && source_file_size == candidate.file_size
                && decode_f32_blob(&blob)
                    .map(|vector| vector.len() == ATMOSPHERE_SIGNATURE_DIM)
                    .unwrap_or(false)
            {
                skipped += 1;
                processed += 1;
                set_atmosphere_progress_counts(progress, processed, generated, skipped, failed);
                continue;
            }
        }

        if !Path::new(&candidate.source_path).is_file() {
            if candidate.priority > 0 {
                skipped += 1;
            } else {
                failed += 1;
            }
            processed += 1;
            set_atmosphere_progress_counts(progress, processed, generated, skipped, failed);
            continue;
        }

        match compute_atmosphere_signature_from_path(&candidate.source_path) {
            Ok(signature) => {
                conn.execute(
                    "
                    INSERT INTO image_atmosphere_signatures (
                      image_id, source_modified_at, source_file_size, signature_blob, updated_at
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5)
                    ON CONFLICT(image_id) DO UPDATE SET
                      source_modified_at = excluded.source_modified_at,
                      source_file_size = excluded.source_file_size,
                      signature_blob = excluded.signature_blob,
                      updated_at = excluded.updated_at
                    ",
                    params![
                        candidate.image_id,
                        candidate.modified_at,
                        candidate.file_size,
                        encode_f32_blob(&signature),
                        now_ms()
                    ],
                )
                .map_err(|error| format!("Failed to save atmosphere signature: {error}"))?;
                upsert_signature_cache_entry(
                    signature_cache,
                    &candidate.image_id,
                    signature.clone(),
                    ATMOSPHERE_SIGNATURE_DIM,
                );
                generated += 1;
            }
            Err(error) => {
                failed += 1;
                set_atmosphere_progress_error(progress, &error);
                push_atmosphere_progress_recent_error(progress, &error);
            }
        }
        processed += 1;
        set_atmosphere_progress_counts(progress, processed, generated, skipped, failed);
    }

    if atmosphere_stop_requested(stop_requested) {
        set_atmosphere_progress_phase(progress, "idle");
    }
    Ok(generated)
}

fn load_atmosphere_generation_candidates(
    conn: &Connection,
) -> Result<Vec<AtmosphereGenerationCandidate>, String> {
    let rows = load_atmosphere_signature_candidates(conn, None)?;
    let mut result = Vec::<AtmosphereGenerationCandidate>::with_capacity(rows.len());
    for row in rows {
        let thumbnail_is_current = matches!(
            (row.thumbnail_source_modified_at, row.thumbnail_source_file_size),
            (Some(modified_at), Some(file_size)) if modified_at == row.modified_at && file_size == row.file_size
        );
        let use_thumbnail = thumbnail_is_current
            && row
                .thumbnail_path
                .as_deref()
                .map(|path| Path::new(path).is_file())
                .unwrap_or(false);
        let source_path = if use_thumbnail {
            row.thumbnail_path.unwrap_or_else(|| row.image_path.clone())
        } else {
            row.image_path.clone()
        };
        let priority = if use_thumbnail { 0 } else { 1 };
        result.push(AtmosphereGenerationCandidate {
            image_id: row.image_id,
            source_path,
            modified_at: row.modified_at,
            file_size: row.file_size,
            priority,
        });
    }
    result.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| b.modified_at.cmp(&a.modified_at))
            .then_with(|| a.image_id.cmp(&b.image_id))
    });
    Ok(result)
}

fn generate_color_signatures_once(
    database_path: &Path,
    progress: &Arc<Mutex<ColorSignatureGenerationProgress>>,
    pause_requested: &Arc<Mutex<bool>>,
    stop_requested: &Arc<Mutex<bool>>,
    signature_cache: &Arc<Mutex<Option<SignatureCache>>>,
) -> Result<i64, String> {
    set_color_signature_progress_phase(progress, "collecting");
    let conn = open_database(database_path)?;
    clear_incompatible_color_signature_records(&conn)?;
    let candidates = load_color_signature_generation_candidates(&conn)?;
    set_color_signature_progress_total(progress, candidates.len() as i64);
    set_color_signature_progress_phase(progress, "generating");

    let mut generated = 0i64;
    let mut skipped = 0i64;
    let mut failed = 0i64;
    let mut processed = 0i64;

    for candidate in &candidates {
        if color_signature_stop_requested(stop_requested) {
            break;
        }
        wait_for_color_signature_resume(progress, pause_requested, stop_requested);
        if color_signature_stop_requested(stop_requested) {
            break;
        }

        let existing = conn
            .query_row(
                "
                SELECT source_modified_at, source_file_size, signature_blob
                FROM image_color_signatures
                WHERE image_id = ?1
                ",
                params![candidate.image_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| format!("Failed to read color signature: {error}"))?;
        if let Some((source_modified_at, source_file_size, blob)) = existing {
            if source_modified_at == candidate.modified_at
                && source_file_size == candidate.file_size
                && decode_f32_blob(&blob)
                    .map(|vector| vector.len() == COLOR_SIGNATURE_DIM)
                    .unwrap_or(false)
            {
                skipped += 1;
                processed += 1;
                set_color_signature_progress_counts(progress, processed, generated, skipped, failed);
                continue;
            }
        }

        let Some(thumbnail_path) = candidate.thumbnail_path.as_deref() else {
            skipped += 1;
            processed += 1;
            set_color_signature_progress_counts(progress, processed, generated, skipped, failed);
            continue;
        };
        if !Path::new(thumbnail_path).is_file() {
            skipped += 1;
            processed += 1;
            set_color_signature_progress_counts(progress, processed, generated, skipped, failed);
            continue;
        }

        match compute_color_signature_from_path(thumbnail_path) {
            Ok(signature) => {
                conn.execute(
                    "
                    INSERT INTO image_color_signatures (
                      image_id, source_modified_at, source_file_size, signature_blob, updated_at
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5)
                    ON CONFLICT(image_id) DO UPDATE SET
                      source_modified_at = excluded.source_modified_at,
                      source_file_size = excluded.source_file_size,
                      signature_blob = excluded.signature_blob,
                      updated_at = excluded.updated_at
                    ",
                    params![
                        candidate.image_id,
                        candidate.modified_at,
                        candidate.file_size,
                        encode_f32_blob(&signature),
                        now_ms()
                    ],
                )
                .map_err(|error| format!("Failed to save color signature: {error}"))?;
                upsert_signature_cache_entry(
                    signature_cache,
                    &candidate.image_id,
                    signature.clone(),
                    COLOR_SIGNATURE_DIM,
                );
                generated += 1;
            }
            Err(error) => {
                failed += 1;
                set_color_signature_progress_error(progress, &error);
                push_color_signature_progress_recent_error(progress, &error);
            }
        }

        processed += 1;
        set_color_signature_progress_counts(progress, processed, generated, skipped, failed);
    }

    if color_signature_stop_requested(stop_requested) {
        set_color_signature_progress_phase(progress, "idle");
    }
    Ok(generated)
}

fn load_color_signature_generation_candidates(
    conn: &Connection,
) -> Result<Vec<ColorSignatureGenerationCandidate>, String> {
    let mut stmt = conn
        .prepare(
            "
            SELECT
              i.id,
              t.thumb_path,
              i.modified_at,
              i.file_size
            FROM images i
            LEFT JOIN image_thumbnails t ON t.image_id = i.id
            WHERE i.source = 'library'
              AND COALESCE(i.trashed, 0) = 0
              AND COALESCE(i.missing, 0) = 0
            ORDER BY i.modified_at DESC, i.id ASC
            ",
        )
        .map_err(|error| format!("Failed to load color signature candidates: {error}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ColorSignatureGenerationCandidate {
                image_id: row.get(0)?,
                thumbnail_path: row.get(1)?,
                modified_at: row.get(2)?,
                file_size: row.get(3)?,
            })
        })
        .map_err(|error| format!("Failed to load color signature candidates: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to load color signature candidates: {error}"))?;
    Ok(rows)
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
    let worker_count = THUMBNAIL_WORKER_COUNT.max(1);
    let queue_capacity = THUMBNAIL_WORKER_QUEUE_CAPACITY.max(1);
    let (result_tx, result_rx) = mpsc::channel::<ThumbnailWorkerResult>();
    let mut worker_senders = Vec::with_capacity(worker_count);
    let mut worker_handles = Vec::with_capacity(worker_count);
    for _ in 0..worker_count {
        let (job_tx, job_rx) = mpsc::sync_channel::<Option<ThumbnailCandidate>>(queue_capacity);
        let worker_result_tx = result_tx.clone();
        let worker_thumb_root = thumb_root.clone();
        let handle = thread::spawn(move || {
            thumbnail_worker_loop(job_rx, worker_result_tx, worker_thumb_root);
        });
        worker_senders.push(job_tx);
        worker_handles.push(handle);
    }
    drop(result_tx);

    let mut generated = 0i64;
    let mut skipped = 0i64;
    let mut failed = 0i64;
    let mut processed = 0i64;
    let mut in_flight = 0usize;
    let mut next_worker_index = 0usize;
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

        let sender_index = next_worker_index % worker_senders.len();
        next_worker_index += 1;
        let sender = &worker_senders[sender_index];
        let mut pending_candidate = Some(candidate);
        while let Some(job_candidate) = pending_candidate.take() {
            if thumbnail_stop_requested(stop_requested) {
                stop_now = true;
                break;
            }
            wait_for_thumbnail_resume(progress, pause_requested, stop_requested);
            if thumbnail_stop_requested(stop_requested) {
                stop_now = true;
                break;
            }
            match sender.try_send(Some(job_candidate)) {
                Ok(()) => {
                    in_flight += 1;
                }
                Err(TrySendError::Full(returned_candidate)) => {
                    pending_candidate = returned_candidate;
                    match result_rx.recv_timeout(std::time::Duration::from_millis(40)) {
                        Ok(result) => {
                            in_flight = in_flight.saturating_sub(1);
                            apply_thumbnail_worker_result(
                                &conn,
                                progress,
                                result,
                                &mut processed,
                                &mut generated,
                                skipped,
                                &mut failed,
                            );
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            let error = "Thumbnail workers disconnected unexpectedly".to_string();
                            set_thumbnail_progress_error(progress, &error);
                            push_thumbnail_progress_recent_error(progress, &error);
                            failed += 1;
                            processed += 1;
                            set_thumbnail_progress_counts(
                                progress, processed, generated, skipped, failed,
                            );
                            stop_now = true;
                            break;
                        }
                    }
                }
                Err(TrySendError::Disconnected(returned_candidate)) => {
                    let image_id = returned_candidate
                        .as_ref()
                        .map(|candidate| candidate.image_id.as_str())
                        .unwrap_or("unknown");
                    let error = format!(
                        "Thumbnail worker disconnected before processing {}",
                        image_id
                    );
                    set_thumbnail_progress_error(progress, &error);
                    push_thumbnail_progress_recent_error(progress, &error);
                    failed += 1;
                    processed += 1;
                    set_thumbnail_progress_counts(progress, processed, generated, skipped, failed);
                }
            }
        }
        if stop_now {
            break;
        }
    }

    drop(worker_senders);

    while in_flight > 0 {
        match result_rx.recv_timeout(std::time::Duration::from_millis(120)) {
            Ok(result) => {
                in_flight -= 1;
                apply_thumbnail_worker_result(
                    &conn,
                    progress,
                    result,
                    &mut processed,
                    &mut generated,
                    skipped,
                    &mut failed,
                );
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let error = "Thumbnail worker result channel disconnected".to_string();
                set_thumbnail_progress_error(progress, &error);
                push_thumbnail_progress_recent_error(progress, &error);
                break;
            }
        }
    }

    for handle in worker_handles {
        if handle.join().is_err() {
            let error = "Thumbnail worker thread panicked".to_string();
            set_thumbnail_progress_error(progress, &error);
            push_thumbnail_progress_recent_error(progress, &error);
        }
    }

    cleanup_orphan_thumbnail_files(&conn, &thumb_root)?;
    if stop_now {
        set_thumbnail_progress_phase(progress, "idle");
    }
    Ok(generated)
}

fn thumbnail_worker_loop(
    receiver: mpsc::Receiver<Option<ThumbnailCandidate>>,
    sender: mpsc::Sender<ThumbnailWorkerResult>,
    thumb_root: PathBuf,
) {
    while let Ok(job) = receiver.recv() {
        let Some(candidate) = job else {
            break;
        };
        let output =
            generate_single_thumbnail(&candidate, &thumb_root, THUMBNAIL_LONG_EDGE, THUMBNAIL_WEBP_QUALITY);
        let _ = sender.send(ThumbnailWorkerResult { candidate, output });
    }
}

fn apply_thumbnail_worker_result(
    conn: &Connection,
    progress: &Arc<Mutex<ThumbnailGenerationProgress>>,
    result: ThumbnailWorkerResult,
    processed: &mut i64,
    generated: &mut i64,
    skipped: i64,
    failed: &mut i64,
) {
    match result.output {
        Ok(next_thumb_path) => {
            if let Err(error) = upsert_thumbnail_record(
                conn,
                &result.candidate.image_id,
                &next_thumb_path,
                result.candidate.modified_at,
                result.candidate.file_size,
            ) {
                set_thumbnail_progress_error(progress, &error);
                push_thumbnail_progress_recent_error(progress, &error);
                *failed += 1;
            } else {
                *generated += 1;
                if let Some(previous_thumb) = result.candidate.current_thumb_path {
                    if previous_thumb != next_thumb_path {
                        let _ = fs::remove_file(previous_thumb);
                    }
                }
            }
        }
        Err(error) => {
            set_thumbnail_progress_error(progress, &error);
            push_thumbnail_progress_recent_error(progress, &error);
            *failed += 1;
        }
    }

    *processed += 1;
    set_thumbnail_progress_counts(progress, *processed, *generated, skipped, *failed);
}

fn generate_natural_language_embeddings_once(
    database_path: &Path,
    progress: &Arc<Mutex<NaturalLanguageScanProgress>>,
    clip_vector_cache: &Arc<Mutex<Option<ClipImageVectorCache>>>,
    clip_image_encoder_service: &Arc<Mutex<Option<ClipImageEncoderService>>>,
    clip_image_encoder_last_used_at: &Arc<Mutex<i64>>,
    clip_image_encoder_release_worker_running: &Arc<Mutex<bool>>,
    pause_requested: &Arc<Mutex<bool>>,
    stop_requested: &Arc<Mutex<bool>>,
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
              e.source_modified_at
            FROM images i
            LEFT JOIN image_clip_embeddings e
              ON e.image_id = i.id
             AND e.model_id = ?1
             AND e.model_version = ?2
            WHERE i.source = 'library'
              AND COALESCE(i.trashed, 0) = 0
            ORDER BY i.modified_at DESC, i.id ASC
            ",
        )
        .map_err(|error| format!("Failed to load natural language scan candidates: {error}"))?;
    let candidates = stmt
        .query_map(params![CHINESE_CLIP_MODEL_ID, CHINESE_CLIP_MODEL_VERSION], |row| {
            Ok(NaturalLanguageEmbeddingCandidate {
                image_id: row.get(0)?,
                image_path: row.get(1)?,
                modified_at: row.get(2)?,
                current_source_modified_at: row.get(3)?,
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
    let script_path = resolve_chinese_clip_image_service_script_path()?;
    let mut generated = 0i64;
    let mut skipped = 0i64;
    let mut failed = 0i64;
    let mut processed = 0i64;

    for candidate in candidates {
        if natural_language_scan_stop_requested(stop_requested) {
            break;
        }
        wait_for_natural_language_scan_resume(progress, pause_requested, stop_requested);
        if natural_language_scan_stop_requested(stop_requested) {
            break;
        }

        if !Path::new(&candidate.image_path).is_file() {
            clear_image_clip_embedding(&conn, &candidate.image_id)?;
            remove_clip_vector_cache_entry(clip_vector_cache, &candidate.image_id)?;
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

        touch_clip_image_service_last_used(clip_image_encoder_last_used_at);
        let embedding_result = {
            let mut image_service_guard = clip_image_encoder_service
                .lock()
                .map_err(|_| "Clip image encoder service is locked".to_string())?;
            run_chinese_clip_image_embedding_via_service_with_recovery(
                &mut image_service_guard,
                &model_root,
                &script_path,
                &candidate.image_path,
            )
        };
        touch_clip_image_service_last_used(clip_image_encoder_last_used_at);
        ensure_clip_image_service_idle_reaper_started(
            clip_image_encoder_service,
            clip_image_encoder_last_used_at,
            clip_image_encoder_release_worker_running,
        );

        match embedding_result {
            Ok(vector) => {
                if let Err(error) = upsert_image_clip_embedding(
                    &conn,
                    &candidate.image_id,
                    &vector,
                    candidate.modified_at,
                ) {
                    failed += 1;
                    set_natural_language_scan_progress_error(progress, &error);
                    push_natural_language_scan_recent_error(progress, &error);
                } else {
                    upsert_clip_vector_cache_entry(clip_vector_cache, &candidate.image_id, vector.clone());
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
    matches!(candidate.current_source_modified_at, Some(source_modified_at) if source_modified_at == candidate.modified_at)
}

fn clear_image_clip_embedding(conn: &Connection, image_id: &str) -> Result<(), String> {
    conn.execute(
        "DELETE FROM image_clip_embeddings WHERE image_id = ?1 AND model_id = ?2 AND model_version = ?3",
        params![image_id, CHINESE_CLIP_MODEL_ID, CHINESE_CLIP_MODEL_VERSION],
    )
    .map_err(|error| format!("Failed to clear stale clip embedding: {error}"))?;
    Ok(())
}

fn upsert_image_clip_embedding(
    conn: &Connection,
    image_id: &str,
    vector: &[f32],
    source_modified_at: i64,
) -> Result<(), String> {
    if vector.is_empty() {
        return Err("CLIP embedding is empty".to_string());
    }
    let normalized_vector = normalize_vector(vector);
    let dim = i64::try_from(normalized_vector.len()).map_err(|_| "CLIP embedding dimension overflow".to_string())?;
    let vector_blob = encode_f32_blob(&normalized_vector);
    let now = now_ms();
    conn.execute(
        "
        INSERT INTO image_clip_embeddings (
          image_id, model_id, model_version, dimension, normalized, vector_blob, source_modified_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?7)
        ON CONFLICT(image_id, model_id, model_version) DO UPDATE SET
          dimension = excluded.dimension,
          normalized = excluded.normalized,
          vector_blob = excluded.vector_blob,
          source_modified_at = excluded.source_modified_at,
          updated_at = excluded.updated_at
        ",
        params![
            image_id,
            CHINESE_CLIP_MODEL_ID,
            CHINESE_CLIP_MODEL_VERSION,
            dim,
            vector_blob,
            source_modified_at,
            now
        ],
    )
    .map_err(|error| format!("Failed to save CLIP embedding: {error}"))?;
    Ok(())
}

pub fn warmup_clip_vector_cache(state: &AppState) -> Result<(), String> {
    ensure_clip_vector_cache_loaded(state)
}

fn ensure_atmosphere_signature_cache_loaded(state: &AppState) -> Result<(), String> {
    {
        let cache = state
            .atmosphere_signature_cache
            .lock()
            .map_err(|_| "Atmosphere signature cache state is locked".to_string())?;
        if cache.is_some() {
            return Ok(());
        }
    }

    let conn = open_database(&state.database_path)?;
    let vectors = load_existing_atmosphere_signatures(&conn, None)?;
    let mut cache = state
        .atmosphere_signature_cache
        .lock()
        .map_err(|_| "Atmosphere signature cache state is locked".to_string())?;
    *cache = Some(SignatureCache {
        dimension: ATMOSPHERE_SIGNATURE_DIM,
        vectors,
    });
    Ok(())
}

fn ensure_color_signature_cache_loaded(state: &AppState) -> Result<(), String> {
    {
        let cache = state
            .color_signature_cache
            .lock()
            .map_err(|_| "Color signature cache state is locked".to_string())?;
        if cache.is_some() {
            return Ok(());
        }
    }

    let conn = open_database(&state.database_path)?;
    let vectors = load_existing_color_signatures(&conn, None)?;
    let mut cache = state
        .color_signature_cache
        .lock()
        .map_err(|_| "Color signature cache state is locked".to_string())?;
    *cache = Some(SignatureCache {
        dimension: COLOR_SIGNATURE_DIM,
        vectors,
    });
    Ok(())
}

fn ensure_clip_vector_cache_loaded(state: &AppState) -> Result<(), String> {
    {
        let cache = state
            .clip_vector_cache
            .lock()
            .map_err(|_| "Clip vector cache state is locked".to_string())?;
        if cache.is_some() {
            return Ok(());
        }
    }

    let conn = open_database(&state.database_path)?;
    let loaded = load_clip_vector_cache_from_database(&conn)?;
    let mut cache = state
        .clip_vector_cache
        .lock()
        .map_err(|_| "Clip vector cache state is locked".to_string())?;
    *cache = Some(loaded);
    Ok(())
}

fn clear_optional_cache<T>(cache: &Arc<Mutex<Option<T>>>) {
    if let Ok(mut value) = cache.lock() {
        *value = None;
    }
}

fn invalidate_all_similarity_caches(state: &AppState) {
    clear_optional_cache(&state.clip_vector_cache);
    clear_optional_cache(&state.atmosphere_signature_cache);
    clear_optional_cache(&state.color_signature_cache);
}

fn load_clip_vector_cache_from_database(conn: &Connection) -> Result<ClipImageVectorCache, String> {
    let mut stmt = conn
        .prepare(
            "
            SELECT e.image_id, e.dimension, e.normalized, e.vector_blob
            FROM image_clip_embeddings e
            JOIN images i ON i.id = e.image_id
            WHERE e.model_id = ?1
              AND e.model_version = ?2
              AND i.source = 'library'
              AND COALESCE(i.trashed, 0) = 0
            ",
        )
        .map_err(|error| format!("Failed to prepare clip vector cache query: {error}"))?;
    let rows = stmt
        .query_map(params![CHINESE_CLIP_MODEL_ID, CHINESE_CLIP_MODEL_VERSION], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Vec<u8>>(3)?,
            ))
        })
        .map_err(|error| format!("Failed to query clip vector cache: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to query clip vector cache: {error}"))?;
    drop(stmt);

    let mut vectors = HashMap::<String, Vec<f32>>::new();
    let mut dimension = 0usize;
    for (image_id, stored_dimension, normalized_flag, blob) in rows {
        let mut vector = decode_f32_blob(&blob)?;
        if vector.is_empty() {
            continue;
        }
        if normalized_flag == 0 {
            vector = normalize_vector(&vector);
        }
        let expected_dimension = usize::try_from(stored_dimension).unwrap_or(0);
        if expected_dimension > 0 && vector.len() != expected_dimension {
            continue;
        }
        if dimension == 0 {
            dimension = vector.len();
        }
        if vector.len() != dimension {
            continue;
        }
        vectors.insert(image_id, vector);
    }

    Ok(ClipImageVectorCache {
        model_id: CHINESE_CLIP_MODEL_ID.to_string(),
        model_version: CHINESE_CLIP_MODEL_VERSION.to_string(),
        dimension,
        vectors,
    })
}

fn remove_clip_vector_cache_entry(
    clip_vector_cache: &Arc<Mutex<Option<ClipImageVectorCache>>>,
    image_id: &str,
) -> Result<(), String> {
    let mut cache = clip_vector_cache
        .lock()
        .map_err(|_| "Clip vector cache state is locked".to_string())?;
    if let Some(cache) = cache.as_mut() {
        cache.vectors.remove(image_id);
    }
    Ok(())
}

fn upsert_clip_vector_cache_entry(
    clip_vector_cache: &Arc<Mutex<Option<ClipImageVectorCache>>>,
    image_id: &str,
    vector: Vec<f32>,
) {
    if vector.is_empty() {
        return;
    }
    if let Ok(mut cache) = clip_vector_cache.lock() {
        if let Some(cache) = cache.as_mut() {
            if cache.dimension == 0 {
                cache.dimension = vector.len();
            }
            if cache.dimension != vector.len() {
                return;
            }
            cache.vectors.insert(image_id.to_string(), vector);
        }
    }
}

fn remove_signature_cache_entry(
    signature_cache: &Arc<Mutex<Option<SignatureCache>>>,
    image_id: &str,
) -> Result<(), String> {
    let mut cache = signature_cache
        .lock()
        .map_err(|_| "Signature cache state is locked".to_string())?;
    if let Some(cache) = cache.as_mut() {
        cache.vectors.remove(image_id);
    }
    Ok(())
}

fn upsert_signature_cache_entry(
    signature_cache: &Arc<Mutex<Option<SignatureCache>>>,
    image_id: &str,
    vector: Vec<f32>,
    expected_dimension: usize,
) {
    if vector.is_empty() || vector.len() != expected_dimension {
        return;
    }
    if let Ok(mut cache) = signature_cache.lock() {
        if let Some(cache) = cache.as_mut() {
            if cache.dimension == 0 {
                cache.dimension = expected_dimension;
            }
            if cache.dimension != expected_dimension {
                return;
            }
            cache.vectors.insert(image_id.to_string(), vector);
        }
    }
}

fn stop_python_child_service<T, F>(service: &mut Option<T>, mut child_accessor: F)
where
    F: FnMut(&mut T) -> &mut Child,
{
    if let Some(mut running) = service.take() {
        let _ = child_accessor(&mut running).kill();
    }
}

fn release_wd_tagger_service(service: &Arc<Mutex<Option<WdTaggerService>>>) {
    if let Ok(mut guard) = service.lock() {
        stop_python_child_service(&mut *guard, |running| &mut running.child);
    }
}

fn release_clip_image_service(service: &Arc<Mutex<Option<ClipImageEncoderService>>>) {
    if let Ok(mut guard) = service.lock() {
        stop_python_child_service(&mut *guard, |running| &mut running.child);
    }
}

fn touch_clip_image_service_last_used(last_used_at: &Arc<Mutex<i64>>) {
    if let Ok(mut last_used) = last_used_at.lock() {
        *last_used = now_ms();
    }
}

fn ensure_clip_image_service_idle_reaper_started(
    service: &Arc<Mutex<Option<ClipImageEncoderService>>>,
    last_used_at: &Arc<Mutex<i64>>,
    worker_running: &Arc<Mutex<bool>>,
) {
    let should_spawn = if let Ok(mut running) = worker_running.lock() {
        if *running {
            false
        } else {
            *running = true;
            true
        }
    } else {
        false
    };
    if !should_spawn {
        return;
    }

    let service = Arc::clone(service);
    let last_used_at = Arc::clone(last_used_at);
    let worker_running = Arc::clone(worker_running);
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_millis(CLIP_IMAGE_SERVICE_IDLE_CHECK_INTERVAL_MS));

            let last_used = match last_used_at.lock() {
                Ok(value) => *value,
                Err(_) => 0,
            };
            let now = now_ms();
            if now.saturating_sub(last_used) < CLIP_IMAGE_SERVICE_IDLE_RELEASE_MS {
                let has_service = match service.lock() {
                    Ok(guard) => guard.is_some(),
                    Err(_) => false,
                };
                if has_service {
                    continue;
                }
                break;
            }
            let latest_last_used = match last_used_at.lock() {
                Ok(value) => *value,
                Err(_) => 0,
            };
            if now_ms().saturating_sub(latest_last_used) >= CLIP_IMAGE_SERVICE_IDLE_RELEASE_MS {
                release_clip_image_service(&service);
                break;
            }
        }

        if let Ok(mut running) = worker_running.lock() {
            *running = false;
        }
    });
}

fn run_chinese_clip_text_embedding_via_service(
    text: &str,
    state: &AppState,
) -> Result<Vec<f32>, String> {
    let model_root = resolve_chinese_clip_model_dir(None)?;
    let script_path = resolve_chinese_clip_text_service_script_path()?;
    let mut service_guard = state
        .clip_text_encoder_service
        .lock()
        .map_err(|_| "Clip text encoder service is locked".to_string())?;
    ensure_clip_text_service_started(&mut service_guard, &model_root, &script_path)?;
    let request = serde_json::json!({ "text": text });
    let primary = {
        let service = service_guard
            .as_mut()
            .ok_or_else(|| "Clip text encoder service unavailable".to_string())?;
        run_clip_service_request(&mut service.stdin, &mut service.stdout, request.clone(), "text")
    };
    let vector = match primary {
        Ok(vector) => vector,
        Err(first_error) => {
            stop_python_child_service(&mut service_guard, |running| &mut running.child);
            ensure_clip_text_service_started(&mut service_guard, &model_root, &script_path)?;
            let service = service_guard
                .as_mut()
                .ok_or_else(|| "Clip text encoder service unavailable after restart".to_string())?;
            run_clip_service_request(&mut service.stdin, &mut service.stdout, request, "text")
                .map_err(|second_error| {
                    format!(
                        "Clip text encoder failed and restart retry also failed. first: {first_error}; second: {second_error}"
                    )
                })?
        }
    };
    if vector.is_empty() {
        return Err("Clip text response embedding is empty".to_string());
    }
    Ok(normalize_vector(&vector))
}

fn ensure_clip_text_service_started(
    service: &mut Option<ClipTextEncoderService>,
    model_root: &Path,
    script_path: &Path,
) -> Result<(), String> {
    let need_restart = match service.as_ref() {
        None => true,
        Some(existing) => existing.model_root != model_root,
    };
    if !need_restart {
        return Ok(());
    }

    stop_python_child_service(service, |running| &mut running.child);
    *service = Some(spawn_clip_text_service(model_root, script_path)?);
    Ok(())
}

fn spawn_clip_text_service(model_root: &Path, script_path: &Path) -> Result<ClipTextEncoderService, String> {
    let mut command = python_command();
    let mut child = command
        .arg("-X")
        .arg("utf8")
        .arg(script_path)
        .arg("--model-dir")
        .arg(model_root)
        .arg("--provider")
        .arg("cpu")
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Failed to start clip text encoder service: {error}"))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Clip text encoder stdin unavailable".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Clip text encoder stdout unavailable".to_string())?;

    Ok(ClipTextEncoderService {
        child,
        stdin,
        stdout: BufReader::new(stdout),
        model_root: model_root.to_path_buf(),
    })
}

fn run_chinese_clip_image_embedding_via_service(
    image_path: &str,
    service: &mut ClipImageEncoderService,
) -> Result<Vec<f32>, String> {
    let request = serde_json::json!({ "image_path": image_path });
    let vector = run_clip_service_request(
        &mut service.stdin,
        &mut service.stdout,
        request,
        "image",
    )?;
    if vector.is_empty() {
        return Err("Clip image response embedding is empty".to_string());
    }
    Ok(normalize_vector(&vector))
}

fn run_chinese_clip_image_embedding_via_service_with_recovery(
    service: &mut Option<ClipImageEncoderService>,
    model_root: &Path,
    script_path: &Path,
    image_path: &str,
) -> Result<Vec<f32>, String> {
    ensure_clip_image_service_started(service, model_root, script_path)?;
    let primary = {
        let running = service
            .as_mut()
            .ok_or_else(|| "Clip image encoder service unavailable".to_string())?;
        run_chinese_clip_image_embedding_via_service(image_path, running)
    };
    match primary {
        Ok(vector) => Ok(vector),
        Err(first_error) => {
            stop_python_child_service(service, |running| &mut running.child);
            ensure_clip_image_service_started(service, model_root, script_path)?;
            let running = service
                .as_mut()
                .ok_or_else(|| "Clip image encoder service unavailable after restart".to_string())?;
            run_chinese_clip_image_embedding_via_service(image_path, running).map_err(|second_error| {
                format!(
                    "Clip image encoder failed and restart retry also failed. first: {first_error}; second: {second_error}"
                )
            })
        }
    }
}

fn ensure_clip_image_service_started(
    service: &mut Option<ClipImageEncoderService>,
    model_root: &Path,
    script_path: &Path,
) -> Result<(), String> {
    let need_restart = match service.as_ref() {
        None => true,
        Some(existing) => existing.model_root != model_root,
    };
    if !need_restart {
        return Ok(());
    }

    stop_python_child_service(service, |running| &mut running.child);
    *service = Some(spawn_clip_image_service(model_root, script_path)?);
    Ok(())
}

fn spawn_clip_image_service(model_root: &Path, script_path: &Path) -> Result<ClipImageEncoderService, String> {
    let mut command = python_command();
    let mut child = command
        .arg("-X")
        .arg("utf8")
        .arg(script_path)
        .arg("--model-dir")
        .arg(model_root)
        .arg("--provider")
        .arg("cpu")
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Failed to start clip image encoder service: {error}"))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Clip image encoder stdin unavailable".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Clip image encoder stdout unavailable".to_string())?;

    Ok(ClipImageEncoderService {
        child,
        stdin,
        stdout: BufReader::new(stdout),
        model_root: model_root.to_path_buf(),
    })
}

fn run_clip_service_request(
    stdin: &mut ChildStdin,
    stdout: &mut BufReader<ChildStdout>,
    request: serde_json::Value,
    mode: &str,
) -> Result<Vec<f32>, String> {
    stdin
        .write_all(request.to_string().as_bytes())
        .and_then(|_| stdin.write_all(b"\n"))
        .and_then(|_| stdin.flush())
        .map_err(|error| format!("Failed to write clip {mode} request: {error}"))?;

    let mut response_line = String::new();
    stdout
        .read_line(&mut response_line)
        .map_err(|error| format!("Failed to read clip {mode} response: {error}"))?;
    if response_line.trim().is_empty() {
        return Err(format!("Clip {mode} encoder returned empty response"));
    }
    let value: serde_json::Value = serde_json::from_str(response_line.trim())
        .map_err(|error| format!("Invalid clip {mode} response: {error}"))?;
    if let Some(error_text) = value.get("error").and_then(|item| item.as_str()) {
        return Err(format!("Clip {mode} encoder error: {error_text}"));
    }
    let array = value
        .get("embedding")
        .and_then(|item| item.as_array())
        .ok_or_else(|| format!("Clip {mode} response missing embedding"))?;
    let mut vector = Vec::<f32>::with_capacity(array.len());
    for item in array {
        let number = item
            .as_f64()
            .ok_or_else(|| format!("Clip {mode} response contains invalid value"))?;
        vector.push(number as f32);
    }
    Ok(vector)
}

fn encode_f32_blob(vector: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::<u8>::with_capacity(vector.len() * 4);
    for value in vector {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn decode_f32_blob(blob: &[u8]) -> Result<Vec<f32>, String> {
    if blob.len() % 4 != 0 {
        return Err("Invalid clip embedding blob length".to_string());
    }
    let mut values = Vec::<f32>::with_capacity(blob.len() / 4);
    for chunk in blob.chunks_exact(4) {
        values.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(values)
}

fn dot_product(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right.iter())
        .fold(0f32, |acc, (lv, rv)| acc + lv * rv)
}

fn legacy_model_name_to_model_id(legacy: &str) -> &str {
    let normalized = legacy.trim().to_lowercase();
    if normalized.contains("chinese-clip") || normalized.contains("cn_clip") {
        CHINESE_CLIP_MODEL_ID
    } else {
        CHINESE_CLIP_MODEL_ID
    }
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

fn clear_color_signature_cache_storage(conn: &Connection) -> Result<(), String> {
    conn.execute("DELETE FROM image_color_signatures", [])
        .map_err(|error| format!("Failed to clear color signature cache: {error}"))?;
    Ok(())
}

fn clear_atmosphere_signature_cache_storage(conn: &Connection) -> Result<(), String> {
    conn.execute("DELETE FROM image_atmosphere_signatures", [])
        .map_err(|error| format!("Failed to clear atmosphere signature cache: {error}"))?;
    Ok(())
}

fn clear_incompatible_color_signature_records(conn: &Connection) -> Result<i64, String> {
    let removed = conn
        .execute(
        "DELETE FROM image_color_signatures WHERE length(signature_blob) != ?1",
        params![(COLOR_SIGNATURE_DIM as i64) * 4],
    )
    .map_err(|error| format!("Failed to clear incompatible color signatures: {error}"))?;
    Ok(removed as i64)
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

fn atmosphere_stop_requested(stop_requested: &Arc<Mutex<bool>>) -> bool {
    stop_requested.lock().map(|value| *value).unwrap_or(false)
}

fn atmosphere_pause_requested(pause_requested: &Arc<Mutex<bool>>) -> bool {
    pause_requested.lock().map(|value| *value).unwrap_or(false)
}

fn wait_for_atmosphere_resume(
    progress: &Arc<Mutex<AtmosphereGenerationProgress>>,
    pause_requested: &Arc<Mutex<bool>>,
    stop_requested: &Arc<Mutex<bool>>,
) {
    loop {
        if atmosphere_stop_requested(stop_requested) {
            return;
        }
        if atmosphere_pause_requested(pause_requested) {
            set_atmosphere_progress_phase(progress, "paused");
            thread::sleep(std::time::Duration::from_millis(120));
            continue;
        }
        break;
    }
}

fn color_signature_stop_requested(stop_requested: &Arc<Mutex<bool>>) -> bool {
    stop_requested.lock().map(|value| *value).unwrap_or(false)
}

fn color_signature_pause_requested(pause_requested: &Arc<Mutex<bool>>) -> bool {
    pause_requested.lock().map(|value| *value).unwrap_or(false)
}

fn wait_for_color_signature_resume(
    progress: &Arc<Mutex<ColorSignatureGenerationProgress>>,
    pause_requested: &Arc<Mutex<bool>>,
    stop_requested: &Arc<Mutex<bool>>,
) {
    loop {
        if color_signature_stop_requested(stop_requested) {
            return;
        }
        if color_signature_pause_requested(pause_requested) {
            set_color_signature_progress_phase(progress, "paused");
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
    let dictionary_path = resolve_dictionary_source_path()?;
    if !dictionary_path.is_file() {
        return Ok(HashMap::new());
    }
    let exact_pairs = load_cn_tag_dictionary_pairs_from_source(&dictionary_path)?;
    Ok(build_lookup_dictionary_map(&exact_pairs))
}

fn load_cn_tag_dictionary_pairs_from_source(path: &Path) -> Result<HashMap<String, String>, String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if extension == "csv" {
        return load_cn_tag_dictionary_pairs_from_csv(path);
    }
    load_cn_tag_dictionary_pairs_from_xlsx(path)
}

fn load_cn_tag_dictionary_pairs_from_csv(path: &Path) -> Result<HashMap<String, String>, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read dictionary CSV {}: {error}", path.display()))?;
    let mut pairs = HashMap::<String, String>::new();
    for (line_index, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let fields = parse_csv_line(trimmed);
        let tag_en = fields
            .get(1)
            .map(|value| value.trim())
            .unwrap_or("");
        let tag_zh = fields
            .get(4)
            .map(|value| value.trim())
            .unwrap_or("");
        if tag_en.is_empty() || tag_zh.is_empty() {
            continue;
        }
        if line_index == 0
            && (tag_en.eq_ignore_ascii_case("tag")
                || tag_en.eq_ignore_ascii_case("url")
                || tag_en.eq_ignore_ascii_case("english")
                || tag_en.eq_ignore_ascii_case("en")
                || tag_zh.contains("翻译")
                || tag_zh.contains("中文"))
        {
            continue;
        }
        pairs.insert(tag_en.to_string(), tag_zh.to_string());
    }
    Ok(pairs)
}

fn load_cn_tag_dictionary_pairs_from_xlsx(path: &Path) -> Result<HashMap<String, String>, String> {
    let mut workbook = open_workbook_auto(path)
        .map_err(|error| format!("Failed to open dictionary workbook {}: {error}", path.display()))?;
    let sheet_name = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| "Dictionary workbook has no sheets".to_string())?;
    let range = workbook
        .worksheet_range(&sheet_name)
        .map_err(|error| format!("Failed to read dictionary sheet: {error}"))?;

    let mut rows = range.rows();
    let header = rows.next().ok_or_else(|| "Dictionary sheet is empty".to_string())?;
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

    let mut pairs = HashMap::<String, String>::new();
    for row in rows {
        let tag_en = excel_cell_to_string(row.get(tag_idx)).unwrap_or_default();
        let tag_zh = excel_cell_to_string(row.get(cn_idx)).unwrap_or_default();
        let tag_en = tag_en.trim();
        let tag_zh = tag_zh.trim();
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
        pairs.insert(tag_en.to_string(), tag_zh.to_string());
    }
    Ok(pairs)
}

fn build_lookup_dictionary_map(pairs: &HashMap<String, String>) -> HashMap<String, String> {
    let mut map = HashMap::<String, String>::with_capacity(pairs.len() * 2);
    for (tag_en, tag_zh) in pairs {
        map.insert(tag_en.clone(), tag_zh.clone());
        map.insert(normalize_tag_key(tag_en), tag_zh.clone());
    }
    map
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::<String>::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();
    let mut in_quotes = false;
    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                if in_quotes {
                    if matches!(chars.peek(), Some('"')) {
                        current.push('"');
                        let _ = chars.next();
                    } else {
                        in_quotes = false;
                    }
                } else {
                    in_quotes = true;
                }
            }
            ',' if !in_quotes => {
                fields.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    fields.push(current.trim().to_string());
    fields
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

fn resolve_dictionary_source_path() -> Result<PathBuf, String> {
    let mut candidates = Vec::<PathBuf>::new();
    if let Ok(cwd) = env::current_dir() {
        candidates.push(
            cwd.join("wd-swinv2-tagger-v3")
                .join("selected_tags_full_translation.csv"),
        );
        candidates.push(
            cwd.join("..")
                .join("wd-swinv2-tagger-v3")
                .join("selected_tags_full_translation.csv"),
        );
        candidates.push(cwd.join("wd-swinv2-tagger-v3").join("dictionary01.xlsx"));
        candidates.push(cwd.join("..").join("wd-swinv2-tagger-v3").join("dictionary01.xlsx"));
    }
    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.push(
                exe_dir
                    .join("wd-swinv2-tagger-v3")
                    .join("selected_tags_full_translation.csv"),
            );
            candidates.push(
                exe_dir
                    .join("..")
                    .join("wd-swinv2-tagger-v3")
                    .join("selected_tags_full_translation.csv"),
            );
            candidates.push(
                exe_dir
                    .join("..")
                    .join("..")
                    .join("wd-swinv2-tagger-v3")
                    .join("selected_tags_full_translation.csv"),
            );
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
    Ok(PathBuf::from("selected_tags_full_translation.csv"))
}

fn ensure_app_meta_table(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "
        CREATE TABLE IF NOT EXISTS app_meta (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL,
          updated_at INTEGER NOT NULL
        )
        ",
        [],
    )
    .map_err(|error| format!("Failed to ensure app_meta table: {error}"))?;
    Ok(())
}

fn read_app_meta_value(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT value FROM app_meta WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|error| format!("Failed to read app_meta ({key}): {error}"))
}

fn write_app_meta_value(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    let now = now_ms();
    conn.execute(
        "
        INSERT INTO app_meta (key, value, updated_at)
        VALUES (?1, ?2, ?3)
        ON CONFLICT(key) DO UPDATE SET
          value = excluded.value,
          updated_at = excluded.updated_at
        ",
        params![key, value, now],
    )
    .map_err(|error| format!("Failed to write app_meta ({key}): {error}"))?;
    Ok(())
}

fn sync_tag_dictionary_from_source_if_changed(state: &AppState) -> Result<(), String> {
    let source_path = resolve_dictionary_source_path()?;
    if !source_path.is_file() {
        return Ok(());
    }
    let metadata = fs::metadata(&source_path)
        .map_err(|error| format!("Failed to read dictionary source metadata: {error}"))?;
    let modified_at = metadata
        .modified()
        .ok()
        .map(system_time_ms)
        .unwrap_or(0);
    let signature = format!(
        "{}|{}|{}|{}",
        TAG_DICTIONARY_SOURCE_SCHEMA_VERSION,
        source_path.to_string_lossy(),
        modified_at,
        metadata.len()
    );

    let mut conn = open_database(&state.database_path)?;
    ensure_app_meta_table(&conn)?;
    let last_signature = read_app_meta_value(&conn, "tag_dictionary_source_signature")?;
    if last_signature.as_deref() == Some(signature.as_str()) {
        return Ok(());
    }

    let exact_pairs = load_cn_tag_dictionary_pairs_from_source(&source_path)?;
    let tx = conn
        .transaction()
        .map_err(|error| format!("Failed to open dictionary sync transaction: {error}"))?;
    for (tag_en, tag_zh) in &exact_pairs {
        tx.execute(
            "
            INSERT INTO tag_dictionary (tag_en, tag_zh, updated_at)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(tag_en) DO UPDATE SET
              tag_zh = excluded.tag_zh,
              updated_at = excluded.updated_at
            ",
            params![tag_en, tag_zh, now_ms()],
        )
        .map_err(|error| format!("Failed to upsert tag dictionary from source: {error}"))?;
    }
    write_app_meta_value(&tx, "tag_dictionary_source_signature", &signature)?;
    tx.commit()
        .map_err(|error| format!("Failed to commit dictionary sync transaction: {error}"))?;
    Ok(())
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

fn set_atmosphere_progress(
    progress: &Arc<Mutex<AtmosphereGenerationProgress>>,
    next: AtmosphereGenerationProgress,
) {
    if let Ok(mut state) = progress.lock() {
        *state = next;
    }
}

fn update_atmosphere_progress<F>(progress: &Arc<Mutex<AtmosphereGenerationProgress>>, update: F)
where
    F: FnOnce(&mut AtmosphereGenerationProgress),
{
    if let Ok(mut state) = progress.lock() {
        update(&mut state);
    }
}

fn set_atmosphere_progress_phase(progress: &Arc<Mutex<AtmosphereGenerationProgress>>, phase: &str) {
    update_atmosphere_progress(progress, |state| {
        state.phase = phase.to_string();
        state.running = phase != "idle";
        state.paused = phase == "paused";
    });
}

fn set_atmosphere_progress_total(progress: &Arc<Mutex<AtmosphereGenerationProgress>>, total: i64) {
    update_atmosphere_progress(progress, |state| {
        state.total_candidates = total.max(0);
    });
}

fn set_atmosphere_progress_counts(
    progress: &Arc<Mutex<AtmosphereGenerationProgress>>,
    processed: i64,
    generated: i64,
    skipped: i64,
    failed: i64,
) {
    update_atmosphere_progress(progress, |state| {
        state.processed_images = processed.max(0);
        state.generated_images = generated.max(0);
        state.skipped_images = skipped.max(0);
        state.failed_images = failed.max(0);
    });
}

fn set_atmosphere_progress_error(progress: &Arc<Mutex<AtmosphereGenerationProgress>>, error: &str) {
    update_atmosphere_progress(progress, |state| {
        state.last_error = Some(error.to_string());
    });
}

fn push_atmosphere_progress_recent_error(
    progress: &Arc<Mutex<AtmosphereGenerationProgress>>,
    error: &str,
) {
    update_atmosphere_progress(progress, |state| {
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

fn set_atmosphere_progress_done(progress: &Arc<Mutex<AtmosphereGenerationProgress>>) {
    update_atmosphere_progress(progress, |state| {
        state.running = false;
        state.paused = false;
        state.phase = "idle".to_string();
    });
}

fn set_color_signature_progress(
    progress: &Arc<Mutex<ColorSignatureGenerationProgress>>,
    next: ColorSignatureGenerationProgress,
) {
    if let Ok(mut state) = progress.lock() {
        *state = next;
    }
}

fn update_color_signature_progress<F>(
    progress: &Arc<Mutex<ColorSignatureGenerationProgress>>,
    update: F,
) where
    F: FnOnce(&mut ColorSignatureGenerationProgress),
{
    if let Ok(mut state) = progress.lock() {
        update(&mut state);
    }
}

fn set_color_signature_progress_phase(
    progress: &Arc<Mutex<ColorSignatureGenerationProgress>>,
    phase: &str,
) {
    update_color_signature_progress(progress, |state| {
        state.phase = phase.to_string();
        state.running = phase != "idle";
        state.paused = phase == "paused";
    });
}

fn set_color_signature_progress_total(
    progress: &Arc<Mutex<ColorSignatureGenerationProgress>>,
    total: i64,
) {
    update_color_signature_progress(progress, |state| {
        state.total_candidates = total.max(0);
    });
}

fn set_color_signature_progress_counts(
    progress: &Arc<Mutex<ColorSignatureGenerationProgress>>,
    processed: i64,
    generated: i64,
    skipped: i64,
    failed: i64,
) {
    update_color_signature_progress(progress, |state| {
        state.processed_images = processed.max(0);
        state.generated_images = generated.max(0);
        state.skipped_images = skipped.max(0);
        state.failed_images = failed.max(0);
    });
}

fn set_color_signature_progress_error(
    progress: &Arc<Mutex<ColorSignatureGenerationProgress>>,
    error: &str,
) {
    update_color_signature_progress(progress, |state| {
        state.last_error = Some(error.to_string());
    });
}

fn push_color_signature_progress_recent_error(
    progress: &Arc<Mutex<ColorSignatureGenerationProgress>>,
    error: &str,
) {
    update_color_signature_progress(progress, |state| {
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

fn set_color_signature_progress_done(progress: &Arc<Mutex<ColorSignatureGenerationProgress>>) {
    update_color_signature_progress(progress, |state| {
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
        state.paused = phase == "paused";
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
        state.paused = false;
        state.phase = "idle".to_string();
    });
}

fn natural_language_scan_stop_requested(stop_requested: &Arc<Mutex<bool>>) -> bool {
    stop_requested.lock().map(|value| *value).unwrap_or(false)
}

fn natural_language_scan_pause_requested(pause_requested: &Arc<Mutex<bool>>) -> bool {
    pause_requested.lock().map(|value| *value).unwrap_or(false)
}

fn wait_for_natural_language_scan_resume(
    progress: &Arc<Mutex<NaturalLanguageScanProgress>>,
    pause_requested: &Arc<Mutex<bool>>,
    stop_requested: &Arc<Mutex<bool>>,
) {
    loop {
        if natural_language_scan_stop_requested(stop_requested) {
            return;
        }
        if natural_language_scan_pause_requested(pause_requested) {
            set_natural_language_scan_progress_phase(progress, "paused");
            thread::sleep(Duration::from_millis(120));
            continue;
        }
        set_natural_language_scan_progress_phase(progress, "generating");
        return;
    }
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
        state.running = phase != "idle";
        state.paused = phase == "paused";
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
        state.paused = false;
        state.phase = "idle".to_string();
    });
}

fn background_scan_stop_requested(stop_requested: &Arc<Mutex<bool>>) -> bool {
    stop_requested.lock().map(|value| *value).unwrap_or(false)
}

fn background_scan_pause_requested(pause_requested: &Arc<Mutex<bool>>) -> bool {
    pause_requested.lock().map(|value| *value).unwrap_or(false)
}

fn wait_for_background_scan_resume(
    progress: &Arc<Mutex<BackgroundScanProgress>>,
    pause_requested: &Arc<Mutex<bool>>,
    stop_requested: &Arc<Mutex<bool>>,
    resume_phase: &str,
) {
    loop {
        if background_scan_stop_requested(stop_requested) {
            return;
        }
        if background_scan_pause_requested(pause_requested) {
            set_scan_progress_phase(progress, "paused");
            thread::sleep(Duration::from_millis(120));
            continue;
        }
        set_scan_progress_phase(progress, resume_phase);
        return;
    }
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

fn resolve_wd_tagger_service_script_path() -> Result<PathBuf, String> {
    let mut candidates = Vec::<PathBuf>::new();
    if let Ok(cwd) = env::current_dir() {
        candidates.push(cwd.join("src-tauri").join("scripts").join("wd_tagger_service.py"));
        candidates.push(cwd.join("scripts").join("wd_tagger_service.py"));
    }
    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.push(exe_dir.join("scripts").join("wd_tagger_service.py"));
            candidates.push(exe_dir.join("..").join("scripts").join("wd_tagger_service.py"));
            candidates.push(
                exe_dir
                    .join("..")
                    .join("..")
                    .join("src-tauri")
                    .join("scripts")
                    .join("wd_tagger_service.py"),
            );
        }
    }

    for candidate in candidates {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    Err("Cannot find wd_tagger_service.py under src-tauri/scripts".to_string())
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

fn resolve_chinese_clip_image_service_script_path() -> Result<PathBuf, String> {
    let mut candidates = Vec::<PathBuf>::new();
    if let Ok(cwd) = env::current_dir() {
        candidates.push(
            cwd.join("src-tauri")
                .join("scripts")
                .join("chinese_clip_image_service.py"),
        );
        candidates.push(cwd.join("scripts").join("chinese_clip_image_service.py"));
    }
    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.push(exe_dir.join("scripts").join("chinese_clip_image_service.py"));
            candidates.push(exe_dir.join("..").join("scripts").join("chinese_clip_image_service.py"));
            candidates.push(
                exe_dir
                    .join("..")
                    .join("..")
                    .join("src-tauri")
                    .join("scripts")
                    .join("chinese_clip_image_service.py"),
            );
        }
    }

    for candidate in candidates {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    Err("Cannot find chinese_clip_image_service.py under src-tauri/scripts".to_string())
}

fn resolve_chinese_clip_text_service_script_path() -> Result<PathBuf, String> {
    let mut candidates = Vec::<PathBuf>::new();
    if let Ok(cwd) = env::current_dir() {
        candidates.push(
            cwd.join("src-tauri")
                .join("scripts")
                .join("chinese_clip_text_service.py"),
        );
        candidates.push(cwd.join("scripts").join("chinese_clip_text_service.py"));
    }
    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.push(exe_dir.join("scripts").join("chinese_clip_text_service.py"));
            candidates.push(exe_dir.join("..").join("scripts").join("chinese_clip_text_service.py"));
            candidates.push(
                exe_dir
                    .join("..")
                    .join("..")
                    .join("src-tauri")
                    .join("scripts")
                    .join("chinese_clip_text_service.py"),
            );
        }
    }

    for candidate in candidates {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    Err("Cannot find chinese_clip_text_service.py under src-tauri/scripts".to_string())
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
