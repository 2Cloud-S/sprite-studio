pub mod recovery;
mod asset_io;
mod vfx;

pub fn verify_project_file(path: &std::path::Path) -> std::result::Result<(), String> {
    let dir = path.parent().ok_or("Project file has no directory")?;
    studio::read_project(dir).map(|_| ())
}
mod environment;
mod studio;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(studio::StudioLock::default())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            vfx::vfx_command,
            environment::environment_command,
            environment::environment_library,
            studio::get_app_state,
            studio::set_workspace,
            studio::list_projects,
            studio::list_deleted_projects,
            studio::delete_project,
            studio::restore_project,
            studio::create_project,
            studio::rename_project,
            studio::open_project,
            studio::import_anchor,
            studio::read_asset_preview,
            studio::configure_snapper,
            studio::run_snap,
            studio::run_auto_fit,
            studio::apply_snap,
            studio::read_snap_preview,
            studio::configure_python,
            studio::run_cleanup,
            studio::apply_cleanup,
            studio::read_cleanup_preview,
            studio::create_animation,
            studio::rename_animation,
            studio::import_animation_frames,
            studio::read_animation_frame_preview,
            studio::propose_alignment,
            studio::apply_alignment,
            studio::propose_animation_preview,
            studio::apply_animation_preview,
            studio::run_export,
            studio::apply_export,
            studio::read_export_asset,
            studio::read_export_manifest,
            studio::import_pose_board,
            studio::read_pose_board_preview,
            studio::run_pose_extraction,
            studio::read_extraction_candidate,
            studio::apply_pose_extraction,
            studio::read_raw_animation_frame,
            studio::analyze_batch_scale,
            studio::run_batch_snap,
            studio::run_batch_auto_fit,
            studio::apply_batch_snap,
            studio::run_batch_cleanup,
            studio::apply_batch_cleanup,
            studio::approve_batch_clean,
            studio::read_batch_frame_preview,
            studio::update_workflow,
            studio::update_animation_cleanup_settings,
            studio::update_runtime,
            studio::update_runtime_policy,
            studio::confirm_native_review,
            studio::replace_raw_frame,
            studio::apply_frame_recovery,
            studio::read_frame_recovery_preview,
            studio::read_frame_source_preview,
            studio::read_frame_recovery_source_preview,
            studio::import_upscaled_frame,
            studio::approve_upscale,
            studio::export_snapped,
            studio::open_snap_folder,
            studio::open_workspace_folder,
            studio::open_project_exports_folder,
            studio::export_reference,
            studio::reference_export_folder,
            studio::open_reference_export_folder,
            studio::open_animation_export_folder,
            studio::read_upscaled_preview,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
