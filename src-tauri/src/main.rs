use illutag_core::library::{
    add_gallery_folder, add_image_to_reference_board, assign_image_to_user_folder,
    background_scan_progress,
    background_scan_status,
    pause_background_scan,
    resume_background_scan,
    stop_background_scan,
    natural_language_scan_progress,
    natural_language_scan_status,
    pause_natural_language_scan,
    resume_natural_language_scan,
    search_gallery_image_ids_by_external_image,
    search_gallery_image_ids_by_natural_language,
    start_natural_language_scan,
    stop_natural_language_scan,
    warmup_clip_vector_cache,
    copy_image_to_system_clipboard,
    start_thumbnail_generation,
    start_atmosphere_generation,
    pause_atmosphere_generation,
    resume_atmosphere_generation,
    stop_atmosphere_generation,
    rebuild_atmosphere_signature_cache,
    atmosphere_generation_status,
    start_color_signature_generation,
    pause_color_signature_generation,
    resume_color_signature_generation,
    stop_color_signature_generation,
    rebuild_color_signature_cache,
    color_signature_generation_status,
    pause_thumbnail_generation,
    resume_thumbnail_generation,
    stop_thumbnail_generation,
    clear_thumbnail_cache,
    rebuild_thumbnail_cache,
    thumbnail_generation_status,
    auto_arrange_reference_board, create_reference_board, create_reference_board_folder,
    create_user_folder, delete_reference_board, delete_reference_board_folder, delete_user_folder,
    duplicate_reference_board_item, export_gallery_image_from_state, export_reference_board_item_from_state,
    import_reference_board_item_to_library, list_library_from_state,
    list_image_auto_tags, list_image_user_tags, add_image_user_custom_tag, remove_image_user_custom_tag,
    add_image_user_supplement_tag, remove_image_user_supplement_tag,
    list_tag_management_state, create_user_tag_folder, rename_user_tag_folder, delete_user_tag_folder,
    assign_user_tag_to_folder, unassign_user_tag_from_folder, create_user_custom_tag, delete_user_custom_tag,
    search_gallery_image_ids, start_startup_cleanup, startup_cleanup_status, suggest_known_auto_tags,
    move_reference_board_to_folder, paste_image_to_reference_board, read_image_bytes,
    remove_gallery_folder, remove_image_from_index, remove_image_from_user_folder, remove_reference_board_item,
    restore_image_from_trash,
    toggle_image_favorite,
    restore_reference_board_item,
    rename_reference_board, rename_reference_board_folder, reorder_reference_board,
    reorder_reference_board_folder, reorder_user_folder, rename_user_folder, update_reference_board_item_layout,
    bring_reference_board_item_to_front, start_scan_all_folders_with_tagging, test_wd_swinv2_tagger,
    AppState, AtmosphereGenerationProgress, BackgroundScanProgress, BackgroundScanStatus, ColorSignatureGenerationProgress, GallerySearchFilters, ImageAutoTagSummary, ImageBytes, ImageUserTagSummary, KnownAutoTagSuggestion, LibraryStore, NaturalLanguageScanProgress, NaturalLanguageScanStatus, StartupCleanupStatus, TagManagementState, ThumbnailGenerationProgress,
    WdTaggerTestResult,
};
use std::sync::{Arc, Mutex};
use tauri::{Manager, State};

#[tauri::command]
fn list_library(state: State<AppState>) -> Result<LibraryStore, String> {
    list_library_from_state(&state)
}

#[tauri::command]
fn add_gallery_folder_command(
    folder_path: String,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    add_gallery_folder(folder_path, &state)
}

#[tauri::command]
fn remove_gallery_folder_command(
    folder_path: String,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    remove_gallery_folder(folder_path, &state)
}

#[tauri::command]
fn remove_image_from_index_command(
    image_id: String,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    remove_image_from_index(image_id, &state)
}

#[tauri::command]
fn restore_image_from_trash_command(
    image_id: String,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    restore_image_from_trash(image_id, &state)
}

