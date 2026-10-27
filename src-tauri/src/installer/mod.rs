use crate::{
    extractor::{extension, hash_file, MAX_FILE},
    inspector,
    model::{FontFace, FontFile, InstalledFont},
};
use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

pub trait Platform: Send {
    fn root(&self) -> &Path;
    fn register(&self, path: &Path, faces: &[FontFace]) -> Result<(), String>;
    fn unregister(&self, path: &Path, faces: &[FontFace]) -> Result<(), String>;
    fn refresh(&self) -> Result<(), String>;
    fn search_roots(&self) -> Vec<PathBuf>;
    fn trash(&self, path: &Path) -> Result<(), String> {
        trash::delete(path).map_err(|e| e.to_string())
    }
}
pub fn native() -> Result<Box<dyn Platform>, String> {
    #[cfg(target_os = "macos")]
    return Ok(Box::new(macos::MacInstaller::new()?));
    #[cfg(target_os = "windows")]
    return Ok(Box::new(windows::WindowsInstaller::new()?));
    #[cfg(target_os = "linux")]
    return Ok(Box::new(linux::LinuxInstaller::new()?));
}
pub fn target(platform: &dyn Platform, file: &FontFile) -> PathBuf {
    platform
        .root()
        .join(format!("Fontcatch-{}.{}", file.hash, extension(&file.path)))
}
pub fn install(platform: &dyn Platform, file: &FontFile) -> Result<PathBuf, String> {
    if hash_file(&file.path, MAX_FILE)? != file.hash {
        return Err("staged_font_changed".into());
    }
    fs::create_dir_all(platform.root()).map_err(|e| e.to_string())?;
    let target = target(platform, file);
    // No overwrite, including files left by another app or an unrecorded crash.
    if target.exists() {
        return Err("destination_exists".into());
    }
    let mut tmp = tempfile::NamedTempFile::new_in(platform.root()).map_err(|e| e.to_string())?;
    let count = std::io::copy(
        &mut crate::files::open_regular(&file.path, MAX_FILE)?.take(MAX_FILE + 1),
        &mut tmp,
    )
    .map_err(|e| e.to_string())?;
    if count > MAX_FILE {
        return Err("staged_font_changed".into());
    }
    tmp.flush().map_err(|e| e.to_string())?;
    tmp.as_file().sync_all().map_err(|e| e.to_string())?;
    if hash_file(tmp.path(), MAX_FILE)? != file.hash {
        return Err("staged_font_changed".into());
    }
    tmp.persist_noclobber(&target).map_err(|e| e.to_string())?;
    platform.register(&target, &file.faces)?;
    Ok(target)
}
pub fn remove_owned(
    platform: &dyn Platform,
    path: &Path,
    hash: &str,
    faces: &[FontFace],
) -> Result<(), String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };
    if !metadata.is_file() {
        return Err("installed_font_changed".into());
    }
    if path.parent() != Some(platform.root())
        || !path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .starts_with("Fontcatch-")
        || hash_file(path, MAX_FILE)? != hash
    {
        return Err("installed_font_changed".into());
    }
    platform.unregister(path, faces)?;
    if let Err(error) = platform.trash(path) {
        let _ = platform.register(path, faces);
        return Err(error);
    }
    Ok(())
}
#[derive(Debug, Default)]
pub struct Plan {
    pub install: Vec<FontFile>,
    pub retired: Vec<InstalledFont>,
    pub skipped: usize,
    pub updates: bool,
    pub warnings: Vec<String>,
}
pub fn plan(files: &[FontFile], owned: &[InstalledFont], external: &HashMap<String, u32>) -> Plan {
    let owned: HashMap<_, _> = owned
        .iter()
        .filter(|f| f.path.is_file())
        .map(|f| (&f.face.postscript, f))
        .collect();

    let mut p = Plan::default();
    for file in files {
        let mut skip = false;
        let mut retired = Vec::new();
        for face in &file.faces {
            if external.contains_key(&face.postscript) {
                p.warnings.push("external_duplicate".into());
                skip = true;
                continue;
            }
            if let Some(old) = owned.get(&face.postscript) {
                if old.hash == file.hash || face.revision <= old.face.revision {
                    skip = true;
                    p.warnings.push("already_installed".into());
                } else if owned.values().any(|other| {
                    other.path == old.path
                        && !file
                            .faces
                            .iter()
                            .any(|f| f.postscript == other.face.postscript)
                }) {
                    skip = true;
                    p.warnings.push("partial_collection_update".into());
                } else {
                    retired.push((*old).clone());
                }
            }
        }
        if skip {
            p.skipped += file.faces.len();
        } else {
            p.updates |= !retired.is_empty();
            p.retired.extend(retired);
            p.install.push(file.clone());
        }
    }
    p.warnings.sort();
    p.warnings.dedup();
    p
}

