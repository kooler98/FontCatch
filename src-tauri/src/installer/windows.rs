use super::*;
use std::{collections::HashSet, io::ErrorKind, os::windows::ffi::OsStrExt, sync::Mutex};
use windows_sys::Win32::{
    Graphics::Gdi::{AddFontResourceW, RemoveFontResourceW},
    UI::WindowsAndMessaging::{SendNotifyMessageW, HWND_BROADCAST, WM_FONTCHANGE},
};
use winreg::{
    enums::{HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE},
    types::ToRegValue,
    RegKey,
};

const REGISTRY: &str = "Software\\Microsoft\\Windows NT\\CurrentVersion\\Fonts";
pub struct WindowsInstaller {
    root: PathBuf,
    loaded: Mutex<HashSet<PathBuf>>,
}
impl WindowsInstaller {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            root: dirs::data_local_dir()
                .ok_or("local_data_unavailable")?
                .join("Microsoft/Windows/Fonts"),
            loaded: Mutex::new(HashSet::new()),
        })
    }

    fn owned_path(&self, path: &Path) -> Result<Vec<u16>, String> {
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        let valid_name = filename
            .strip_prefix("Fontcatch-")
            .and_then(|name| name.split_once('.'))
            .is_some_and(|(hash, extension)| {
                hash.len() == 64
                    && hash.bytes().all(|c| c.is_ascii_hexdigit())
                    && matches!(extension, "ttf" | "otf" | "ttc")
            });
        if path.parent() != Some(self.root.as_path()) || !valid_name {
            return Err("installed_font_changed".into());
        }
        wide(path)
    }
}

fn wide(path: &Path) -> Result<Vec<u16>, String> {
    let mut encoded: Vec<_> = path.as_os_str().encode_wide().collect();
    if !path.is_absolute() || encoded.contains(&0) {
        return Err("installed_font_changed".into());
    }
    encoded.push(0);
    Ok(encoded)
}

fn legacy_key(face: &FontFace) -> String {
    format!("{} (TrueType)", face.postscript)
}

fn keys(path: &Path, faces: &[FontFace]) -> Result<Vec<String>, String> {
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("invalid_font")?;
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    for face in faces {
        if face.postscript.is_empty() || face.postscript.contains('\0') {
            return Err("invalid_font".into());
        }
        // Each owned file gets its own registration, so an update never replaces
        // another file's registry mapping before the old file is retired.
        let name = format!("{} [{}] (TrueType)", face.postscript, filename);
        if seen.insert(name.to_lowercase()) {
            names.push(name);
        }
    }
    if names.is_empty() {
        return Err("invalid_font".into());
    }
    Ok(names)
}