#[tauri::command]
fn toggle_image_favorite_command(
    image_id: String,
    favorite: bool,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    toggle_image_favorite(image_id, favorite, &state)
}

#[tauri::command]
fn read_image_bytes_command(
    image_id: String,
    state: State<AppState>,
) -> Result<ImageBytes, String> {
    read_image_bytes(image_id, &state)
}

#[tauri::command]
fn copy_image_to_system_clipboard_command(
    image_id: String,
    state: State<AppState>,
) -> Result<(), String> {
    copy_image_to_system_clipboard(image_id, &state)
}

#[tauri::command]
fn test_wd_swinv2_tagger_command(
    image_id: String,
    general_threshold: Option<f32>,
    character_threshold: Option<f32>,
    model_dir: Option<String>,
    state: State<AppState>,
) -> Result<WdTaggerTestResult, String> {
    test_wd_swinv2_tagger(
        image_id,
        general_threshold,
        character_threshold,
        model_dir,
        &state,
    )
}

#[tauri::command]
fn start_scan_all_folders_with_tagging_command(state: State<AppState>) -> Result<bool, String> {
    start_scan_all_folders_with_tagging(&state)
}

#[tauri::command]
fn background_scan_status_command(state: State<AppState>) -> Result<BackgroundScanStatus, String> {
    background_scan_status(&state)
}

#[tauri::command]
fn background_scan_progress_command(state: State<AppState>) -> Result<BackgroundScanProgress, String> {
    background_scan_progress(&state)
}

#[tauri::command]
fn pause_background_scan_command(state: State<AppState>) -> Result<bool, String> {
    pause_background_scan(&state)
}

#[tauri::command]
fn resume_background_scan_command(state: State<AppState>) -> Result<bool, String> {
    resume_background_scan(&state)
}

#[tauri::command]
fn stop_background_scan_command(state: State<AppState>) -> Result<bool, String> {
    stop_background_scan(&state)
}

#[tauri::command]
fn start_natural_language_scan_command(state: State<AppState>) -> Result<bool, String> {
    start_natural_language_scan(&state)
}

#[tauri::command]
fn natural_language_scan_status_command(state: State<AppState>) -> Result<NaturalLanguageScanStatus, String> {
    natural_language_scan_status(&state)
}

#[tauri::command]
fn natural_language_scan_progress_command(state: State<AppState>) -> Result<NaturalLanguageScanProgress, String> {
    natural_language_scan_progress(&state)
}

#[tauri::command]
fn pause_natural_language_scan_command(state: State<AppState>) -> Result<bool, String> {
    pause_natural_language_scan(&state)
}

#[tauri::command]
fn resume_natural_language_scan_command(state: State<AppState>) -> Result<bool, String> {
    resume_natural_language_scan(&state)
}

#[tauri::command]
fn stop_natural_language_scan_command(state: State<AppState>) -> Result<bool, String> {
    stop_natural_language_scan(&state)
}

#[tauri::command]
fn start_startup_cleanup_command(state: State<AppState>) -> Result<bool, String> {
    start_startup_cleanup(&state)
}

#[tauri::command]
fn startup_cleanup_status_command(state: State<AppState>) -> Result<StartupCleanupStatus, String> {
    startup_cleanup_status(&state)
}

#[tauri::command]
fn start_thumbnail_generation_command(state: State<AppState>) -> Result<bool, String> {
    start_thumbnail_generation(&state)
}

#[tauri::command]
fn thumbnail_generation_status_command(state: State<AppState>) -> Result<ThumbnailGenerationProgress, String> {
    thumbnail_generation_status(&state)
}

#[tauri::command]
fn pause_thumbnail_generation_command(state: State<AppState>) -> Result<bool, String> {
    pause_thumbnail_generation(&state)
}

#[tauri::command]
fn resume_thumbnail_generation_command(state: State<AppState>) -> Result<bool, String> {
    resume_thumbnail_generation(&state)
}

