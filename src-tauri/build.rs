fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_settings",
            "save_settings",
            "get_snapshot",
            "install_font",
            "ignore_font",
            "read_font_preview",
            "reveal_font",
            "open_font_license",
            "plan_uninstall",
            "uninstall_family",
            "open_link",
            "hide_panel",
            "set_dialog_active",
        ]),
    ))
    .expect("Could not build Fontcatch permissions");
}
