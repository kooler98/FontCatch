fn main() {
    let attributes =
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "get_settings",
            "save_settings",
            "get_snapshot",
            "install_font",
            "ignore_font",
            "read_font_preview",
            "export_woff2",
            "reveal_font",
            "open_font_license",
            "plan_uninstall",
            "uninstall_family",
            "open_link",
            "hide_panel",
            "set_dialog_active",
        ]));
    let attributes = if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        // tauri-build links its manifest only to application binaries. The lib
        // test harness also imports Common-Controls v6 (e.g. TaskDialogIndirect),
        // so it needs the same activation context before the Windows loader runs.
        // https://github.com/tauri-apps/tauri/issues/13419
        let manifest = std::path::PathBuf::from(
            std::env::var_os("CARGO_MANIFEST_DIR").expect("Missing Cargo manifest directory"),
        )
        .join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        // Avoid embedding resource #1 twice in the application executable.
        attributes.windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest())
    } else {
        attributes
    };
    tauri_build::try_build(attributes).expect("Could not build Fontcatch permissions");
}