#[tauri::command]
fn stop_thumbnail_generation_command(state: State<AppState>) -> Result<bool, String> {
    stop_thumbnail_generation(&state)
}

#[tauri::command]
fn clear_thumbnail_cache_command(state: State<AppState>) -> Result<(), String> {
    clear_thumbnail_cache(&state)
}

#[tauri::command]
fn rebuild_thumbnail_cache_command(state: State<AppState>) -> Result<bool, String> {
    rebuild_thumbnail_cache(&state)
}

#[tauri::command]
fn start_atmosphere_generation_command(state: State<AppState>) -> Result<bool, String> {
    start_atmosphere_generation(&state)
}

#[tauri::command]
fn atmosphere_generation_status_command(state: State<AppState>) -> Result<AtmosphereGenerationProgress, String> {
    atmosphere_generation_status(&state)
}

#[tauri::command]
fn pause_atmosphere_generation_command(state: State<AppState>) -> Result<bool, String> {
    pause_atmosphere_generation(&state)
}

#[tauri::command]
fn resume_atmosphere_generation_command(state: State<AppState>) -> Result<bool, String> {
    resume_atmosphere_generation(&state)
}

#[tauri::command]
fn stop_atmosphere_generation_command(state: State<AppState>) -> Result<bool, String> {
    stop_atmosphere_generation(&state)
}

#[tauri::command]
fn rebuild_atmosphere_signature_cache_command(state: State<AppState>) -> Result<bool, String> {
    rebuild_atmosphere_signature_cache(&state)
}

#[tauri::command]
fn start_color_signature_generation_command(state: State<AppState>) -> Result<bool, String> {
    start_color_signature_generation(&state)
}

#[tauri::command]
fn color_signature_generation_status_command(
    state: State<AppState>,
) -> Result<ColorSignatureGenerationProgress, String> {
    color_signature_generation_status(&state)
}

#[tauri::command]
fn pause_color_signature_generation_command(state: State<AppState>) -> Result<bool, String> {
    pause_color_signature_generation(&state)
}

#[tauri::command]
fn resume_color_signature_generation_command(state: State<AppState>) -> Result<bool, String> {
    resume_color_signature_generation(&state)
}

#[tauri::command]
fn stop_color_signature_generation_command(state: State<AppState>) -> Result<bool, String> {
    stop_color_signature_generation(&state)
}

#[tauri::command]
fn rebuild_color_signature_cache_command(state: State<AppState>) -> Result<bool, String> {
    rebuild_color_signature_cache(&state)
}

#[tauri::command]
fn list_image_auto_tags_command(
    image_id: String,
    state: State<AppState>,
) -> Result<ImageAutoTagSummary, String> {
    list_image_auto_tags(image_id, &state)
}

#[tauri::command]
fn list_image_user_tags_command(
    image_id: String,
    state: State<AppState>,
) -> Result<ImageUserTagSummary, String> {
    list_image_user_tags(image_id, &state)
}

#[tauri::command]
fn add_image_user_custom_tag_command(
    image_id: String,
    tag_text: String,
    state: State<AppState>,
) -> Result<ImageUserTagSummary, String> {
    add_image_user_custom_tag(image_id, tag_text, &state)
}

#[tauri::command]
fn remove_image_user_custom_tag_command(
    image_id: String,
    tag_text: String,
    state: State<AppState>,
) -> Result<ImageUserTagSummary, String> {
    remove_image_user_custom_tag(image_id, tag_text, &state)
}

#[tauri::command]
fn add_image_user_supplement_tag_command(
    image_id: String,
    tag_en: String,
    tag_zh: Option<String>,
    state: State<AppState>,
) -> Result<ImageUserTagSummary, String> {
    add_image_user_supplement_tag(image_id, tag_en, tag_zh, &state)
}

#[tauri::command]
fn remove_image_user_supplement_tag_command(
    image_id: String,
    tag_en: String,
    state: State<AppState>,
) -> Result<ImageUserTagSummary, String> {
    remove_image_user_supplement_tag(image_id, tag_en, &state)
}

