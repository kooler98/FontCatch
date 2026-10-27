use crate::extractor::is_download;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
        Arc,
    },
    time::{Duration, Instant, SystemTime},
};
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    size: u64,
    modified: Option<SystemTime>,
}
fn stamp(path: &Path) -> Option<Stamp> {
    let m = std::fs::symlink_metadata(path).ok()?;
    if !m.is_file() || !is_download(path) {
        return None;
    }
    Some(Stamp {
        size: m.len(),
        modified: m.modified().ok(),
    })
}
fn scan(roots: &[PathBuf]) -> HashMap<PathBuf, Stamp> {
    let mut files = HashMap::new();
    for root in roots {
        for entry in walkdir::WalkDir::new(root)
            .max_depth(8)
            .follow_links(false)
            .follow_root_links(false)
            .into_iter()
            .filter_entry(|e| e.depth() == 0 || !e.file_name().to_string_lossy().starts_with('.'))
            .filter_map(Result::ok)
            .take(100_000)
        {
            if let Some(s) = stamp(entry.path()) {
                files.insert(entry.path().to_path_buf(), s);
            }
        }
    }
    files
}
fn normalize_roots(roots: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut normalized = Vec::new();
    for root in roots {
        // Native events use canonical paths (notably /private/var on macOS).
        // Resolve only the explicitly selected roots; never follow links below them.
        let root = root.canonicalize().unwrap_or(root);
        if !normalized.contains(&root) {
            normalized.push(root);
        }
    }
    normalized
}
#[derive(Default)]
pub struct Stability {
    pending: HashMap<PathBuf, (Stamp, Instant)>,
}
impl Stability {
    fn observe(&mut self, path: PathBuf, value: Stamp, now: Instant) {
        if self.pending.get(&path).is_none_or(|(old, _)| *old != value) {
            self.pending.insert(path, (value, now));
        }
    }
    fn ready(&mut self, now: Instant) -> Vec<PathBuf> {
        let mut ready = Vec::new();
        self.pending.retain(|path, (old, since)| {
            let Some(current) = stamp(path) else {
                return false;
            };
            if current != *old {
                *old = current;
                *since = now;
            }
            if old.size > 0 && now.duration_since(*since) >= Duration::from_secs(1) {
                ready.push(path.clone());
                false
            } else {
                true
            }
        });
        ready
    }
}
pub struct WatchService {
    watcher: RecommendedWatcher,
    events: Receiver<notify::Result<Event>>,
    roots: Vec<PathBuf>,
    registered: HashSet<PathBuf>,
    known: HashMap<PathBuf, Stamp>,
    stability: Stability,
    last_scan: Instant,
    rescan: Arc<AtomicBool>,
    pub errors: Vec<String>,
}
impl WatchService {
    pub fn new(roots: Vec<PathBuf>) -> Result<Self, String> {
        let roots = normalize_roots(roots);
        let (tx, events) = mpsc::sync_channel(512);
        let rescan = Arc::new(AtomicBool::new(false));
        let overflow = rescan.clone();
        let watcher = notify::recommended_watcher(move |event: notify::Result<Event>| {
            if event.as_ref().map_or(true, |e| e.need_rescan()) {
                overflow.store(true, Ordering::Relaxed);
            }
            if tx.try_send(event).is_err() {
                overflow.store(true, Ordering::Relaxed);
            }
        })
        .map_err(|e| e.to_string())?;
        let mut s = Self {
            watcher,
            events,
            known: scan(&roots),
            roots,
            registered: HashSet::new(),
            stability: Stability::default(),
            last_scan: Instant::now(),
            errors: vec![],
            rescan,
        };
        s.register();
        Ok(s)
    }
    fn register(&mut self) {
        self.errors.clear();
        for root in &self.roots {
            if !std::fs::symlink_metadata(root).is_ok_and(|m| m.is_dir()) {
                if self.registered.remove(root) {
                    let _ = self.watcher.unwatch(root);
                }
                self.errors
                    .push(format!("folder_unavailable:{}", root.display()));
                continue;
            }
            if !self.registered.contains(root) {
                match self.watcher.watch(root, RecursiveMode::Recursive) {
                    Ok(()) => {
                        self.registered.insert(root.clone());
                    }
                    Err(e) => self
                        .errors
                        .push(format!("watch_error:{}:{e}", root.display())),
                }
            }
        }
    }
    pub fn configure(&mut self, roots: Vec<PathBuf>) -> Result<(), String> {
        let roots = normalize_roots(roots);
        if roots != self.roots {
            // Preserve in-flight downloads in retained folders. Only a genuinely new
            // folder gets a baseline; changing another setting must not lose a catch.
            for old in &self.roots {
                if !roots.contains(old) && self.registered.remove(old) {
                    let _ = self.watcher.unwatch(old);
                }
            }
            self.known
                .retain(|p, _| roots.iter().any(|r| p.starts_with(r)));
            self.stability
                .pending
                .retain(|p, _| roots.iter().any(|r| p.starts_with(r)));
            let added: Vec<_> = roots
                .iter()
                .filter(|r| !self.roots.contains(r))
                .cloned()
                .collect();
            for (path, value) in scan(&added) {
                if !self.roots.iter().any(|r| path.starts_with(r)) {
                    self.known.insert(path, value);
                }
            }
            self.roots = roots;
            self.register();
        }
        Ok(())
    }
    fn allowed(&self, path: &Path) -> bool {
        self.roots.iter().any(|root| {
            let Ok(relative) = path.strip_prefix(root) else {
                return false;
            };
            let parts: Vec<_> = relative.components().collect();
            if parts.is_empty()
                || parts.len() > 8
                || !std::fs::symlink_metadata(root).is_ok_and(|m| m.is_dir())
            {
                return false;
            }
            let mut current = root.clone();
            for (i, part) in parts.iter().enumerate() {
                if !matches!(part, std::path::Component::Normal(_))
                    || part.as_os_str().to_string_lossy().starts_with('.')
                {
                    return false;
                }
                current.push(part.as_os_str());
                let Ok(meta) = std::fs::symlink_metadata(&current) else {
                    return false;
                };
                if i + 1 == parts.len() {
                    return meta.is_file() && is_download(&current);
                }
                if !meta.is_dir() {
                    return false;
                }
            }
            false
        })
    }
    fn scan_due(&self, now: Instant) -> bool {
        let elapsed = now.duration_since(self.last_scan);
        elapsed >= Duration::from_secs(30)
            || (self.rescan.load(Ordering::Relaxed) && elapsed >= Duration::from_secs(2))
    }
    pub fn poll(&mut self) -> Vec<PathBuf> {
        let now = Instant::now();
        for _ in 0..256 {
            let Ok(event) = self.events.try_recv() else {
                break;
            };
            if let Ok(event) = event {
                let directory_change = matches!(
                    event.kind,
                    notify::EventKind::Create(_)
                        | notify::EventKind::Remove(_)
                        | notify::EventKind::Modify(notify::event::ModifyKind::Name(_))
                );
                for path in event.paths {
                    if directory_change && self.roots.iter().any(|r| path.starts_with(r)) {
                        // Directory renames may not emit one event per contained font.
                        if path.is_dir() {
                            self.rescan.store(true, Ordering::Relaxed);
                        }
                        if !path.exists() {
                            self.known.remove(&path);
                            self.stability.pending.remove(&path);
                        }
                    }
                    if self.allowed(&path) {
                        if let Some(s) = stamp(&path) {
                            if self.known.get(&path) != Some(&s) {
                                self.stability.observe(path.clone(), s.clone(), now);
                                self.known.insert(path, s);
                            }
                        }
                    }
                }
            }
        }
        if self.scan_due(now) {
            self.rescan.store(false, Ordering::Relaxed);
            self.register();
            let current = scan(&self.roots);
            for (path, s) in &current {
                if self.known.get(path) != Some(s) {
                    self.stability.observe(path.clone(), s.clone(), now);
                }
            }
            self.known = current;
            self.last_scan = now;
        }
        self.stability
            .ready(now)
            .into_iter()
            .filter(|p| self.allowed(p))
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn idle_scans_are_infrequent_but_overflow_recovers_promptly() {
        let r = tempfile::tempdir().unwrap();
        let s = WatchService::new(vec![r.path().into()]).unwrap();
        assert!(!s.scan_due(s.last_scan + Duration::from_secs(2)));
        assert!(s.scan_due(s.last_scan + Duration::from_secs(30)));
        s.rescan.store(true, Ordering::Relaxed);
        assert!(s.scan_due(s.last_scan + Duration::from_secs(2)));
    }
    #[cfg(unix)]
    #[test]
    fn events_cannot_import_through_a_linked_or_hidden_directory() {
        let r = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("outside.ttf"), b"font").unwrap();
        std::os::unix::fs::symlink(outside.path(), r.path().join("linked")).unwrap();
        std::fs::create_dir(r.path().join(".hidden")).unwrap();
        std::fs::write(r.path().join(".hidden/font.ttf"), b"font").unwrap();
        let s = WatchService::new(vec![r.path().into()]).unwrap();
        assert!(!s.allowed(&r.path().join("linked/outside.ttf")));
        assert!(!s.allowed(&r.path().join(".hidden/font.ttf")));
        assert!(s.known.is_empty());
    }
    #[test]
    fn stable_for_one_second_and_growth_resets_clock() {
        let r = tempfile::tempdir().unwrap();
        let p = r.path().join("font.ttf");
        std::fs::write(&p, b"a").unwrap();
        let now = Instant::now();
        let mut gate = Stability::default();
        gate.observe(p.clone(), stamp(&p).unwrap(), now);
        assert!(gate.ready(now + Duration::from_millis(900)).is_empty());
        std::fs::write(&p, b"larger").unwrap();
        assert!(gate.ready(now + Duration::from_millis(1100)).is_empty());
        assert_eq!(gate.ready(now + Duration::from_millis(2200)), vec![p]);
    }
    #[test]
    fn startup_does_not_ingest_existing_downloads() {
        let r = tempfile::tempdir().unwrap();
        std::fs::write(r.path().join("old.ttf"), b"a").unwrap();
        let mut s = WatchService::new(vec![r.path().into()]).unwrap();
        assert!(s.poll().is_empty());
        assert_eq!(s.known.len(), 1);
    }
    #[test]
    fn adding_folder_preserves_download_in_progress_and_baselines_new_folder() {
        let first = tempfile::tempdir().unwrap();
        let added = tempfile::tempdir().unwrap();
        let mut watcher = WatchService::new(vec![first.path().into()]).unwrap();
        let path = first.path().canonicalize().unwrap().join("downloading.ttf");
        let old = added.path().canonicalize().unwrap().join("existing.ttf");
        std::fs::write(&path, b"font").unwrap();
        std::fs::write(&old, b"old").unwrap();
        let now = Instant::now();
        let value = stamp(&path).unwrap();
        watcher.known.insert(path.clone(), value.clone());
        watcher.stability.observe(path.clone(), value, now);
        watcher
            .configure(vec![first.path().into(), added.path().into()])
            .unwrap();
        assert!(watcher.known.contains_key(&old));
        assert_eq!(
            watcher.stability.ready(now + Duration::from_secs(2)),
            vec![path]
        );
    }
    #[test]
    fn deleted_and_empty_downloads_not_ready() {
        let r = tempfile::tempdir().unwrap();
        let p = r.path().join("empty.ttf");
        std::fs::write(&p, b"").unwrap();
        let now = Instant::now();
        let mut gate = Stability::default();
        gate.observe(p.clone(), stamp(&p).unwrap(), now);
        assert!(gate.ready(now + Duration::from_secs(2)).is_empty());
        std::fs::remove_file(p).unwrap();
        assert!(gate.ready(now + Duration::from_secs(3)).is_empty());
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;
    #[test]
    fn new_file_is_delivered_once_by_live_watcher() {
        let root = tempfile::tempdir().unwrap();
        let mut watcher = WatchService::new(vec![root.path().into()]).unwrap();
        let path = root.path().canonicalize().unwrap().join("new.ttf");
        std::fs::write(&path, b"download").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut delivered = Vec::new();
        while Instant::now() < deadline && delivered.is_empty() {
            delivered.extend(watcher.poll());
            std::thread::sleep(Duration::from_millis(100));
        }
        assert_eq!(delivered, vec![path]);
        assert!(watcher.poll().is_empty());
    }
    #[test]
    fn safari_partial_directory_is_not_processed() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("font.download");
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("font.ttf");
        std::fs::write(&path, b"partial").unwrap();
        assert!(stamp(&path).is_none());
        assert!(scan(&[root.path().into()]).is_empty());
    }
}