type FontStamp = (u64, Option<std::time::SystemTime>);
type FontNames = Vec<(String, u32)>;
#[derive(Default)]
pub struct Inventory {
    entries: HashMap<PathBuf, (FontStamp, FontNames)>,
}
impl Inventory {
    pub fn scan(
        &mut self,
        platform: &dyn Platform,
        owned: &[InstalledFont],
    ) -> HashMap<String, u32> {
        let owned: HashMap<_, _> = owned.iter().map(|f| (&f.path, &f.hash)).collect();
        let mut known = HashMap::new();
        let mut seen = std::collections::HashSet::new();
        for root in platform.search_roots() {
            for entry in walkdir::WalkDir::new(root)
                .max_depth(5)
                .follow_links(false)
                .into_iter()
                .filter_map(Result::ok)
                .take(20_000)
            {
                let path = entry.path();
                if !entry.file_type().is_file() || !crate::extractor::is_font(path) {
                    continue;
                }
                if let Some(hash) = owned.get(&path.to_path_buf()) {
                    if hash_file(path, MAX_FILE).is_ok_and(|h| h == **hash) {
                        continue;
                    }
                }
                let Ok(meta) = entry.metadata() else {
                    continue;
                };
                let stamp = (meta.len(), meta.modified().ok());
                seen.insert(path.to_path_buf());
                if self.entries.get(path).is_none_or(|(old, _)| *old != stamp) {
                    // Index only names/revisions. Do not hash every external font on every catch.
                    let faces = inspector::metadata(path)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|f| (f.postscript, f.revision))
                        .collect();
                    self.entries.insert(path.into(), (stamp, faces));
                }
                if let Some((_, faces)) = self.entries.get(path) {
                    for (ps, revision) in faces {
                        known
                            .entry(ps.clone())
                            .and_modify(|v: &mut u32| *v = (*v).max(*revision))
                            .or_insert(*revision);
                    }
                }
            }
        }
        self.entries.retain(|p, _| seen.contains(p));
        known
    }
}
#[cfg(test)]
pub mod test_platform {
    use super::*;
    pub struct Sandbox {
        pub root: PathBuf,
        pub fail_register: bool,
    }
    impl Platform for Sandbox {
        fn root(&self) -> &Path {
            &self.root
        }
        fn register(&self, _: &Path, _: &[FontFace]) -> Result<(), String> {
            if self.fail_register {
                Err("registration_failed".into())
            } else {
                Ok(())
            }
        }
        fn unregister(&self, _: &Path, _: &[FontFace]) -> Result<(), String> {
            Ok(())
        }
        fn refresh(&self) -> Result<(), String> {
            Ok(())
        }
        fn search_roots(&self) -> Vec<PathBuf> {
            vec![self.root.clone()]
        }
        fn trash(&self, path: &Path) -> Result<(), String> {
            let dest = self.root.join("Trash");
            fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
            fs::rename(path, dest.join(path.file_name().unwrap())).map_err(|e| e.to_string())
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::now;
    fn fixture() -> FontFile {
        inspector::inspect(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/FontcatchQA-Regular.ttf")
                .as_path(),
        )
        .unwrap()
    }
    #[test]
    fn newer_version_requires_explicit_update() {
        let root = tempfile::tempdir().unwrap();
        let f = fixture();
        let mut old = InstalledFont {
            face: f.faces[0].clone(),
            path: f.path.clone(),
            hash: "older".into(),
            installed_at: now(),
            source_archive: String::new(),
            source_url: None,
        };
        old.face.revision -= 1;
        let p = plan(&[f], &[old], &HashMap::new());
        assert!(p.updates);
        assert_eq!(p.install.len(), 1);
        drop(root);
    }
    #[test]
    fn same_and_older_versions_are_duplicates() {
        let f = fixture();
        let old = InstalledFont {
            face: f.faces[0].clone(),
            path: f.path.clone(),
            hash: f.hash.clone(),
            installed_at: now(),
            source_archive: String::new(),
            source_url: None,
        };
        let p = plan(&[f], &[old], &HashMap::new());
        assert!(p.install.is_empty());
        assert_eq!(p.skipped, 1);
    }
    #[test]
    fn externally_installed_fonts_are_never_replaced() {
        let f = fixture();
        let mut external = HashMap::new();
        external.insert(f.faces[0].postscript.clone(), 1);
        let p = plan(&[f], &[], &external);
        assert!(p.install.is_empty());
        assert!(!p.updates);
    }
    #[test]
    fn external_inventory_refreshes_changed_and_removed_files() {
        let root = tempfile::tempdir().unwrap();
        let platform = test_platform::Sandbox {
            root: root.path().into(),
            fail_register: false,
        };
        let original = fixture();
        let path = root.path().join("external.ttf");
        fs::copy(&original.path, &path).unwrap();
        let mut inventory = Inventory::default();
        let ps = &original.faces[0].postscript;
        assert_eq!(
            inventory.scan(&platform, &[])[ps],
            original.faces[0].revision
        );
        fs::copy(original.path.with_file_name("FontcatchQA-Newer.ttf"), &path).unwrap();
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(2))
            .unwrap();
        assert!(inventory.scan(&platform, &[])[ps] > original.faces[0].revision);
        fs::remove_file(path).unwrap();
        assert!(inventory.scan(&platform, &[]).is_empty());
    }
    #[test]
    fn destination_collision_does_not_overwrite() {
        let root = tempfile::tempdir().unwrap();
        let p = test_platform::Sandbox {
            root: root.path().into(),
            fail_register: false,
        };
        let f = fixture();
        let dest = target(&p, &f);
        fs::write(&dest, b"existing").unwrap();
        assert_eq!(install(&p, &f).unwrap_err(), "destination_exists");
        assert_eq!(fs::read(dest).unwrap(), b"existing");
    }
    #[test]
    fn modified_owned_file_is_not_trashed() {
        let root = tempfile::tempdir().unwrap();
        let p = test_platform::Sandbox {
            root: root.path().into(),
            fail_register: false,
        };
        let f = fixture();
        let dest = install(&p, &f).unwrap();
        fs::write(&dest, b"user-modified").unwrap();
        assert!(remove_owned(&p, &dest, &f.hash, &f.faces).is_err());
        assert!(dest.exists());
    }
}