#[tauri::command]
fn list_tag_management_state_command(state: State<AppState>) -> Result<TagManagementState, String> {
    list_tag_management_state(&state)
}

#[tauri::command]
fn create_user_tag_folder_command(
    name: String,
    state: State<AppState>,
) -> Result<TagManagementState, String> {
    create_user_tag_folder(name, &state)
}

#[tauri::command]
fn create_user_custom_tag_command(
    tag_text: String,
    state: State<AppState>,
) -> Result<TagManagementState, String> {
    create_user_custom_tag(tag_text, &state)
}

#[tauri::command]
fn delete_user_custom_tag_command(
    tag_text: String,
    state: State<AppState>,
) -> Result<TagManagementState, String> {
    delete_user_custom_tag(tag_text, &state)
}

#[tauri::command]
fn rename_user_tag_folder_command(
    folder_id: i64,
    name: String,
    state: State<AppState>,
) -> Result<TagManagementState, String> {
    rename_user_tag_folder(folder_id, name, &state)
}

#[tauri::command]
fn delete_user_tag_folder_command(
    folder_id: i64,
    state: State<AppState>,
) -> Result<TagManagementState, String> {
    delete_user_tag_folder(folder_id, &state)
}

#[tauri::command]
fn assign_user_tag_to_folder_command(
    folder_id: i64,
    tag_text: String,
    state: State<AppState>,
) -> Result<TagManagementState, String> {
    assign_user_tag_to_folder(folder_id, tag_text, &state)
}

#[tauri::command]
fn unassign_user_tag_from_folder_command(
    tag_text: String,
    state: State<AppState>,
) -> Result<TagManagementState, String> {
    unassign_user_tag_from_folder(tag_text, &state)
}

#[tauri::command]
fn suggest_known_auto_tags_command(
    query: String,
    limit: Option<i64>,
    include_user_custom: Option<bool>,
    include_dictionary: Option<bool>,
    state: State<AppState>,
) -> Result<Vec<KnownAutoTagSuggestion>, String> {
    suggest_known_auto_tags(query, limit, include_user_custom, include_dictionary, &state)
}

#[tauri::command]
fn search_gallery_image_ids_command(
    filters: GallerySearchFilters,
    state: State<AppState>,
) -> Result<Vec<String>, String> {
    search_gallery_image_ids(filters, &state)
}

#[tauri::command]
fn search_gallery_image_ids_by_natural_language_command(
    query: String,
    candidate_image_ids: Option<Vec<String>>,
    state: State<AppState>,
) -> Result<Vec<String>, String> {
    search_gallery_image_ids_by_natural_language(query, candidate_image_ids, &state)
}

#[tauri::command]
fn search_gallery_image_ids_by_external_image_command(
    image_path: Option<String>,
    image_url: Option<String>,
    image_bytes: Option<Vec<u8>>,
    image_base64: Option<String>,
    search_type: Option<String>,
    candidate_image_ids: Option<Vec<String>>,
    limit: Option<usize>,
    state: State<AppState>,
) -> Result<Vec<String>, String> {
    search_gallery_image_ids_by_external_image(
        image_path,
        image_url,
        image_bytes,
        image_base64,
        search_type,
        candidate_image_ids,
        limit,
        &state,
    )
}

#[tauri::command]
fn create_user_folder_command(
    parent_id: Option<i64>,
    name: String,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    create_user_folder(parent_id, name, &state)
}

#[tauri::command]
fn rename_user_folder_command(
    folder_id: i64,
    name: String,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    rename_user_folder(folder_id, name, &state)
}

#[tauri::command]
fn delete_user_folder_command(
    folder_id: i64,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    delete_user_folder(folder_id, &state)
}

#[tauri::command]
fn reorder_user_folder_command(
    folder_id: i64,
    target_folder_id: i64,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    reorder_user_folder(folder_id, target_folder_id, &state)
}

