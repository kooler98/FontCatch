use super::*;
use std::os::windows::ffi::OsStrExt;
use windows_sys::Win32::{
    Graphics::Gdi::{AddFontResourceW, RemoveFontResourceW},
    UI::WindowsAndMessaging::{
        SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_FONTCHANGE,
    },
};
use winreg::{enums::HKEY_CURRENT_USER, RegKey};
const REGISTRY: &str = "Software\\Microsoft\\Windows NT\\CurrentVersion\\Fonts";
pub struct WindowsInstaller {
    root: PathBuf,
}
impl WindowsInstaller {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            root: dirs::data_local_dir()
                .ok_or("local_data_unavailable")?
                .join("Microsoft/Windows/Fonts"),
        })
    }
}
fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}
fn key(face: &FontFace) -> String {
    format!("{} (TrueType)", face.postscript)
}
impl Platform for WindowsInstaller {
    fn root(&self) -> &Path {
        &self.root
    }
    fn register(&self, path: &Path, faces: &[FontFace]) -> Result<(), String> {
        let (k, _) = RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey(REGISTRY)
            .map_err(|e| e.to_string())?;
        for f in faces {
            k.set_value(key(f), &path.to_string_lossy().as_ref())
                .map_err(|e| e.to_string())?;
        }
        let result = unsafe { AddFontResourceW(wide(path).as_ptr()) };
        if result == 0 {
            return Err("font_registration_failed".into());
        }
        Ok(())
    }
    fn unregister(&self, path: &Path, faces: &[FontFace]) -> Result<(), String> {
        let (k, _) = RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey(REGISTRY)
            .map_err(|e| e.to_string())?;
        for f in faces {
            if k.get_value::<String, _>(key(f))
                .is_ok_and(|p| Path::new(&p) == path)
            {
                k.delete_value(key(f)).map_err(|e| e.to_string())?;
            }
        }
        unsafe {
            RemoveFontResourceW(wide(path).as_ptr());
        }
        Ok(())
    }
    fn refresh(&self) -> Result<(), String> {
        unsafe {
            SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_FONTCHANGE,
                0,
                0,
                SMTO_ABORTIFHUNG,
                1000,
                std::ptr::null_mut(),
            );
        }
        Ok(())
    }
    fn search_roots(&self) -> Vec<PathBuf> {
        let mut roots = vec![self.root.clone()];
        if let Some(w) = std::env::var_os("WINDIR") {
            roots.push(PathBuf::from(w).join("Fonts"));
        }
        roots
    }
}
