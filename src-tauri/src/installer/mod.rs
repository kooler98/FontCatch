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
    fn trash_owned(&self, path: &Path, _hash: &str) -> Result<(), String> {
        self.trash(path)
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
    match fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
    }
    verify_owned(platform, path, hash)?;
    platform.unregister(path, faces)?;
    let removal = platform
        .trash_owned(path, hash)
        .and_then(|_| match fs::symlink_metadata(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
            Ok(_) => Err("font_trash_failed".into()),
        });
    if let Err(error) = removal {
        // A failed Shell operation can outlive the user's edit or replacement.
        // Restore registration only for the exact file we originally removed.
        verify_owned(platform, path, hash)?;
        let _ = platform.register(path, faces);
        return Err(error);
    }
    Ok(())
}

fn verify_owned(platform: &dyn Platform, path: &Path, hash: &str) -> Result<(), String> {
    if path.parent() != Some(platform.root())
        || !path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .starts_with("Fontcatch-")
        // hash_file opens a regular file and rejects final symlinks/reparse points.
        || !hash_file(path, MAX_FILE).is_ok_and(|actual| actual == hash)
    {
        return Err("installed_font_changed".into());
    }
    Ok(())
}

#[cfg(any(target_os = "windows", test))]
enum OwnedTrashError {
    Retry(String),
    Stop(String),
}

#[cfg(any(target_os = "windows", test))]
fn retry_owned_trash(
    mut validate: impl FnMut() -> Result<(), String>,
    mut remove: impl FnMut() -> Result<(), OwnedTrashError>,
    mut wait: impl FnMut(std::time::Duration),
) -> Result<(), String> {
    // Allow transient Shell/font-cache locks to settle, but never loop indefinitely
    // or bypass the Recycle Bin. Each retry gets a fresh ownership/hash check.
    let mut delays = [100, 200, 400, 800, 1600].into_iter();
    loop {
        validate()?;
        match remove() {
            Ok(()) => return Ok(()),
            Err(OwnedTrashError::Stop(error)) => return Err(error),
            Err(OwnedTrashError::Retry(error)) => match delays.next() {
                Some(delay) => wait(std::time::Duration::from_millis(delay)),
                None => return Err(error),
            },
        }
    }
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
    use std::cell::Cell;
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
    #[test]
    fn owned_trash_retries_are_bounded_and_return_the_last_error() {
        let mut checks = 0;
        let mut attempts = 0;
        let mut delays = Vec::new();
        let result = retry_owned_trash(
            || {
                checks += 1;
                Ok(())
            },
            || {
                attempts += 1;
                Err(OwnedTrashError::Retry(format!("locked-{attempts}")))
            },
            |delay| delays.push(delay.as_millis()),
        );
        assert_eq!(result, Err("locked-6".into()));
        assert_eq!(checks, 6);
        assert_eq!(attempts, 6);
        assert_eq!(delays, [100, 200, 400, 800, 1600]);
    }
    #[test]
    fn owned_trash_stops_on_success_or_permanent_failure() {
        for succeed in [false, true] {
            let mut attempts = 0;
            let mut waits = 0;
            let result = retry_owned_trash(
                || Ok(()),
                || {
                    attempts += 1;
                    if attempts == 1 {
                        Err(OwnedTrashError::Retry("temporarily locked".into()))
                    } else if succeed {
                        Ok(())
                    } else {
                        Err(OwnedTrashError::Stop("recycling unavailable".into()))
                    }
                },
                |_| waits += 1,
            );
            assert_eq!(attempts, 2);
            assert_eq!(waits, 1);
            assert_eq!(
                result,
                if succeed {
                    Ok(())
                } else {
                    Err("recycling unavailable".into())
                }
            );
        }
    }
    #[test]
    fn owned_trash_rechecks_the_hash_before_retrying() {
        let root = tempfile::tempdir().unwrap();
        let platform = test_platform::Sandbox {
            root: root.path().into(),
            fail_register: false,
        };
        let font = fixture();
        let path = install(&platform, &font).unwrap();
        let mut attempts = 0;
        let result = retry_owned_trash(
            || verify_owned(&platform, &path, &font.hash),
            || {
                attempts += 1;
                fs::write(&path, b"user replacement").unwrap();
                Err(OwnedTrashError::Retry("temporarily locked".into()))
            },
            |_| {},
        );
        assert_eq!(result, Err("installed_font_changed".into()));
        assert_eq!(attempts, 1);
        assert_eq!(fs::read(path).unwrap(), b"user replacement");
    }
    struct FailedTrash {
        root: PathBuf,
        registrations: Cell<usize>,
        replace: bool,
        report_success: bool,
    }
    impl Platform for FailedTrash {
        fn root(&self) -> &Path {
            &self.root
        }
        fn register(&self, _: &Path, _: &[FontFace]) -> Result<(), String> {
            self.registrations.set(self.registrations.get() + 1);
            Ok(())
        }
        fn unregister(&self, _: &Path, _: &[FontFace]) -> Result<(), String> {
            Ok(())
        }
        fn refresh(&self) -> Result<(), String> {
            Ok(())
        }
        fn search_roots(&self) -> Vec<PathBuf> {
            vec![]
        }
        fn trash(&self, path: &Path) -> Result<(), String> {
            if self.replace {
                fs::write(path, b"user replacement").unwrap();
            }
            if self.report_success {
                Ok(())
            } else {
                Err("recycling unavailable".into())
            }
        }
    }
    #[test]
    fn failed_owned_trash_restores_only_the_unchanged_font() {
        for replace in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let platform = FailedTrash {
                root: root.path().into(),
                registrations: Cell::new(0),
                replace,
                report_success: false,
            };
            let font = fixture();
            let path = install(&platform, &font).unwrap();
            let error = remove_owned(&platform, &path, &font.hash, &font.faces).unwrap_err();
            assert!(path.exists());
            if replace {
                assert_eq!(error, "installed_font_changed");
                assert_eq!(platform.registrations.get(), 1);
                assert_eq!(fs::read(path).unwrap(), b"user replacement");
            } else {
                assert_eq!(error, "recycling unavailable");
                assert_eq!(platform.registrations.get(), 2);
                assert_eq!(hash_file(&path, MAX_FILE).unwrap(), font.hash);
            }
        }
    }
    #[test]
    fn a_reported_trash_success_must_actually_remove_the_font() {
        let root = tempfile::tempdir().unwrap();
        let platform = FailedTrash {
            root: root.path().into(),
            registrations: Cell::new(0),
            replace: false,
            report_success: true,
        };
        let font = fixture();
        let path = install(&platform, &font).unwrap();
        assert_eq!(
            remove_owned(&platform, &path, &font.hash, &font.faces),
            Err("font_trash_failed".into())
        );
        assert_eq!(platform.registrations.get(), 2);
        assert_eq!(hash_file(&path, MAX_FILE).unwrap(), font.hash);
    }
}