#[tauri::command]
fn assign_image_to_user_folder_command(
    image_id: String,
    folder_id: i64,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    assign_image_to_user_folder(image_id, folder_id, &state)
}

#[tauri::command]
fn remove_image_from_user_folder_command(
    image_id: String,
    folder_id: i64,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    remove_image_from_user_folder(image_id, folder_id, &state)
}

#[tauri::command]
fn create_reference_board_folder_command(
    name: String,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    create_reference_board_folder(name, &state)
}

#[tauri::command]
fn create_reference_board_command(
    folder_id: Option<i64>,
    name: String,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    create_reference_board(folder_id, name, &state)
}

#[tauri::command]
fn add_image_to_reference_board_command(
    image_id: String,
    board_id: i64,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    add_image_to_reference_board(image_id, board_id, &state)
}

#[tauri::command]
fn paste_image_to_reference_board_command(
    board_id: i64,
    image_bytes: Vec<u8>,
    mime_type: String,
    x: f32,
    y: f32,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    paste_image_to_reference_board(board_id, image_bytes, mime_type, x, y, &state)
}

#[tauri::command]
fn duplicate_reference_board_item_command(
    item_id: i64,
    x: Option<f32>,
    y: Option<f32>,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    duplicate_reference_board_item(item_id, x, y, &state)
}

#[tauri::command]
fn restore_reference_board_item_command(
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
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    restore_reference_board_item(
        board_id, image_id, x, y, width, height, rotation, flip_x, flip_y, z_index, &state,
    )
}

#[tauri::command]
fn import_reference_board_item_to_library_command(
    item_id: i64,
    folder_id: i64,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    import_reference_board_item_to_library(item_id, folder_id, &state)
}

#[tauri::command]
fn export_reference_board_item_command(
    item_id: i64,
    destination: String,
    state: State<AppState>,
) -> Result<(), String> {
    export_reference_board_item_from_state(item_id, destination, &state)
}

#[tauri::command]
fn export_gallery_image_command(
    image_id: String,
    destination: String,
    state: State<AppState>,
) -> Result<(), String> {
    export_gallery_image_from_state(image_id, destination, &state)
}

#[tauri::command]
fn rename_reference_board_command(
    board_id: i64,
    name: String,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    rename_reference_board(board_id, name, &state)
}

#[tauri::command]
fn rename_reference_board_folder_command(
    folder_id: i64,
    name: String,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    rename_reference_board_folder(folder_id, name, &state)
}

#[tauri::command]
fn reorder_reference_board_folder_command(
    folder_id: i64,
    target_folder_id: i64,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    reorder_reference_board_folder(folder_id, target_folder_id, &state)
}

#[tauri::command]
fn move_reference_board_to_folder_command(
    board_id: i64,
    folder_id: Option<i64>,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    move_reference_board_to_folder(board_id, folder_id, &state)
}

#[tauri::command]
fn reorder_reference_board_command(
    board_id: i64,
    target_board_id: i64,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    reorder_reference_board(board_id, target_board_id, &state)
}

#[tauri::command]
fn delete_reference_board_command(
    board_id: i64,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    delete_reference_board(board_id, &state)
}

#[tauri::command]
fn delete_reference_board_folder_command(
    folder_id: i64,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    delete_reference_board_folder(folder_id, &state)
}

#[tauri::command]
fn remove_reference_board_item_command(
    item_id: i64,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    remove_reference_board_item(item_id, &state)
}

#[tauri::command]
fn update_reference_board_item_layout_command(
    item_id: i64,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    rotation: f32,
    flip_x: bool,
    flip_y: bool,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    update_reference_board_item_layout(item_id, x, y, width, height, rotation, flip_x, flip_y, &state)
}

#[tauri::command]
fn bring_reference_board_item_to_front_command(
    item_id: i64,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    bring_reference_board_item_to_front(item_id, &state)
}

#[tauri::command]
fn auto_arrange_reference_board_command(
    board_id: i64,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    auto_arrange_reference_board(board_id, &state)
}