fn missing_values(registry: &RegKey, path: &Path, names: &[String]) -> Result<Vec<String>, String> {
    let expected = path.as_os_str().to_reg_value();
    let mut missing = Vec::new();
    for name in names {
        match registry.get_raw_value(name) {
            Ok(value) if value.vtype == expected.vtype && value.bytes == expected.bytes => {}
            Ok(_) => return Err("external_duplicate".into()),
            Err(error) if error.kind() == ErrorKind::NotFound => missing.push(name.clone()),
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(missing)
}

fn delete_owned_values(registry: &RegKey, path: &Path, names: &[String]) -> Result<(), String> {
    let expected = path.as_os_str().to_reg_value();
    for name in names {
        match registry.get_raw_value(name) {
            Ok(value) if value.vtype == expected.vtype && value.bytes == expected.bytes => {
                registry.delete_value(name).map_err(|e| e.to_string())?;
            }
            Ok(_) => {} // Another application owns this value; leave it intact.
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}

impl Platform for WindowsInstaller {
    fn root(&self) -> &Path {
        &self.root
    }
    fn register(&self, path: &Path, faces: &[FontFace]) -> Result<(), String> {
        let encoded = self.owned_path(path)?;
        let names = keys(path, faces)?;
        let (registry, _) = RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey_with_flags(REGISTRY, KEY_QUERY_VALUE | KEY_SET_VALUE)
            .map_err(|e| e.to_string())?;
        let missing = missing_values(&registry, path, &names)?;
        let mut loaded = self.loaded.lock().map_err(|_| "font_registration_failed")?;
        let added = !loaded.contains(path);
        // AddFontResourceW returns zero on failure and supplies no extended error.
        if added && unsafe { AddFontResourceW(encoded.as_ptr()) } == 0 {
            return Err("font_registration_failed".into());
        }
        let mut written = Vec::new();
        for name in missing {
            // OsStr preserves Windows UTF-16 paths, including non-UTF-8 names.
            if let Err(error) = registry.set_value(&name, &path.as_os_str()) {
                let _ = delete_owned_values(&registry, path, &written);
                if added {
                    unsafe { RemoveFontResourceW(encoded.as_ptr()) };
                }
                return Err(error.to_string());
            }
            written.push(name);
        }
        loaded.insert(path.to_path_buf());
        drop(loaded);
        // This also announces restoration after a failed attempt to recycle a font.
        self.refresh()
    }
    fn unregister(&self, path: &Path, faces: &[FontFace]) -> Result<(), String> {
        let encoded = self.owned_path(path)?;
        let mut names = keys(path, faces)?;
        // Clean pre-0.3.5 mappings only when they still point to this exact file.
        names.extend(faces.iter().map(legacy_key));
        match RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(REGISTRY, KEY_QUERY_VALUE | KEY_SET_VALUE)
        {
            Ok(registry) => delete_owned_values(&registry, path, &names)?,
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        self.loaded
            .lock()
            .map_err(|_| "font_registration_failed")?
            .remove(path);
        // Microsoft documents removing every resource reference before deleting
        // the file. Zero also covers a resource already absent during recovery.
        for _ in 0..256 {
            if unsafe { RemoveFontResourceW(encoded.as_ptr()) } == 0 {
                // Notify consumers before recycling, so they can release cached
                // references instead of learning about removal only afterward.
                return self.refresh();
            }
        }
        Err("font_unregistration_failed".into())
    }
    fn refresh(&self) -> Result<(), String> {
        // No pointers in WM_FONTCHANGE: asynchronous delivery avoids waiting once
        // per top-level window when another app is slow or not responding.
        unsafe { SendNotifyMessageW(HWND_BROADCAST, WM_FONTCHANGE, 0, 0) };
        Ok(())
    }
    fn trash_owned(&self, path: &Path, hash: &str) -> Result<(), String> {
        self.owned_path(path)?;
        retry_owned_trash(
            || verify_owned(self, path, hash),
            || {
                trash::delete(path).map_err(|error| {
                    // With FOF_NOERRORUI, Shell can report a transient sharing
                    // failure only through this generic aborted-operation result.
                    let retryable = match &error {
                        trash::Error::Unknown { description } => {
                            description == "Some operations were aborted"
                        }
                        trash::Error::Os { code, .. } => {
                            matches!(*code as u32, 0x80070020 | 0x80070021)
                        }
                        _ => false,
                    };
                    if retryable {
                        OwnedTrashError::Retry(error.to_string())
                    } else {
                        OwnedTrashError::Stop(error.to_string())
                    }
                })
            },
            std::thread::sleep,
        )
    }
    fn search_roots(&self) -> Vec<PathBuf> {
        let mut roots = vec![self.root.clone()];
        if let Some(w) = std::env::var_os("WINDIR") {
            roots.push(PathBuf::from(w).join("Fonts"));
        }
        roots
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    use windows_sys::Win32::Graphics::Gdi::{
        CreateCompatibleDC, CreateFontIndirectW, DeleteDC, DeleteObject, GdiFlush, GetTextFaceW,
        SelectObject, DEFAULT_CHARSET, LOGFONTW,
    };

    struct TestRegistry(String);
    impl TestRegistry {
        fn new() -> Self {
            Self(format!(
                "Software\\FontcatchTests\\{}",
                uuid::Uuid::new_v4()
            ))
        }
        fn open(&self) -> RegKey {
            RegKey::predef(HKEY_CURRENT_USER)
                .create_subkey(&self.0)
                .unwrap()
                .0
        }
    }
    impl Drop for TestRegistry {
        fn drop(&mut self) {
            let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(&self.0);
        }
    }

    fn fixture() -> FontFile {
        inspector::inspect(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/FontcatchQA-Regular.ttf"),
        )
        .unwrap()
    }

    #[test]
    fn registry_collision_never_overwrites_or_deletes_another_path() {
        let guard = TestRegistry::new();
        let registry = guard.open();
        let path = Path::new(r"C:\Fonts\Fontcatch-test.ttf");
        let names = keys(path, &fixture().faces).unwrap();
        registry
            .set_value(&names[0], &r"C:\Other\original.ttf")
            .unwrap();
        assert_eq!(
            missing_values(&registry, path, &names).unwrap_err(),
            "external_duplicate"
        );
        delete_owned_values(&registry, path, &names).unwrap();
        assert_eq!(
            registry.get_value::<String, _>(&names[0]).unwrap(),
            r"C:\Other\original.ttf"
        );
    }

    #[test]
    fn registry_ownership_round_trips_windows_paths_without_loss() {
        let guard = TestRegistry::new();
        let registry = guard.open();
        let mut encoded: Vec<_> = r"C:\Users\Typographie é\".encode_utf16().collect();
        encoded.push(0xd800); // A legal Windows filename need not be valid UTF-8.
        encoded.extend(r"\font.ttf".encode_utf16());
        let path = PathBuf::from(OsString::from_wide(&encoded));
        let names = vec!["test font".to_string()];
        registry.set_value(&names[0], &path.as_os_str()).unwrap();
        assert!(missing_values(&registry, &path, &names).unwrap().is_empty());
        assert_eq!(&wide(&path).unwrap()[..encoded.len()], encoded.as_slice());
        delete_owned_values(&registry, &path, &names).unwrap();
        assert_eq!(
            registry.get_raw_value(&names[0]).unwrap_err().kind(),
            ErrorKind::NotFound
        );
    }

    #[test]
    fn windows_paths_reject_embedded_nul_and_registration_names_include_file_identity() {
        assert!(wide(Path::new(r"relative\font.ttf")).is_err());
        assert!(wide(&PathBuf::from(OsString::from_wide(&[
            b'C' as u16,
            b':' as u16,
            b'\\' as u16,
            0,
            b'x' as u16,
        ])))
        .is_err());
        let f = fixture();
        assert_ne!(
            keys(Path::new(r"C:\Fonts\Fontcatch-one.ttf"), &f.faces).unwrap(),
            keys(Path::new(r"C:\Fonts\Fontcatch-two.ttf"), &f.faces).unwrap(),
        );
    }

    // A missing font falls back. Ask GDI which face was actually selected rather
    // than treating a successful file copy as proof that Windows loaded the font.
    fn selected_family(family: &str) -> String {
        let mut description = LOGFONTW {
            lfCharSet: DEFAULT_CHARSET,
            ..Default::default()
        };
        for (slot, character) in description
            .lfFaceName
            .iter_mut()
            .take(31)
            .zip(family.encode_utf16())
        {
            *slot = character;
        }
        unsafe {
            let dc = CreateCompatibleDC(std::ptr::null_mut());
            assert!(!dc.is_null());
            let font = CreateFontIndirectW(&description);
            if font.is_null() {
                DeleteDC(dc);
                panic!("GDI could not create the test font");
            }
            let previous = SelectObject(dc, font);
            assert!(!previous.is_null());
            let mut name = [0u16; 64];
            let result = GetTextFaceW(dc, name.len() as i32, name.as_mut_ptr());
            let restored = SelectObject(dc, previous);
            let font_deleted = DeleteObject(font);
            let dc_deleted = DeleteDC(dc);
            let flushed = GdiFlush();
            assert_eq!(restored, font, "GDI must release the selected test font");
            assert_ne!(font_deleted, 0, "GDI test font must be destroyed");
            assert_ne!(dc_deleted, 0, "GDI test context must be destroyed");
            assert_ne!(flushed, 0, "GDI cleanup must finish before font removal");
            assert!(result > 0);
            let length = name.iter().position(|c| *c == 0).unwrap_or(name.len());
            String::from_utf16(&name[..length]).unwrap()
        }
    }

    struct SmokeCleanup<'a> {
        platform: &'a WindowsInstaller,
        path: PathBuf,
        font: &'a FontFile,
    }
    impl Drop for SmokeCleanup<'_> {
        fn drop(&mut self) {
            if self.path.exists() {
                let _ = remove_owned(self.platform, &self.path, &self.font.hash, &self.font.faces);
            }
            let _ = self.platform.refresh();
        }
    }

    #[test]
    #[ignore = "Opt-in: installs the CC0 synthetic QA font for this Windows user, verifies GDI, then moves it to Recycle Bin."]
    fn windows_native_install_uninstall_smoke() {
        assert_eq!(std::env::var("FONTCATCH_WINDOWS_SMOKE").as_deref(), Ok("1"));
        let platform = WindowsInstaller::new().unwrap();
        let font = fixture();
        let path = target(&platform, &font);
        assert!(
            !path.exists(),
            "A test destination already exists; leave it untouched"
        );
        assert_ne!(
            selected_family(&font.faces[0].family),
            font.faces[0].family,
            "A QA font is already installed; leave it untouched"
        );
        let original_hash = hash_file(&font.path, MAX_FILE).unwrap();
        let _cleanup = SmokeCleanup {
            platform: &platform,
            path: path.clone(),
            font: &font,
        };
        assert_eq!(install(&platform, &font).unwrap(), path);
        platform.refresh().unwrap();
        let registry = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(REGISTRY)
            .unwrap();
        let names = keys(&path, &font.faces).unwrap();
        assert!(missing_values(&registry, &path, &names).unwrap().is_empty());
        assert_eq!(selected_family(&font.faces[0].family), font.faces[0].family);
        // Repeated recovery/registration must not accumulate resource references.
        platform.register(&path, &font.faces).unwrap();
        remove_owned(&platform, &path, &font.hash, &font.faces).unwrap();
        platform.refresh().unwrap();
        assert!(!path.exists());
        assert_eq!(
            missing_values(&registry, &path, &names).unwrap().len(),
            names.len()
        );
        assert_ne!(selected_family(&font.faces[0].family), font.faces[0].family);
        assert_eq!(hash_file(&font.path, MAX_FILE).unwrap(), original_hash);
        println!("Windows user-font registration, GDI selection, registry cleanup and Recycle Bin removal verified.");
    }
}
