#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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
    get_user_folder_rule, save_user_folder_rule, UserFolderRule, UserFolderRuleCondition,
    duplicate_reference_board_item, export_gallery_image_from_state, export_reference_board_item_from_state,
    import_reference_board_item_to_library, list_library_from_state,
    list_image_auto_tags, list_image_user_tags, add_image_user_custom_tag, remove_image_user_custom_tag,
    add_image_user_supplement_tag, remove_image_user_supplement_tag,
    list_tag_management_state, create_user_tag_folder, rename_user_tag_folder, delete_user_tag_folder,
    assign_user_tag_to_folder, unassign_user_tag_from_folder, create_user_custom_tag, delete_user_custom_tag,
    dev_cleanup_stress_test_data, dev_create_fake_gallery_data, dev_create_small_file_test_set,
    export_data_migration_backup, export_organized_folder_result, inspect_data_migration_backup, import_data_migration_backup,
    list_gallery_image_ids_for_scope, list_gallery_images_by_ids_page, list_gallery_images_page, search_gallery_image_ids, start_startup_cleanup, startup_cleanup_status, suggest_known_auto_tags,
    move_reference_board_to_folder, paste_image_to_reference_board, paste_images_to_reference_board,
    read_image_bytes,
    remove_gallery_folder, remove_image_from_index, remove_image_from_user_folder, remove_reference_board_item,
    restore_image_from_trash,
    remove_images_from_index, restore_images_from_trash, set_images_favorite,
    assign_images_to_user_folder, move_images_to_user_folder, remove_images_from_user_folder, add_images_user_tags,
    move_image_to_system_trash,
    move_images_to_system_trash,
    toggle_image_favorite,
    restore_reference_board_item,
    rename_reference_board, rename_reference_board_folder, reorder_reference_board,
    reorder_reference_board_folder, reorder_user_folder, rename_user_folder, update_reference_board_item_layout,
    bring_reference_board_item_to_front, start_scan_all_folders_collect_only, start_scan_all_folders_with_tagging, start_tag_pending_images_only, test_wd_swinv2_tagger,
    AppState, AtmosphereGenerationProgress, BackupPathMapping, BackgroundScanProgress, BackgroundScanStatus, BatchSupplementTagInput, BatchSystemTrashResult, ColorSignatureGenerationProgress, DataMigrationBackupInspection, DevFakeGalleryOptions, DevSmallFileSetOptions, DevStressToolResult, GalleryImagePage, GallerySearchFilters, ImageAutoTagSummary, ImageBytes, ImageUserTagSummary, KnownAutoTagSuggestion, LibraryStore, NaturalLanguageScanProgress, NaturalLanguageScanStatus, OrganizedExportManifest, ReferenceBoardPasteImageInput, StartupCleanupStatus, TagManagementState, ThumbnailGenerationProgress,
    WdTaggerTestResult,
};
use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use serde::{Deserialize, Serialize};
use rusqlite::{params, Connection};
use tauri::{Manager, PhysicalPosition, PhysicalSize, Position, Size, State, WebviewWindow, WindowEvent};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DataDirectoryInfo {
    data_dir: String,
    using_fallback: bool,
    fallback_message: Option<String>,
    legacy_data_dir: Option<String>,
    legacy_database_detected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PortableConfig {
    data_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DataDirectoryMigrationResult {
    source_dir: String,
    target_dir: String,
    config_path: String,
    copied_files: u64,
    copied_bytes: u64,
    restart_required: bool,
    message: String,
}

#[tauri::command]
fn list_library(state: State<AppState>) -> Result<LibraryStore, String> {
    let started = Instant::now();
    eprintln!("[startup-prof] list_library_command begin");
    let result = list_library_from_state(&state);
    eprintln!(
        "[startup-prof] list_library_command total_ms={}",
        started.elapsed().as_millis()
    );
    result
}

#[tauri::command]
fn data_directory_info_command(state: State<AppState>) -> Result<DataDirectoryInfo, String> {
    let legacy_data_dir = state.legacy_data_dir.as_ref().map(|path| path.to_string_lossy().to_string());
    let legacy_database_detected = state
        .legacy_data_dir
        .as_ref()
        .map(|path| path.join("illutag.sqlite").is_file())
        .unwrap_or(false);
    Ok(DataDirectoryInfo {
        data_dir: state.data_dir.to_string_lossy().to_string(),
        using_fallback: state.data_dir_fallback_message.is_some(),
        fallback_message: state.data_dir_fallback_message.clone(),
        legacy_data_dir,
        legacy_database_detected,
    })
}

#[tauri::command]
fn list_gallery_images_page_command(
    scope: String,
    folder_id: Option<i64>,
    unclassified_only_parent_folder_id: Option<i64>,
    offset: i64,
    limit: i64,
    state: State<AppState>,
) -> Result<GalleryImagePage, String> {
    list_gallery_images_page(
        scope,
        folder_id,
        unclassified_only_parent_folder_id,
        offset,
        limit,
        &state,
    )
}

#[tauri::command]
fn list_gallery_image_ids_for_scope_command(
    scope: String,
    folder_id: Option<i64>,
    unclassified_only_parent_folder_id: Option<i64>,
    state: State<AppState>,
) -> Result<Vec<String>, String> {
    list_gallery_image_ids_for_scope(
        scope,
        folder_id,
        unclassified_only_parent_folder_id,
        &state,
    )
}

#[tauri::command]
fn list_gallery_images_by_ids_page_command(
    image_ids: Vec<String>,
    offset: i64,
    limit: i64,
    state: State<AppState>,
) -> Result<GalleryImagePage, String> {
    list_gallery_images_by_ids_page(image_ids, offset, limit, &state)
}

#[tauri::command]
fn dev_create_fake_gallery_data_command(
    options: DevFakeGalleryOptions,
    state: State<AppState>,
) -> Result<DevStressToolResult, String> {
    dev_create_fake_gallery_data(options, &state)
}

#[tauri::command]
fn dev_create_small_file_test_set_command(
    options: DevSmallFileSetOptions,
    state: State<AppState>,
) -> Result<DevStressToolResult, String> {
    dev_create_small_file_test_set(options, &state)
}

#[tauri::command]
fn dev_cleanup_stress_test_data_command(
    state: State<AppState>,
) -> Result<DevStressToolResult, String> {
    dev_cleanup_stress_test_data(&state)
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
fn move_image_to_system_trash_command(
    image_id: String,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    move_image_to_system_trash(image_id, &state)
}

#[tauri::command]
fn set_images_favorite_command(
    image_ids: Vec<String>,
    favorite: bool,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    set_images_favorite(image_ids, favorite, &state)
}

#[tauri::command]
fn remove_images_from_index_command(
    image_ids: Vec<String>,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    remove_images_from_index(image_ids, &state)
}

#[tauri::command]
fn restore_images_from_trash_command(
    image_ids: Vec<String>,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    restore_images_from_trash(image_ids, &state)
}

#[tauri::command]
fn move_images_to_system_trash_command(
    image_ids: Vec<String>,
    state: State<AppState>,
) -> Result<BatchSystemTrashResult, String> {
    move_images_to_system_trash(image_ids, &state)
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
fn start_scan_all_folders_collect_only_command(state: State<AppState>) -> Result<bool, String> {
    start_scan_all_folders_collect_only(&state)
}

#[tauri::command]
fn start_tag_pending_images_only_command(state: State<AppState>) -> Result<bool, String> {
    start_tag_pending_images_only(&state)
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
fn export_data_migration_backup_command(
    destination_path: String,
    frontend_settings: Option<serde_json::Value>,
    state: State<AppState>,
) -> Result<(), String> {
    export_data_migration_backup(destination_path, frontend_settings, &state)
}

#[tauri::command]
fn inspect_data_migration_backup_command(
    backup_path: String,
) -> Result<DataMigrationBackupInspection, String> {
    inspect_data_migration_backup(backup_path)
}

#[tauri::command]
fn import_data_migration_backup_command(
    backup_path: String,
    path_mappings: Vec<BackupPathMapping>,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    import_data_migration_backup(backup_path, path_mappings, &state)
}

#[tauri::command]
fn export_organized_folder_result_command(
    destination_dir: String,
    state: State<AppState>,
) -> Result<OrganizedExportManifest, String> {
    export_organized_folder_result(destination_dir, &state)
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
fn get_user_folder_rule_command(
    folder_id: i64,
    state: State<AppState>,
) -> Result<Option<UserFolderRule>, String> {
    get_user_folder_rule(folder_id, &state)
}

#[tauri::command]
fn save_user_folder_rule_command(
    folder_id: i64,
    conditions: Vec<UserFolderRuleCondition>,
    apply_now: bool,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    save_user_folder_rule(folder_id, conditions, apply_now, &state)
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
fn assign_images_to_user_folder_command(
    image_ids: Vec<String>,
    folder_id: i64,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    assign_images_to_user_folder(image_ids, folder_id, &state)
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
fn move_images_to_user_folder_command(
    image_ids: Vec<String>,
    from_folder_id: i64,
    target_folder_id: i64,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    move_images_to_user_folder(image_ids, from_folder_id, target_folder_id, &state)
}

#[tauri::command]
fn remove_images_from_user_folder_command(
    image_ids: Vec<String>,
    folder_id: i64,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    remove_images_from_user_folder(image_ids, folder_id, &state)
}

#[tauri::command]
fn add_images_user_tags_command(
    image_ids: Vec<String>,
    custom_tags: Vec<String>,
    supplement_tags: Vec<BatchSupplementTagInput>,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    add_images_user_tags(image_ids, custom_tags, supplement_tags, &state)
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
fn paste_images_to_reference_board_command(
    board_id: i64,
    images: Vec<ReferenceBoardPasteImageInput>,
    x: f32,
    y: f32,
    stack_offset: f32,
    state: State<AppState>,
) -> Result<LibraryStore, String> {
    paste_images_to_reference_board(board_id, images, x, y, stack_offset, &state)
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
fn window_toggle_always_on_top_command(window: tauri::Window) -> Result<bool, String> {
    let always_on_top = window
        .is_always_on_top()
        .map_err(|error| format!("读取窗口置顶状态失败：{error}"))?;
    let next = !always_on_top;
    window
        .set_always_on_top(next)
        .map_err(|error| format!("更新窗口置顶状态失败：{error}"))?;
    window
        .is_always_on_top()
        .map_err(|error| format!("验证窗口置顶状态失败：{error}"))
}

#[tauri::command]
fn window_is_always_on_top_command(window: tauri::Window) -> Result<bool, String> {
    window
        .is_always_on_top()
        .map_err(|error| format!("读取窗口置顶状态失败：{error}"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WindowMemoryState {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    is_maximized: bool,
    is_fullscreen: bool,
}

const WINDOW_MEMORY_FILE_NAME: &str = "window-state.json";
const PORTABLE_CONFIG_FILE_NAME: &str = "illutag.portable.json";
const WINDOW_STATE_SAVE_DEBOUNCE_MS: u64 = 350;

fn capture_window_memory_state(window: &WebviewWindow) -> Result<WindowMemoryState, String> {
    let position = window
        .outer_position()
        .map_err(|error| format!("failed to read window position: {error}"))?;
    let size = window
        .outer_size()
        .map_err(|error| format!("failed to read window size: {error}"))?;
    let is_maximized = window
        .is_maximized()
        .map_err(|error| format!("failed to read window maximized state: {error}"))?;
    let is_fullscreen = window
        .is_fullscreen()
        .map_err(|error| format!("failed to read window fullscreen state: {error}"))?;

    Ok(WindowMemoryState {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
        is_maximized,
        is_fullscreen,
    })
}

fn window_state_intersects_available_monitor(window: &WebviewWindow, state: &WindowMemoryState) -> Result<bool, String> {
    let monitors = window
        .available_monitors()
        .map_err(|error| format!("failed to read available monitors: {error}"))?;
    if monitors.is_empty() {
        return Ok(true);
    }

    let window_left = state.x as i64;
    let window_top = state.y as i64;
    let window_right = window_left + state.width as i64;
    let window_bottom = window_top + state.height as i64;

    Ok(monitors.iter().any(|monitor| {
        let monitor_position = monitor.position();
        let monitor_size = monitor.size();
        let monitor_left = monitor_position.x as i64;
        let monitor_top = monitor_position.y as i64;
        let monitor_right = monitor_left + monitor_size.width as i64;
        let monitor_bottom = monitor_top + monitor_size.height as i64;
        let intersection_width = window_right.min(monitor_right) - window_left.max(monitor_left);
        let intersection_height = window_bottom.min(monitor_bottom) - window_top.max(monitor_top);
        intersection_width > 80 && intersection_height > 80
    }))
}

fn move_window_state_to_visible_monitor(window: &WebviewWindow, state: &mut WindowMemoryState) -> Result<(), String> {
    if window_state_intersects_available_monitor(window, state)? {
        return Ok(());
    }

    let monitors = window
        .available_monitors()
        .map_err(|error| format!("failed to read available monitors: {error}"))?;
    let Some(monitor) = monitors.first() else {
        return Ok(());
    };

    let monitor_position = monitor.position();
    let monitor_size = monitor.size();
    let width = state.width.min(monitor_size.width).max(500);
    let height = state.height.min(monitor_size.height).max(500);

    state.width = width;
    state.height = height;
    state.x = monitor_position.x + ((monitor_size.width.saturating_sub(width) / 2) as i32);
    state.y = monitor_position.y + ((monitor_size.height.saturating_sub(height) / 2) as i32);
    Ok(())
}

fn restore_window_memory_state(window: &WebviewWindow, mut state: WindowMemoryState) -> Result<(), String> {
    state.width = state.width.max(500);
    state.height = state.height.max(500);
    move_window_state_to_visible_monitor(window, &mut state)?;

    if window
        .is_fullscreen()
        .map_err(|error| format!("failed to read window fullscreen state: {error}"))?
    {
        window
            .set_fullscreen(false)
            .map_err(|error| format!("failed to leave fullscreen before restore: {error}"))?;
    }

    if window
        .is_maximized()
        .map_err(|error| format!("failed to read window maximized state: {error}"))?
    {
        window
            .unmaximize()
            .map_err(|error| format!("failed to unmaximize before restore: {error}"))?;
    }

    window
        .set_size(Size::Physical(PhysicalSize {
            width: state.width,
            height: state.height,
        }))
        .map_err(|error| format!("failed to restore window size: {error}"))?;
    window
        .set_position(Position::Physical(PhysicalPosition {
            x: state.x,
            y: state.y,
        }))
        .map_err(|error| format!("failed to restore window position: {error}"))?;

    if state.is_maximized {
        window
            .maximize()
            .map_err(|error| format!("failed to restore window maximized state: {error}"))?;
    }
    if state.is_fullscreen {
        window
            .set_fullscreen(true)
            .map_err(|error| format!("failed to restore window fullscreen state: {error}"))?;
    }

    Ok(())
}

fn read_window_memory_state(path: &Path) -> Result<Option<WindowMemoryState>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)
        .map_err(|error| format!("failed to read window memory state: {error}"))?;
    let state = serde_json::from_str::<WindowMemoryState>(&content)
        .map_err(|error| format!("failed to parse window memory state: {error}"))?;
    Ok(Some(state))
}

fn save_window_memory_state(path: &Path, window: &WebviewWindow) -> Result<(), String> {
    let state = capture_window_memory_state(window)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create window memory directory: {error}"))?;
    }
    let content = serde_json::to_string_pretty(&state)
        .map_err(|error| format!("failed to serialize window memory state: {error}"))?;
    fs::write(path, content)
        .map_err(|error| format!("failed to write window memory state: {error}"))
}

fn restore_window_memory_state_from_path(window: &WebviewWindow, path: &Path) -> Result<(), String> {
    if let Some(state) = read_window_memory_state(path)? {
        restore_window_memory_state(window, state)?;
    }
    Ok(())
}

fn install_window_memory_state_listener(window: &WebviewWindow, path: PathBuf) {
    let save_generation = Arc::new(AtomicU64::new(0));
    let window_for_listener = window.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::Moved(_) | WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
            let generation = save_generation.fetch_add(1, Ordering::SeqCst) + 1;
            let save_generation = Arc::clone(&save_generation);
            let window = window_for_listener.clone();
            let path = path.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(WINDOW_STATE_SAVE_DEBOUNCE_MS));
                if save_generation.load(Ordering::SeqCst) != generation {
                    return;
                }
                if let Err(error) = save_window_memory_state(&path, &window) {
                    eprintln!("[window-memory] save failed: {error}");
                }
            });
        }
        WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed => {
            if let Err(error) = save_window_memory_state(&path, &window_for_listener) {
                eprintln!("[window-memory] save failed: {error}");
            }
        }
        _ => {}
    });
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

#[tauri::command]
fn open_gallery_image_with_default_app_command(
    image_id: String,
    state: State<AppState>,
) -> Result<(), String> {
    let library = list_library_from_state(&state)?;
    let image = library
        .images
        .iter()
        .find(|entry| entry.id == image_id)
        .ok_or_else(|| "Image not found".to_string())?;
    open_path_with_default_app(&image.path)
}

#[tauri::command]
fn open_external_url_command(url: String) -> Result<(), String> {
    let trimmed = url.trim();
    let allowed = trimmed.starts_with("https://github.com/YazawaSunrise/illuTag")
        || trimmed.starts_with("http://github.com/YazawaSunrise/illuTag");
    if !allowed {
        return Err("Unsupported external URL".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(trimmed)
            .spawn()
            .map_err(|error| format!("Failed to open URL: {error}"))?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        let status = Command::new("open")
            .arg(trimmed)
            .status()
            .map_err(|error| format!("Failed to open URL: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("Failed to open URL. status={status}"))
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let status = Command::new("xdg-open")
            .arg(trimmed)
            .status()
            .map_err(|error| format!("Failed to open URL: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("Failed to open URL. status={status}"))
        }
    }
}

#[tauri::command]
fn open_data_directory_command(state: State<AppState>) -> Result<(), String> {
    open_path_with_default_app(&state.data_dir.to_string_lossy())
}

fn open_path_with_default_app(path: &str) -> Result<(), String> {
    let normalized_path = path.strip_prefix(r"\\?\").unwrap_or(path);

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(normalized_path)
            .spawn()
            .map_err(|error| format!("Failed to open image with default app: {error}"))?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        let status = Command::new("open")
            .arg(normalized_path)
            .status()
            .map_err(|error| format!("Failed to open image with default app: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!(
                "Failed to open image with default app. status={status}"
            ))
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let status = Command::new("xdg-open")
            .arg(normalized_path)
            .status()
            .map_err(|error| format!("Failed to open image with default app: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!(
                "Failed to open image with default app. status={status}"
            ))
        }
    }
}

fn exe_dir() -> Result<PathBuf, String> {
    let exe_path = env::current_exe().map_err(|error| format!("Failed to resolve exe path: {error}"))?;
    exe_path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "Failed to resolve exe directory".to_string())
}

fn portable_config_path() -> Result<PathBuf, String> {
    Ok(exe_dir()?.join(PORTABLE_CONFIG_FILE_NAME))
}

fn read_portable_config(path: &Path) -> Result<PortableConfig, String> {
    if !path.is_file() {
        return Ok(PortableConfig::default());
    }
    let text = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read portable config {}: {error}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("Failed to parse portable config {}: {error}", path.display()))
}

fn write_portable_config(path: &Path, config: &PortableConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create portable config directory: {error}"))?;
    }
    let text = serde_json::to_string_pretty(config)
        .map_err(|error| format!("Failed to serialize portable config: {error}"))?;
    fs::write(path, text).map_err(|error| format!("Failed to write portable config: {error}"))
}

fn request_stop_background_tasks(state: &AppState) {
    if let Ok(mut value) = state.background_scan_stop_requested.lock() {
        *value = true;
    }
    if let Ok(mut value) = state.background_scan_pause_requested.lock() {
        *value = false;
    }
    if let Ok(mut value) = state.thumbnail_generation_stop_requested.lock() {
        *value = true;
    }
    if let Ok(mut value) = state.thumbnail_generation_pause_requested.lock() {
        *value = false;
    }
    if let Ok(mut value) = state.natural_language_scan_stop_requested.lock() {
        *value = true;
    }
    if let Ok(mut value) = state.natural_language_scan_pause_requested.lock() {
        *value = false;
    }
    if let Ok(mut value) = state.atmosphere_generation_stop_requested.lock() {
        *value = true;
    }
    if let Ok(mut value) = state.atmosphere_generation_pause_requested.lock() {
        *value = false;
    }
    if let Ok(mut value) = state.color_signature_generation_stop_requested.lock() {
        *value = true;
    }
    if let Ok(mut value) = state.color_signature_generation_pause_requested.lock() {
        *value = false;
    }
}

fn background_tasks_running(state: &AppState) -> bool {
    state.background_scan_running.lock().map(|value| *value).unwrap_or(false)
        || state.thumbnail_generation_running.lock().map(|value| *value).unwrap_or(false)
        || state.natural_language_scan_running.lock().map(|value| *value).unwrap_or(false)
        || state.atmosphere_generation_running.lock().map(|value| *value).unwrap_or(false)
        || state.color_signature_generation_running.lock().map(|value| *value).unwrap_or(false)
        || state.startup_cleanup_running.lock().map(|value| *value).unwrap_or(false)
}

fn stop_background_tasks_before_migration(state: &AppState) -> Result<(), String> {
    request_stop_background_tasks(state);
    let started = Instant::now();
    while background_tasks_running(state) {
        if started.elapsed() > Duration::from_secs(20) {
            return Err("Background tasks are still running. Please try migration again later.".to_string());
        }
        thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}
fn copy_directory_recursive(source: &Path, target: &Path) -> Result<(u64, u64), String> {
    fs::create_dir_all(target)
        .map_err(|error| format!("Failed to create target data directory {}: {error}", target.display()))?;
    let mut copied_files = 0u64;
    let mut copied_bytes = 0u64;
    for entry in fs::read_dir(source)
        .map_err(|error| format!("Failed to read data directory {}: {error}", source.display()))?
    {
        let entry = entry.map_err(|error| format!("Failed to read data directory entry: {error}"))?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let metadata = entry
            .metadata()
            .map_err(|error| format!("Failed to read metadata {}: {error}", source_path.display()))?;
        if metadata.is_dir() {
            let (nested_files, nested_bytes) = copy_directory_recursive(&source_path, &target_path)?;
            copied_files += nested_files;
            copied_bytes += nested_bytes;
        } else if metadata.is_file() {
            fs::copy(&source_path, &target_path).map_err(|error| {
                format!(
                    "Failed to copy {} to {}: {error}",
                    source_path.display(),
                    target_path.display()
                )
            })?;
            copied_files += 1;
            copied_bytes += metadata.len();
        }
    }
    Ok((copied_files, copied_bytes))
}

fn validate_migrated_data_directory(source: &Path, target: &Path) -> Result<(), String> {
    for file_name in ["illutag.sqlite", "illutag.test.sqlite"] {
        let source_file = source.join(file_name);
        if !source_file.is_file() {
            continue;
        }
        let target_file = target.join(file_name);
        if !target_file.is_file() {
            return Err(format!("迁移校验失败：目标目录缺少 {file_name}"));
        }
        let source_size = source_file
            .metadata()
            .map_err(|error| format!("Failed to inspect source database: {error}"))?
            .len();
        let target_size = target_file
            .metadata()
            .map_err(|error| format!("Failed to inspect target database: {error}"))?
            .len();
        if source_size != target_size {
            return Err(format!("迁移校验失败：{file_name} 文件大小不一致"));
        }
    }
    Ok(())
}

fn strip_windows_long_path_prefix(path: &str) -> &str {
    path.strip_prefix(r"\\?\").unwrap_or(path)
}

fn path_prefix_variants(paths: &[PathBuf]) -> Vec<String> {
    let mut seen = HashSet::<String>::new();
    let mut variants = Vec::<String>::new();
    for path in paths {
        let value = path.to_string_lossy().to_string();
        for candidate in [
            value.clone(),
            strip_windows_long_path_prefix(&value).to_string(),
            value.replace('\\', "/"),
            strip_windows_long_path_prefix(&value).replace('\\', "/"),
        ] {
            if candidate.is_empty() {
                continue;
            }
            let key = candidate.to_ascii_lowercase();
            if seen.insert(key) {
                variants.push(candidate);
            }
        }
    }
    variants.sort_by_key(|value| std::cmp::Reverse(value.len()));
    variants
}

fn target_prefix_for_source_variant(source_variant: &str, target_dir: &Path) -> String {
    let target = target_dir.to_string_lossy().to_string();
    if source_variant.contains('/') && !source_variant.contains('\\') {
        strip_windows_long_path_prefix(&target).replace('\\', "/")
    } else if source_variant.starts_with(r"\\?\") {
        target
    } else {
        strip_windows_long_path_prefix(&target).to_string()
    }
}

fn replace_path_prefix(value: &str, source_prefixes: &[String], target_dir: &Path) -> Option<String> {
    for source_prefix in source_prefixes {
        let Some(head) = value.get(..source_prefix.len()) else {
            continue;
        };
        let Some(tail) = value.get(source_prefix.len()..) else {
            continue;
        };
        if head.eq_ignore_ascii_case(source_prefix) {
            let target_prefix = target_prefix_for_source_variant(source_prefix, target_dir);
            return Some(format!("{target_prefix}{tail}"));
        }
    }
    None
}

fn rewrite_path_prefix_in_table(
    conn: &Connection,
    table_name: &str,
    column_name: &str,
    source_prefixes: &[String],
    target_dir: &Path,
) -> Result<(), String> {
    let select_sql = format!("SELECT rowid, {column_name} FROM {table_name}");
    let mut stmt = conn
        .prepare(&select_sql)
        .map_err(|error| format!("Failed to prepare {table_name}.{column_name} rewrite query: {error}"))?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))
        .map_err(|error| format!("Failed to query {table_name}.{column_name}: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Failed to collect {table_name}.{column_name}: {error}"))?;
    drop(stmt);

    let update_sql = format!("UPDATE {table_name} SET {column_name} = ?1 WHERE rowid = ?2");
    for (rowid, current_path) in rows {
        let Some(next_path) = replace_path_prefix(&current_path, source_prefixes, target_dir) else {
            continue;
        };
        if next_path == current_path {
            continue;
        }
        conn.execute(&update_sql, params![next_path, rowid])
            .map_err(|error| format!("Failed to rewrite {table_name}.{column_name}: {error}"))?;
    }
    Ok(())
}

fn rewrite_image_id_references(
    conn: &Connection,
    old_image_id: &str,
    new_image_id: &str,
) -> Result<(), String> {
    for table_name in [
        "reference_board_items",
        "image_thumbnails",
        "image_clip_embeddings",
        "image_atmosphere_signatures",
        "image_color_signatures",
        "image_user_folders",
        "image_auto_tags",
        "image_user_custom_tags",
        "image_user_supplement_tags",
        "image_tags",
        "image_ai_state",
    ] {
        let sql = format!("UPDATE {table_name} SET image_id = ?1 WHERE image_id = ?2");
        conn.execute(&sql, params![new_image_id, old_image_id])
            .map_err(|error| format!("Failed to rewrite {table_name}.image_id: {error}"))?;
    }
    Ok(())
}

fn rewrite_migrated_database_paths(
    source_dirs: &[PathBuf],
    target_dir: &Path,
) -> Result<(), String> {
    let source_prefixes = path_prefix_variants(source_dirs);
    for database_name in ["illutag.sqlite", "illutag.test.sqlite"] {
        let database_path = target_dir.join(database_name);
        if !database_path.is_file() {
            continue;
        }

        let mut conn = Connection::open(&database_path)
            .map_err(|error| format!("Failed to open migrated database {}: {error}", database_path.display()))?;
        conn.execute("PRAGMA foreign_keys = OFF", [])
            .map_err(|error| format!("Failed to disable foreign keys for migration rewrite: {error}"))?;
        let tx = conn
            .transaction()
            .map_err(|error| format!("Failed to open migration rewrite transaction: {error}"))?;

        rewrite_path_prefix_in_table(&tx, "image_thumbnails", "thumb_path", &source_prefixes, target_dir)?;
        rewrite_path_prefix_in_table(&tx, "folders", "path", &source_prefixes, target_dir)?;

        let mut stmt = tx
            .prepare(
                "
                SELECT id, path
                FROM images
                WHERE COALESCE(source, '') = 'reference'
                ",
            )
            .map_err(|error| format!("Failed to prepare reference image rewrite query: {error}"))?;
        let reference_images = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| format!("Failed to query reference images for migration rewrite: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Failed to collect reference images for migration rewrite: {error}"))?;
        drop(stmt);

        for (old_id, old_path) in reference_images {
            let new_id = replace_path_prefix(&old_id, &source_prefixes, target_dir)
                .unwrap_or_else(|| old_id.clone());
            let new_path = replace_path_prefix(&old_path, &source_prefixes, target_dir)
                .unwrap_or_else(|| old_path.clone());
            if new_id == old_id && new_path == old_path {
                continue;
            }
            if new_id != old_id {
                rewrite_image_id_references(&tx, &old_id, &new_id)?;
            }
            tx.execute(
                "UPDATE images SET id = ?1, path = ?2 WHERE id = ?3",
                params![new_id, new_path, old_id],
            )
            .map_err(|error| format!("Failed to rewrite reference image path: {error}"))?;
        }

        tx.commit()
            .map_err(|error| format!("Failed to commit migration path rewrite: {error}"))?;
    }
    Ok(())
}

#[tauri::command]
fn migrate_data_directory_command(
    target_dir: String,
    state: State<AppState>,
) -> Result<DataDirectoryMigrationResult, String> {
    let trimmed = target_dir.trim();
    if trimmed.is_empty() {
        return Err("请选择目标数据目录".to_string());
    }

    let raw_source_dir = state.data_dir.clone();
    let source_dir = raw_source_dir
        .canonicalize()
        .unwrap_or_else(|_| state.data_dir.clone());
    let target_dir = PathBuf::from(trimmed);
    fs::create_dir_all(&target_dir)
        .map_err(|error| format!("Failed to create target data directory {}: {error}", target_dir.display()))?;
    let target_dir = target_dir
        .canonicalize()
        .map_err(|error| format!("Failed to read target data directory {}: {error}", target_dir.display()))?;

    if source_dir == target_dir {
        return Err("目标目录与当前数据目录相同".to_string());
    }
    if target_dir.starts_with(&source_dir) {
        return Err("目标目录不能位于当前数据目录内部".to_string());
    }

    let config_path = portable_config_path()?;
    stop_background_tasks_before_migration(&state)?;
    let (copied_files, copied_bytes) = copy_directory_recursive(&source_dir, &target_dir)?;
    validate_migrated_data_directory(&source_dir, &target_dir)?;
    rewrite_migrated_database_paths(&[raw_source_dir, source_dir.clone()], &target_dir)?;
    write_portable_config(
        &config_path,
        &PortableConfig {
            data_dir: Some(target_dir.to_string_lossy().to_string()),
        },
    )?;

    Ok(DataDirectoryMigrationResult {
        source_dir: source_dir.to_string_lossy().to_string(),
        target_dir: target_dir.to_string_lossy().to_string(),
        config_path: config_path.to_string_lossy().to_string(),
        copied_files,
        copied_bytes,
        restart_required: true,
        message: "数据目录迁移完成。旧目录不会自动删除，请重启 illuTag，确认新目录正常后再按需手动清理旧目录。".to_string(),
    })
}
fn resolve_portable_data_dir(system_app_data_dir: PathBuf) -> (PathBuf, Option<String>) {
    if let Ok(config_path) = portable_config_path() {
        match read_portable_config(&config_path) {
            Ok(config) => {
                if let Some(configured_data_dir) = config
                    .data_dir
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    let configured_data_dir = PathBuf::from(configured_data_dir);
                    match fs::create_dir_all(&configured_data_dir) {
                        Ok(()) => return (configured_data_dir, None),
                        Err(error) => {
                            let message = format!(
                                "配置的数据目录 {} 无法创建：{error}。已回退到默认数据目录。",
                                configured_data_dir.display()
                            );
                            eprintln!("[data-dir] {message}");
                        }
                    }
                }
            }
            Err(error) => eprintln!("[data-dir] {error}"),
        }
    }

    let exe_data_dir_result = exe_dir().map(|dir| dir.join("data"));

    match exe_data_dir_result {
        Ok(data_dir) => match fs::create_dir_all(&data_dir) {
            Ok(()) => {
                let portable_database_exists = data_dir.join("illutag.sqlite").is_file();
                let legacy_database_exists = system_app_data_dir.join("illutag.sqlite").is_file();
                if !portable_database_exists && legacy_database_exists {
                    let message = format!(
                        "检测到旧版 AppData 数据库，当前暂时使用旧数据目录 {}。可在设置中迁移数据目录。",
                        system_app_data_dir.display()
                    );
                    eprintln!("[data-dir] {message}");
                    return (system_app_data_dir, Some(message));
                }
                (data_dir, None)
            }
            Err(error) => {
                let message = format!(
                    "无法创建 exe 同级 data 目录 {}：{error}。已改用系统数据目录 {}",
                    data_dir.display(),
                    system_app_data_dir.display()
                );
                eprintln!("[data-dir] {message}");
                if let Err(fallback_error) = fs::create_dir_all(&system_app_data_dir) {
                    eprintln!(
                        "[data-dir] failed to create fallback data directory {}: {fallback_error}",
                        system_app_data_dir.display()
                    );
                }
                (system_app_data_dir, Some(message))
            }
        },
        Err(error) => {
            let message = format!(
                "无法定位 exe 数据目录：{error}。已改用系统数据目录 {}",
                system_app_data_dir.display()
            );
            eprintln!("[data-dir] {message}");
            if let Err(fallback_error) = fs::create_dir_all(&system_app_data_dir) {
                eprintln!(
                    "[data-dir] failed to create fallback data directory {}: {fallback_error}",
                    system_app_data_dir.display()
                );
            }
            (system_app_data_dir, Some(message))
        }
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| Box::<dyn std::error::Error>::from(error))?;
            let legacy_data_dir = app_data_dir.clone();
            let (data_dir, data_dir_fallback_message) = resolve_portable_data_dir(app_data_dir);
            let window_memory_state_path = data_dir.join(WINDOW_MEMORY_FILE_NAME);

            if let Some(window) = app.get_webview_window("main") {
                if let Err(error) =
                    restore_window_memory_state_from_path(&window, &window_memory_state_path)
                {
                    eprintln!("[window-memory] restore failed: {error}");
                }
                install_window_memory_state_listener(&window, window_memory_state_path);
                window
                    .show()
                    .map_err(|error| Box::<dyn std::error::Error>::from(error))?;
            }

            let database_file_name = if env::var("ILLUTAG_USE_TEST_DB")
                .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
                .unwrap_or(false)
            {
                "illutag.test.sqlite"
            } else {
                "illutag.sqlite"
            };
            let database_path = data_dir.join(database_file_name);
            eprintln!("[database] using {}", database_path.display());
            let app_state = AppState {
                database_path,
                data_dir,
                data_dir_fallback_message,
                legacy_data_dir: Some(legacy_data_dir),
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
            data_directory_info_command,
            migrate_data_directory_command,
            list_gallery_images_page_command,
            list_gallery_image_ids_for_scope_command,
            list_gallery_images_by_ids_page_command,
            dev_create_fake_gallery_data_command,
            dev_create_small_file_test_set_command,
            dev_cleanup_stress_test_data_command,
            add_gallery_folder_command,
            remove_gallery_folder_command,
            remove_image_from_index_command,
            remove_images_from_index_command,
            restore_image_from_trash_command,
            restore_images_from_trash_command,
            move_image_to_system_trash_command,
            move_images_to_system_trash_command,
            toggle_image_favorite_command,
            set_images_favorite_command,
            read_image_bytes_command,
            copy_image_to_system_clipboard_command,
            test_wd_swinv2_tagger_command,
            start_scan_all_folders_with_tagging_command,
            start_scan_all_folders_collect_only_command,
            start_tag_pending_images_only_command,
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
            export_data_migration_backup_command,
            inspect_data_migration_backup_command,
            import_data_migration_backup_command,
            export_organized_folder_result_command,
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
            get_user_folder_rule_command,
            save_user_folder_rule_command,
            assign_image_to_user_folder_command,
            assign_images_to_user_folder_command,
            remove_image_from_user_folder_command,
            move_images_to_user_folder_command,
            remove_images_from_user_folder_command,
            add_images_user_tags_command,
            create_reference_board_folder_command,
            create_reference_board_command,
            add_image_to_reference_board_command,
            paste_image_to_reference_board_command,
            paste_images_to_reference_board_command,
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
            window_toggle_always_on_top_command,
            window_is_always_on_top_command,
            window_start_dragging_command,
            open_gallery_image_with_default_app_command,
            open_data_directory_command,
            open_external_url_command,
            window_close_command
        ])
        .run(tauri::generate_context!())
        .expect("failed to run illuTag");
}