#[tauri::command]
fn window_minimize_command(window: tauri::Window) -> Result<(), String> {
    window
        .minimize()
        .map_err(|error| format!("最小化窗口失败：{error}"))
}

#[tauri::command]
fn window_toggle_maximize_command(window: tauri::Window) -> Result<bool, String> {
    let maximized = window
        .is_maximized()
        .map_err(|error| format!("读取窗口最大化状态失败：{error}"))?;
    if maximized {
        window
            .unmaximize()
            .map_err(|error| format!("还原窗口失败：{error}"))?;
    } else {
        window
            .maximize()
            .map_err(|error| format!("最大化窗口失败：{error}"))?;
    }
    window
        .is_maximized()
        .map_err(|error| format!("更新窗口最大化状态失败：{error}"))
}

#[tauri::command]
fn window_is_maximized_command(window: tauri::Window) -> Result<bool, String> {
    window
        .is_maximized()
        .map_err(|error| format!("读取窗口最大化状态失败：{error}"))
}

#[tauri::command]
fn window_close_command(window: tauri::Window) -> Result<(), String> {
    window
        .close()
        .map_err(|error| format!("关闭窗口失败：{error}"))
}

#[tauri::command]
fn window_start_dragging_command(window: tauri::Window) -> Result<(), String> {
    window
        .start_dragging()
        .map_err(|error| format!("failed to start dragging window: {error}"))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| Box::<dyn std::error::Error>::from(error))?;

            let app_state = AppState {
                database_path: app_data_dir.join("illutag.sqlite"),
                library: Arc::new(Mutex::new(None)),
                background_scan_running: Arc::new(Mutex::new(false)),
                background_scan_pending: Arc::new(Mutex::new(false)),
                background_scan_pause_requested: Arc::new(Mutex::new(false)),
                background_scan_stop_requested: Arc::new(Mutex::new(false)),
                background_scan_progress: Arc::new(Mutex::new(BackgroundScanProgress::default())),
                startup_cleanup_running: Arc::new(Mutex::new(false)),
                startup_cleanup_generation: Arc::new(Mutex::new(0)),
                thumbnail_generation_running: Arc::new(Mutex::new(false)),
                thumbnail_generation_pending: Arc::new(Mutex::new(false)),
                thumbnail_generation_pause_requested: Arc::new(Mutex::new(false)),
                thumbnail_generation_stop_requested: Arc::new(Mutex::new(false)),
                thumbnail_generation_progress: Arc::new(Mutex::new(ThumbnailGenerationProgress::default())),
                atmosphere_generation_running: Arc::new(Mutex::new(false)),
                atmosphere_generation_pending: Arc::new(Mutex::new(false)),
                atmosphere_generation_pause_requested: Arc::new(Mutex::new(false)),
                atmosphere_generation_stop_requested: Arc::new(Mutex::new(false)),
                atmosphere_generation_progress: Arc::new(Mutex::new(AtmosphereGenerationProgress::default())),
                color_signature_generation_running: Arc::new(Mutex::new(false)),
                color_signature_generation_pending: Arc::new(Mutex::new(false)),
                color_signature_generation_pause_requested: Arc::new(Mutex::new(false)),
                color_signature_generation_stop_requested: Arc::new(Mutex::new(false)),
                color_signature_generation_progress: Arc::new(Mutex::new(ColorSignatureGenerationProgress::default())),
                natural_language_scan_running: Arc::new(Mutex::new(false)),
                natural_language_scan_pending: Arc::new(Mutex::new(false)),
                natural_language_scan_pause_requested: Arc::new(Mutex::new(false)),
                natural_language_scan_stop_requested: Arc::new(Mutex::new(false)),
                natural_language_scan_progress: Arc::new(Mutex::new(NaturalLanguageScanProgress::default())),
                clip_vector_cache: Arc::new(Mutex::new(None)),
                atmosphere_signature_cache: Arc::new(Mutex::new(None)),
                color_signature_cache: Arc::new(Mutex::new(None)),
                clip_text_encoder_service: Arc::new(Mutex::new(None)),
                clip_image_encoder_service: Arc::new(Mutex::new(None)),
                clip_image_encoder_last_used_at: Arc::new(Mutex::new(0)),
                clip_image_encoder_release_worker_running: Arc::new(Mutex::new(false)),
                wd_tagger_service: Arc::new(Mutex::new(None)),
            };
            let _ = warmup_clip_vector_cache(&app_state);
            app.manage(app_state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_library,
            add_gallery_folder_command,
            remove_gallery_folder_command,
            remove_image_from_index_command,
            restore_image_from_trash_command,
            toggle_image_favorite_command,
            read_image_bytes_command,
            copy_image_to_system_clipboard_command,
            test_wd_swinv2_tagger_command,
            start_scan_all_folders_with_tagging_command,
            background_scan_status_command,
            background_scan_progress_command,
            pause_background_scan_command,
            resume_background_scan_command,
            stop_background_scan_command,
            start_natural_language_scan_command,
            natural_language_scan_status_command,
            natural_language_scan_progress_command,
            pause_natural_language_scan_command,
            resume_natural_language_scan_command,
            stop_natural_language_scan_command,
            start_startup_cleanup_command,
            startup_cleanup_status_command,
            start_thumbnail_generation_command,
            thumbnail_generation_status_command,
            pause_thumbnail_generation_command,
            resume_thumbnail_generation_command,
            stop_thumbnail_generation_command,
            clear_thumbnail_cache_command,
            rebuild_thumbnail_cache_command,
            start_atmosphere_generation_command,
            atmosphere_generation_status_command,
            pause_atmosphere_generation_command,
            resume_atmosphere_generation_command,
            stop_atmosphere_generation_command,
            rebuild_atmosphere_signature_cache_command,
            start_color_signature_generation_command,
            color_signature_generation_status_command,
            pause_color_signature_generation_command,
            resume_color_signature_generation_command,
            stop_color_signature_generation_command,
            rebuild_color_signature_cache_command,
            list_image_auto_tags_command,
            list_image_user_tags_command,
            add_image_user_custom_tag_command,
            remove_image_user_custom_tag_command,
            add_image_user_supplement_tag_command,
            remove_image_user_supplement_tag_command,
            list_tag_management_state_command,
            create_user_tag_folder_command,
            create_user_custom_tag_command,
            delete_user_custom_tag_command,
            rename_user_tag_folder_command,
            delete_user_tag_folder_command,
            assign_user_tag_to_folder_command,
            unassign_user_tag_from_folder_command,
            suggest_known_auto_tags_command,
            search_gallery_image_ids_command,
            search_gallery_image_ids_by_natural_language_command,
            search_gallery_image_ids_by_external_image_command,
            create_user_folder_command,
            rename_user_folder_command,
            delete_user_folder_command,
            reorder_user_folder_command,
            assign_image_to_user_folder_command,
            remove_image_from_user_folder_command,
            create_reference_board_folder_command,
            create_reference_board_command,
            add_image_to_reference_board_command,
            paste_image_to_reference_board_command,
            duplicate_reference_board_item_command,
            restore_reference_board_item_command,
            import_reference_board_item_to_library_command,
            export_reference_board_item_command,
            export_gallery_image_command,
            rename_reference_board_command,
            rename_reference_board_folder_command,
            reorder_reference_board_folder_command,
            move_reference_board_to_folder_command,
            reorder_reference_board_command,
            delete_reference_board_command,
            delete_reference_board_folder_command,
            remove_reference_board_item_command,
            update_reference_board_item_layout_command,
            bring_reference_board_item_to_front_command,
            auto_arrange_reference_board_command,
            window_minimize_command,
            window_toggle_maximize_command,
            window_is_maximized_command,
            window_start_dragging_command,
            window_close_command
        ])
        .run(tauri::generate_context!())
        .expect("failed to run illuTag");
}
