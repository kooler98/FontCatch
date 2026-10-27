use crate::{
    extractor, inspector,
    installer::{self, Platform},
    library::Library,
    model::{now, FontFile, InstalledFont, Job, Snapshot, UninstallOutcome, UninstallPlan},
    origin,
    settings::{Language, Mode, Settings},
    watcher::WatchService,
};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::mpsc::{self, Sender},
    time::Duration,
};
use tauri::Emitter;
use tauri_plugin_notification::NotificationExt;

type Reply = Sender<Result<Snapshot, String>>;
pub enum Message {
    Snapshot(Reply),
    Configure(Settings, Reply),
    Install(String, bool, Reply),
    Ignore(String, Reply),
    Preview(String, Sender<Result<Vec<u8>, String>>),
    Font(String, Sender<Result<InstalledFont, String>>),
    UninstallPlan(String, Sender<Result<UninstallPlan, String>>),
    Uninstall(
        String,
        String,
        bool,
        Sender<Result<UninstallOutcome, String>>,
    ),
}
#[derive(Clone)]
pub struct EngineHandle(mpsc::SyncSender<Message>);
impl EngineHandle {
    pub fn request<T>(
        &self,
        create: impl FnOnce(Sender<Result<T, String>>) -> Message,
    ) -> Result<T, String> {
        let (tx, rx) = mpsc::channel();
        self.0.try_send(create(tx)).map_err(|e| match e {
            mpsc::TrySendError::Full(_) => "engine_busy",
            mpsc::TrySendError::Disconnected(_) => "engine_unavailable",
        })?;
        rx.recv().map_err(|_| "engine_unavailable")?
    }
}
pub struct Engine {
    pub library: Library,
    cache: PathBuf,
    settings: Settings,
    platform: Box<dyn Platform>,
    inventory: installer::Inventory,
    processing: Option<String>,
    watcher_errors: Vec<String>,
    app: Option<tauri::AppHandle>,
}
impl Engine {
    pub fn new(
        data: &Path,
        settings: Settings,
        platform: Box<dyn Platform>,
        app: Option<tauri::AppHandle>,
    ) -> Result<Self, String> {
        crate::files::private_dir(data)?;
        let cache = data.join("pending");
        crate::files::private_dir(&cache)?;
        let mut engine = Self {
            library: Library::new(&data.join("library.sqlite")).map_err(|e| e.to_string())?,
            cache,
            settings,
            platform,
            inventory: installer::Inventory::default(),
            processing: None,
            watcher_errors: vec![],
            app,
        };
        engine.recover()?;
        Ok(engine)
    }
    pub fn snapshot(&self) -> Result<Snapshot, String> {
        Ok(Snapshot {
            recent: self
                .library
                .recent()?
                .into_iter()
                .map(|mut job| {
                    // History needs names/styles, not another copy of every license text.
                    for file in &mut job.files {
                        for face in &mut file.faces {
                            face.license = None;
                            face.license_url = None;
                            face.axes.clear();
                        }
                    }
                    job
                })
                .collect(),
            installed: self.library.installed()?,
            processing: self.processing.clone(),
            watcher_errors: self.watcher_errors.clone(),
        })
    }
    fn publish(&self) {
        if let Some(app) = &self.app {
            if let Ok(snapshot) = self.snapshot() {
                let pending = snapshot
                    .recent
                    .iter()
                    .filter(|j| j.status == "pending")
                    .count();
                crate::panel::set_pending(app, pending);
                let _ = app.emit("library-changed", snapshot);
            }
        }
    }
    fn notify(&self, job: &Job, installed: bool) {
        let Some(app) = &self.app else { return };
        let n = app.notification();
        if !matches!(
            n.permission_state(),
            Ok(tauri::plugin::PermissionState::Granted)
        ) {
            return;
        }
        let mut names: Vec<_> = job
            .files
            .iter()
            .flat_map(|f| f.faces.iter().map(|f| f.family.as_str()))
            .collect();
        names.sort();
        names.dedup();
        let title = if installed {
            if self.settings.language == Language::Fr {
                "Typo installée"
            } else {
                "Typeface installed"
            }
        } else if self.settings.language == Language::Fr {
            "Nouvelle typo détectée"
        } else {
            "New typeface detected"
        };
        let count = if installed {
            job.installed_count
        } else {
            job.files.iter().map(|f| f.faces.len()).sum()
        };
        let body = format!(
            "{} ({} {})",
            names.join(" + "),
            count,
            if count == 1 { "style" } else { "styles" }
        );
        let _ = n.builder().title(title).body(body).show();
    }
    fn save(&self, job: &Job) -> Result<(), String> {
        self.library.save_job(job)?;
        self.publish();
        Ok(())
    }
    pub fn ingest(&mut self, path: &Path) -> Result<Option<String>, String> {
        if path.starts_with(&self.cache)
            || path.starts_with(self.platform.root())
            || !extractor::is_download(path)
        {
            return Ok(None);
        }
        let hash = extractor::hash_file(path, extractor::MAX_ARCHIVE)?;
        if self.library.seen(path, &hash)? {
            return Ok(None);
        }
        self.processing = Some(
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into(),
        );
        self.publish();
        let result = self.prepare(path, &hash);
        self.processing = None;
        match result {
            Ok(None) => {
                self.library.mark_seen(path, &hash)?;
                self.publish();
                Ok(None)
            }
            Ok(Some(mut job)) => {
                self.library.save_job(&job)?;
                self.library.mark_seen(path, &hash)?;
                self.publish();
                let id = job.id.clone();
                if job.status == "pending"
                    && self.settings.mode == Mode::Auto
                    && !job.requires_update
                {
                    if let Err(e) = self.install_job(&id, false) {
                        job = self.find(&id)?;
                        if job.status != "pending" {
                            job.error = Some(e);
                            self.save(&job)?;
                        }
                    }
                } else if job.status == "pending" {
                    self.notify(&job, false);
                }
                Ok(Some(id))
            }
            Err(error) => {
                // A changing download will be retried by the stability gate, never marked seen.
                if error == "download_changed" {
                    self.publish();
                    return Ok(None);
                }
                let mut job = self.empty_job(path, hash);
                job.status = "failed".into();
                job.error = Some(error);
                self.library.save_job(&job)?;
                self.library.mark_seen(path, &job.source_hash)?;
                self.publish();
                Ok(Some(job.id))
            }
        }
    }
    fn empty_job(&self, path: &Path, hash: String) -> Job {
        Job {
            id: uuid::Uuid::new_v4().to_string(),
            source_path: path.into(),
            source_hash: hash,
            source_url: origin::origin(path),
            created_at: now(),
            updated_at: now(),
            status: "pending".into(),
            files: vec![],
            warnings: vec![],
            error: None,
            requires_update: false,
            installed_count: 0,
            skipped_count: 0,
            removed_count: 0,
            planned: vec![],
            retired: vec![],
        }
    }
    fn prepare(&mut self, path: &Path, hash: &str) -> Result<Option<Job>, String> {
        let extracted = extractor::extract(path, &self.cache)?;
        if extractor::hash_file(path, extractor::MAX_ARCHIVE)? != hash {
            return Err("download_changed".into());
        }
        if extracted.files.is_empty() && !extracted.web_only {
            return Ok(None);
        }
        let mut job = self.empty_job(path, hash.into());
        if extracted.web_only {
            job.status = "ignored".into();
            job.warnings.push("web_fonts_only".into());
            return Ok(Some(job));
        }
        let mut fonts = Vec::new();
        let mut invalid = false;
        let mut face_count = 0usize;
        for file in &extracted.files {
            match inspector::inspect(file) {
                Ok(font) => {
                    face_count += font.faces.len();
                    if face_count > 1024 {
                        return Err("archive_limit".into());
                    }
                    fonts.push(font);
                }
                Err(_) => invalid = true,
            }
        }
        if fonts.is_empty() {
            return Err("invalid_font".into());
        }
        let (files, mut warnings) = inspector::select(fonts, &self.settings.font_preference);
        if invalid {
            warnings.push("some_invalid_fonts".into());
        }
        job.warnings = warnings;
        job.files = files;
        if job.files.is_empty() {
            job.status = "ignored".into();
            return Ok(Some(job));
        }
        let owned = self.library.installed()?;
        let external = self.inventory.scan(self.platform.as_ref(), &owned);
        let plan = installer::plan(&job.files, &owned, &external);
        job.requires_update = plan.updates;
        job.skipped_count = plan.skipped;
        job.warnings.extend(plan.warnings);
        if plan.install.is_empty() {
            job.status = "ignored".into();
            return Ok(Some(job));
        }
        // Persist the independent snapshot for approval even if the original is moved/deleted.
        let dest = self.cache.join(&job.id);
        let old = extracted.dir.path().to_path_buf();
        std::fs::rename(&old, &dest).map_err(|e| e.to_string())?;
        for file in &mut job.files {
            file.path = dest.join(
                file.path
                    .strip_prefix(&old)
                    .map_err(|_| "invalid_staging_path")?,
            );
        }
        Ok(Some(job))
    }
    fn find(&self, id: &str) -> Result<Job, String> {
        self.library.job(id)
    }
    fn font(&self, postscript: &str) -> Result<InstalledFont, String> {
        self.library
            .installed()?
            .into_iter()
            .find(|f| f.face.postscript == postscript)
            .ok_or_else(|| "font_not_found".into())
    }
    fn preview(&self, postscript: &str) -> Result<Vec<u8>, String> {
        crate::preview::read(&self.font(postscript)?, self.platform.root())
    }
    fn removal_fonts(&self, family: &str) -> Result<Vec<InstalledFont>, String> {
        let all = self.library.installed()?;
        let paths: HashSet<_> = all
            .iter()
            .filter(|f| f.face.family == family)
            .map(|f| f.path.clone())
            .collect();
        if paths.is_empty() {
            return Err("font_not_found".into());
        }
        // A TTC may contain more than one family. The confirmation includes them all.
        let mut fonts: Vec<_> = all
            .into_iter()
            .filter(|f| paths.contains(&f.path))
            .collect();
        fonts.sort_by(|a, b| a.face.postscript.cmp(&b.face.postscript));
        Ok(fonts)
    }
    fn uninstall_plan(&self, family: &str) -> Result<UninstallPlan, String> {
        use sha2::{Digest, Sha256};
        let fonts = self.removal_fonts(family)?;
        let fingerprint = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&fonts).map_err(|e| e.to_string())?)
        );
        let mut families: Vec<_> = fonts.iter().map(|f| f.face.family.clone()).collect();
        families.sort();
        families.dedup();
        Ok(UninstallPlan {
            family: family.into(),
            fingerprint,
            families,
            font_count: fonts.len(),
            file_count: fonts.iter().map(|f| &f.path).collect::<HashSet<_>>().len(),
        })
    }
    fn uninstall(
        &mut self,
        family: &str,
        fingerprint: &str,
        confirmed: bool,
    ) -> Result<UninstallOutcome, String> {
        if !confirmed {
            return Err("uninstall_confirmation_required".into());
        }
        if self.uninstall_plan(family)?.fingerprint != fingerprint {
            return Err("library_changed_review".into());
        }
        let fonts = self.removal_fonts(family)?;
        let mut job = self.empty_job(Path::new(family), fingerprint.into());
        job.status = "uninstalling".into();
        job.retired = fonts;
        let mut paths = HashSet::new();
        for f in &job.retired {
            if paths.insert(f.path.clone()) {
                job.files.push(FontFile {
                    path: f.path.clone(),
                    hash: f.hash.clone(),
                    faces: job
                        .retired
                        .iter()
                        .filter(|other| other.path == f.path)
                        .map(|other| other.face.clone())
                        .collect(),
                });
            }
        }
        self.save(&job)?; // Persist the user's exact consent before touching files.
        let error = self.resume_uninstall(&mut job).err();
        self.publish();
        Ok(UninstallOutcome {
            snapshot: self.snapshot()?,
            removed_count: job.removed_count,
            error,
        })
    }
    fn resume_uninstall(&mut self, job: &mut Job) -> Result<(), String> {
        while let Some(first) = job.retired.first().cloned() {
            let group: Vec<_> = job
                .retired
                .iter()
                .filter(|f| f.path == first.path)
                .cloned()
                .collect();
            let owned = self.library.installed()?;
            let unchanged = group.iter().all(|f| {
                owned.iter().any(|current| {
                    current.face.postscript == f.face.postscript
                        && current.path == f.path
                        && current.hash == f.hash
                })
            });
            let result = if !unchanged || first.path.parent() != Some(self.platform.root()) {
                Err("library_changed_review".into())
            } else {
                installer::remove_owned(
                    self.platform.as_ref(),
                    &first.path,
                    &first.hash,
                    &group.iter().map(|f| f.face.clone()).collect::<Vec<_>>(),
                )
            };
            if let Err(error) = result {
                job.status = "uninstall_failed".into();
                job.error = Some(error.clone());
                self.save(job)?;
                let _ = self.platform.refresh();
                return Err(error);
            }
            let mut next = job.clone();
            next.retired.retain(|f| f.path != first.path);
            next.removed_count += group.len();
            next.updated_at = now();
            if next.retired.is_empty() {
                next.status = "uninstalled".into();
            }
            // If a crash occurs after Trash but before this commit, the journal still
            // identifies the missing file and recovery completes this exact step.
            self.library.commit_uninstall(&group, &next)?;
            *job = next;
            self.publish();
        }
        if self.platform.refresh().is_err() {
            job.warnings.push("font_refresh_failed".into());
            self.save(job)?;
        }
        for cancelled in self.library.jobs()?.iter().filter(|j| {
            j.status == "ignored" && j.warnings.iter().any(|w| w == "removed_pending_cancelled")
        }) {
            self.clean_cache(cancelled);
        }
        Ok(())
    }
    fn configure(&mut self, settings: Settings) -> Result<(), String> {
        self.settings = settings;
        self.reconcile_pending()
    }
    fn reconcile_pending(&mut self) -> Result<(), String> {
        // A pending request is a live decision, not a permanent snapshot of the
        // install state: another download may have installed the same PostScript name.
        for mut job in self
            .library
            .pending()?
            .into_iter()
            .filter(|j| j.status == "pending")
        {
            let owned = self.library.installed()?;
            let external = self.inventory.scan(self.platform.as_ref(), &owned);
            let plan = installer::plan(&job.files, &owned, &external);
            job.requires_update = plan.updates;
            job.skipped_count = plan.skipped;
            job.warnings.extend(plan.warnings);
            job.warnings.sort();
            job.warnings.dedup();
            if plan.install.is_empty() {
                job.status = "ignored".into();
                job.updated_at = now();
                self.save(&job)?;
                self.clean_cache(&job);
            } else {
                self.library.save_job(&job)?;
                if self.settings.mode == Mode::Auto && !plan.updates {
                    // Re-plan immediately before writing. Never approve an update on
                    // behalf of the user, including one discovered during this pass.
                    if let Err(error) = self.install_job(&job.id, false) {
                        let mut current = self.find(&job.id)?;
                        if error != "update_confirmation_required" && current.status == "pending" {
                            current.status = "failed".into();
                            current.error = Some(error);
                            current.updated_at = now();
                            self.save(&current)?;
                        }
                    }
                }
            }
        }
        self.publish();
        Ok(())
    }
    pub fn ignore(&mut self, id: &str) -> Result<(), String> {
        let mut job = self.find(id)?;
        if job.status != "pending" && job.status != "failed" {
            return Err("job_not_pending".into());
        }
        if !job.planned.is_empty() {
            return Err("rollback_incomplete".into());
        }
        job.status = "ignored".into();
        job.updated_at = now();
        self.save(&job)?;
        self.clean_cache(&job);
        Ok(())
    }
    fn clean_cache(&self, job: &Job) {
        if uuid::Uuid::parse_str(&job.id).is_ok() {
            let _ = std::fs::remove_dir_all(self.cache.join(&job.id));
        }
    }
    pub fn install_job(&mut self, id: &str, confirm_update: bool) -> Result<(), String> {
        let mut job = self.find(id)?;
        if job.status != "pending" && job.status != "failed" {
            return Err("job_not_pending".into());
        }
        if !job.planned.is_empty() {
            return Err("rollback_incomplete".into());
        }
        if job.files.is_empty() {
            return Err("no_installable_fonts".into());
        }
        let owned = self.library.installed()?;
        let external = self.inventory.scan(self.platform.as_ref(), &owned);
        let plan = installer::plan(&job.files, &owned, &external);
        job.skipped_count = plan.skipped;
        job.warnings.extend(plan.warnings);
        job.warnings.sort();
        job.warnings.dedup();
        job.requires_update = plan.updates;
        if plan.updates && !confirm_update {
            job.status = "pending".into();
            self.save(&job)?;
            return Err("update_confirmation_required".into());
        }
        if plan.install.is_empty() {
            job.status = "ignored".into();
            job.updated_at = now();
            self.save(&job)?;
            self.clean_cache(&job);
            return Ok(());
        }
        // Journal every intended new file before touching the OS font directory.
        // Pre-existing destinations are never claimed by the rollback journal.
        for file in &plan.install {
            if installer::target(self.platform.as_ref(), file).exists() {
                return Err("destination_exists".into());
            }
        }
        job.status = "installing".into();
        job.error = None;
        job.planned = plan.install;
        job.retired = plan.retired;
        job.updated_at = now();
        self.save(&job)?;
        let mut installed = Vec::new();
        let result = (|| -> Result<(), String> {
            for file in &job.planned {
                let path = installer::install(self.platform.as_ref(), file)?;
                for face in &file.faces {
                    installed.push(InstalledFont {
                        face: face.clone(),
                        path: path.clone(),
                        hash: file.hash.clone(),
                        installed_at: now(),
                        source_archive: job
                            .source_path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into(),
                        source_url: job.source_url.clone(),
                    });
                }
            }
            self.platform.refresh()?;
            Ok(())
        })();
        if let Err(error) = result {
            return self.fail_install(&mut job, error);
        }
        job.status = "installed".into();
        job.installed_count = installed.len();
        job.updated_at = now();
        // Commit metadata and completion atomically; a crash before this rolls back all new resources.
        if let Err(error) = self.library.commit_install(&installed, &job) {
            return self.fail_install(&mut job, error);
        }
        self.retire_old(&mut job);
        job.planned.clear();
        if self.settings.trash_after_install && extractor::extension(&job.source_path) == "zip" {
            if extractor::hash_file(&job.source_path, extractor::MAX_ARCHIVE)
                .is_ok_and(|h| h == job.source_hash)
            {
                if self.platform.trash(&job.source_path).is_err() {
                    job.warnings.push("source_trash_failed".into());
                }
            } else if job.source_path.exists() {
                job.warnings.push("source_changed_not_trashed".into());
            }
        }
        self.save(&job)?;
        self.clean_cache(&job);
        self.notify(&job, true);
        Ok(())
    }
    fn rollback(&self, job: &Job) -> Vec<String> {
        let mut errors = Vec::new();
        for file in &job.planned {
            let target = installer::target(self.platform.as_ref(), file);
            if let Err(e) =
                installer::remove_owned(self.platform.as_ref(), &target, &file.hash, &file.faces)
            {
                errors.push(e);
            }
        }
        // Windows registrations may have been replaced before a failure; restore the previous mappings.
        let mut paths = HashSet::new();
        for old in &job.retired {
            if paths.insert(old.path.clone()) && old.path.exists() {
                let faces: Vec<_> = job
                    .retired
                    .iter()
                    .filter(|f| f.path == old.path)
                    .map(|f| f.face.clone())
                    .collect();
                if let Err(e) = self.platform.register(&old.path, &faces) {
                    errors.push(e);
                }
            }
        }
        let _ = self.platform.refresh();
        errors
    }
    fn fail_install(&self, job: &mut Job, error: String) -> Result<(), String> {
        let errors = self.rollback(job);
        job.status = "failed".into();
        job.error = Some(error.clone());
        job.updated_at = now();
        if errors.is_empty() {
            job.planned.clear();
            job.retired.clear();
        } else {
            job.warnings.push("rollback_incomplete".into());
        }
        self.save(job)?;
        Err(error)
    }
    fn retire_old(&self, job: &mut Job) {
        let Ok(current) = self.library.installed() else {
            job.warnings.push("old_font_trash_failed".into());
            return;
        };
        let mut seen = HashSet::new();
        let mut failed = Vec::new();
        for old in &job.retired {
            if !seen.insert(old.path.clone()) || current.iter().any(|f| f.path == old.path) {
                continue;
            }
            let faces: Vec<_> = job
                .retired
                .iter()
                .filter(|f| f.path == old.path)
                .map(|f| f.face.clone())
                .collect();
            if installer::remove_owned(self.platform.as_ref(), &old.path, &old.hash, &faces)
                .is_err()
            {
                failed.extend(job.retired.iter().filter(|f| f.path == old.path).cloned());
            }
        }
        job.retired = failed;
        if !job.retired.is_empty() {
            job.warnings.push("old_font_trash_failed".into());
        }
        let _ = self.platform.refresh();
    }
    fn recover(&mut self) -> Result<(), String> {
        for mut job in self.library.jobs()? {
            if job.status == "uninstalling" {
                if let Err(error) = self.resume_uninstall(&mut job) {
                    if job.status == "uninstalling" {
                        return Err(error);
                    }
                }
            } else if job.status == "installing"
                || (job.status == "failed" && !job.planned.is_empty())
            {
                let errors = self.rollback(&job);
                job.status = "failed".into();
                job.error = Some("interrupted_installation".into());
                if errors.is_empty() {
                    job.planned.clear();
                    job.retired.clear();
                } else {
                    job.warnings.push("rollback_incomplete".into());
                }
                self.library.save_job(&job)?;
            } else if job.status == "installed" && !job.retired.is_empty() {
                self.retire_old(&mut job);
                job.planned.clear();
                self.library.save_job(&job)?;
            }
        }
        Ok(())
    }
}
pub fn start(
    app: tauri::AppHandle,
    data: PathBuf,
    settings: Settings,
) -> Result<EngineHandle, String> {
    let mut engine = Engine::new(&data, settings.clone(), installer::native()?, Some(app))?;
    let (tx, rx) = mpsc::sync_channel(64);
    std::thread::Builder::new()
        .name("fontcatch-worker".into())
        .spawn(move || {
            // Folder baselining is filesystem work; never block the macOS UI thread.
            let mut watcher = match WatchService::new(settings.folders.clone()) {
                Ok(watcher) => watcher,
                Err(error) => {
                    engine.watcher_errors.push(format!("watch_error:{error}"));
                    engine.publish();
                    return;
                }
            };
            engine.watcher_errors = watcher.errors.clone();
            if let Err(error) = engine.reconcile_pending() {
                engine
                    .watcher_errors
                    .push(format!("processing_error:{error}"));
            }
            engine.publish();
            loop {
                match rx.recv_timeout(Duration::from_millis(250)) {
                    Ok(Message::Snapshot(reply)) => {
                        let _ = reply.send(engine.snapshot());
                    }
                    Ok(Message::Configure(settings, reply)) => {
                        let result = watcher.configure(settings.folders.clone());
                        let result = result.and_then(|_| engine.configure(settings));
                        if result.is_ok() {
                            engine.watcher_errors = watcher.errors.clone();
                            engine.publish();
                        }
                        let _ = reply.send(result.and_then(|_| engine.snapshot()));
                    }
                    Ok(Message::Install(id, confirm, reply)) => {
                        let result = engine.install_job(&id, confirm);
                        let result = result.and_then(|_| engine.reconcile_pending());
                        let _ = reply.send(result.and_then(|_| engine.snapshot()));
                    }
                    Ok(Message::Ignore(id, reply)) => {
                        let result = engine.ignore(&id);
                        let _ = reply.send(result.and_then(|_| engine.snapshot()));
                    }
                    Ok(Message::Preview(ps, reply)) => {
                        let _ = reply.send(engine.preview(&ps));
                    }
                    Ok(Message::Font(ps, reply)) => {
                        let _ = reply.send(engine.font(&ps));
                    }
                    Ok(Message::UninstallPlan(family, reply)) => {
                        let _ = reply.send(engine.uninstall_plan(&family));
                    }
                    Ok(Message::Uninstall(family, fingerprint, confirmed, reply)) => {
                        let _ = reply.send(engine.uninstall(&family, &fingerprint, confirmed));
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
                let ready = watcher.poll();
                if watcher.errors != engine.watcher_errors {
                    engine.watcher_errors = watcher.errors.clone();
                    engine.publish();
                }
                let received_files = !ready.is_empty();
                for path in ready {
                    if let Err(error) = engine.ingest(&path) {
                        engine.processing = None;
                        engine
                            .watcher_errors
                            .push(format!("processing_error:{error}"));
                        engine.publish();
                    }
                }
                if received_files {
                    if let Err(error) = engine.reconcile_pending() {
                        engine
                            .watcher_errors
                            .push(format!("processing_error:{error}"));
                        engine.publish();
                    }
                }
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(EngineHandle(tx))
}
#[cfg(test)]
mod tests {
    use super::*;
    use installer::test_platform::Sandbox;
    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }
    fn engine(root: &Path, mode: Mode) -> Engine {
        let settings = Settings {
            mode,
            ..Settings::default()
        };
        Engine::new(
            &root.join("data"),
            settings,
            Box::new(Sandbox {
                root: root.join("fonts"),
                fail_register: false,
            }),
            None,
        )
        .unwrap()
    }
    #[test]
    fn pending_survives_restart_and_original_deletion_then_installs() {
        let r = tempfile::tempdir().unwrap();
        let p = r.path().join("download.ttf");
        std::fs::copy(fixture("FontcatchQA-Regular.ttf"), &p).unwrap();
        let mut e = engine(r.path(), Mode::Ask);
        let id = e.ingest(&p).unwrap().unwrap();
        assert!(e.library.installed().unwrap().is_empty());
        std::fs::remove_file(&p).unwrap();
        drop(e);
        let mut e = engine(r.path(), Mode::Ask);
        assert_eq!(e.library.jobs().unwrap()[0].status, "pending");
        e.install_job(&id, false).unwrap();
        assert_eq!(e.library.installed().unwrap().len(), 1);
        assert!(e.library.installed().unwrap()[0].path.exists());
    }
    #[test]
    fn auto_installs_new_font_but_newer_version_always_waits() {
        let r = tempfile::tempdir().unwrap();
        let mut e = engine(r.path(), Mode::Auto);
        let first = e
            .ingest(&fixture("FontcatchQA-Regular.ttf"))
            .unwrap()
            .unwrap();
        assert_eq!(e.find(&first).unwrap().status, "installed");
        let update = e
            .ingest(&fixture("FontcatchQA-Newer.ttf"))
            .unwrap()
            .unwrap();
        assert_eq!(e.find(&update).unwrap().status, "pending");
        assert_eq!(
            e.install_job(&update, false).unwrap_err(),
            "update_confirmation_required"
        );
        e.install_job(&update, true).unwrap();
        assert_eq!(e.library.installed().unwrap().len(), 1);
        assert_eq!(e.library.installed().unwrap()[0].face.revision, 131072);
        assert_eq!(
            std::fs::read_dir(r.path().join("fonts/Trash"))
                .unwrap()
                .count(),
            1
        );
    }
    #[test]
    fn repeated_download_events_do_not_duplicate_jobs() {
        let r = tempfile::tempdir().unwrap();
        let mut e = engine(r.path(), Mode::Ask);
        let p = fixture("FontcatchQA-Regular.ttf");
        assert!(e.ingest(&p).unwrap().is_some());
        assert!(e.ingest(&p).unwrap().is_none());
        assert_eq!(e.library.jobs().unwrap().len(), 1);
    }
    #[test]
    fn switching_to_auto_processes_pending_once_and_preserves_ignored_choices() {
        let r = tempfile::tempdir().unwrap();
        let mut e = engine(r.path(), Mode::Ask);
        let first = e
            .ingest(&fixture("FontcatchQA-Regular.ttf"))
            .unwrap()
            .unwrap();
        let duplicate = r.path().join("copy.ttf");
        std::fs::copy(fixture("FontcatchQA-Regular.ttf"), &duplicate).unwrap();
        let second = e.ingest(&duplicate).unwrap().unwrap();
        let ignored = e
            .ingest(&fixture("FontcatchOther-Regular.ttf"))
            .unwrap()
            .unwrap();
        e.ignore(&ignored).unwrap();
        let mut settings = e.settings.clone();
        settings.mode = Mode::Auto;
        e.configure(settings).unwrap();
        let mut statuses = [
            e.find(&first).unwrap().status,
            e.find(&second).unwrap().status,
        ];
        statuses.sort();
        assert_eq!(statuses, ["ignored", "installed"]);
        assert_eq!(e.find(&ignored).unwrap().status, "ignored");
        assert_eq!(e.library.installed().unwrap().len(), 1);
        assert!(!e.cache.join(first).exists());
        assert!(!e.cache.join(second).exists());
    }
    #[test]
    fn asking_mode_clears_obsolete_duplicates_but_keeps_new_fonts_pending() {
        let r = tempfile::tempdir().unwrap();
        let mut e = engine(r.path(), Mode::Ask);
        let first = e
            .ingest(&fixture("FontcatchQA-Regular.ttf"))
            .unwrap()
            .unwrap();
        let duplicate = r.path().join("copy.ttf");
        std::fs::copy(fixture("FontcatchQA-Regular.ttf"), &duplicate).unwrap();
        let second = e.ingest(&duplicate).unwrap().unwrap();
        let other = e
            .ingest(&fixture("FontcatchOther-Regular.ttf"))
            .unwrap()
            .unwrap();
        e.install_job(&first, false).unwrap();
        e.reconcile_pending().unwrap();
        assert_eq!(e.find(&second).unwrap().status, "ignored");
        assert!(e
            .find(&second)
            .unwrap()
            .warnings
            .contains(&"already_installed".into()));
        assert_eq!(e.find(&other).unwrap().status, "pending");
        assert_eq!(e.library.installed().unwrap().len(), 1);
    }
    #[test]
    fn auto_resume_processes_saved_requests_but_never_approves_updates() {
        let r = tempfile::tempdir().unwrap();
        let mut e = engine(r.path(), Mode::Auto);
        e.ingest(&fixture("FontcatchQA-Regular.ttf")).unwrap();
        e.settings.mode = Mode::Ask;
        let update = e
            .ingest(&fixture("FontcatchQA-Newer.ttf"))
            .unwrap()
            .unwrap();
        let other = e
            .ingest(&fixture("FontcatchOther-Regular.ttf"))
            .unwrap()
            .unwrap();
        drop(e);
        let mut e = engine(r.path(), Mode::Auto);
        e.reconcile_pending().unwrap();
        assert_eq!(e.find(&update).unwrap().status, "pending");
        assert!(e.find(&update).unwrap().requires_update);
        assert_eq!(e.find(&other).unwrap().status, "installed");
        assert!(e
            .library
            .installed()
            .unwrap()
            .iter()
            .all(|f| f.face.revision == 65536));
        assert_eq!(e.library.installed().unwrap().len(), 2);
    }
    #[test]
    fn a_failed_pending_install_does_not_block_the_rest_of_the_auto_queue() {
        let r = tempfile::tempdir().unwrap();
        let mut e = engine(r.path(), Mode::Ask);
        let missing = e
            .ingest(&fixture("FontcatchQA-Regular.ttf"))
            .unwrap()
            .unwrap();
        let other = e
            .ingest(&fixture("FontcatchOther-Regular.ttf"))
            .unwrap()
            .unwrap();
        std::fs::remove_file(&e.find(&missing).unwrap().files[0].path).unwrap();
        let mut settings = e.settings.clone();
        settings.mode = Mode::Auto;
        e.configure(settings).unwrap();
        assert_eq!(e.find(&missing).unwrap().status, "failed");
        assert_eq!(e.find(&other).unwrap().status, "installed");
        assert_eq!(e.library.installed().unwrap().len(), 1);
    }
    #[test]
    fn ignored_fonts_never_install_and_cached_files_are_released() {
        let r = tempfile::tempdir().unwrap();
        let mut e = engine(r.path(), Mode::Ask);
        let id = e
            .ingest(&fixture("FontcatchQA-Regular.ttf"))
            .unwrap()
            .unwrap();
        e.ignore(&id).unwrap();
        assert!(e.library.installed().unwrap().is_empty());
        assert!(!e.cache.join(id).exists());
    }
    #[test]
    fn previews_only_serve_unchanged_managed_fonts() {
        let r = tempfile::tempdir().unwrap();
        let mut e = engine(r.path(), Mode::Auto);
        e.ingest(&fixture("FontcatchQA-Regular.ttf")).unwrap();
        let font = e.library.installed().unwrap().remove(0);
        assert_eq!(
            e.preview(&font.face.postscript).unwrap(),
            std::fs::read(fixture("FontcatchQA-Regular.ttf")).unwrap()
        );
        assert_eq!(e.preview("../../secret").unwrap_err(), "font_not_found");
        std::fs::write(&font.path, b"user modified").unwrap();
        assert_eq!(
            e.preview(&font.face.postscript).unwrap_err(),
            "installed_font_changed"
        );
    }
    #[test]
    fn uninstall_requires_confirmation_and_a_current_plan() {
        let r = tempfile::tempdir().unwrap();
        let mut e = engine(r.path(), Mode::Auto);
        e.ingest(&fixture("FontcatchQA-Regular.ttf")).unwrap();
        let plan = e.uninstall_plan("Fontcatch QA").unwrap();
        assert!(e.uninstall(&plan.family, &plan.fingerprint, false).is_err());
        e.ingest(&fixture("FontcatchQA-Variable.ttf")).unwrap();
        assert!(e.uninstall(&plan.family, &plan.fingerprint, true).is_err());
        assert_eq!(e.library.installed().unwrap().len(), 2);
        assert!(e
            .library
            .installed()
            .unwrap()
            .iter()
            .all(|f| f.path.exists()));
    }
    #[test]
    fn uninstall_trashes_files_and_cancels_pending_updates_without_reinstalling() {
        let r = tempfile::tempdir().unwrap();
        let mut e = engine(r.path(), Mode::Auto);
        e.ingest(&fixture("FontcatchQA-Regular.ttf")).unwrap();
        let update = e
            .ingest(&fixture("FontcatchQA-Newer.ttf"))
            .unwrap()
            .unwrap();
        let plan = e.uninstall_plan("Fontcatch QA").unwrap();
        let outcome = e.uninstall(&plan.family, &plan.fingerprint, true).unwrap();
        assert!(outcome.error.is_none());
        assert_eq!(outcome.removed_count, 1);
        assert!(e.library.installed().unwrap().is_empty());
        assert_eq!(e.find(&update).unwrap().status, "ignored");
        assert!(!e.cache.join(update).exists());
        assert!(fixture("FontcatchQA-Regular.ttf").exists());
        assert_eq!(
            std::fs::read_dir(r.path().join("fonts/Trash"))
                .unwrap()
                .count(),
            1
        );
        drop(e);
        let mut e = engine(r.path(), Mode::Auto);
        e.reconcile_pending().unwrap();
        assert!(e.library.installed().unwrap().is_empty());
    }
    #[test]
    fn shared_collection_lists_and_removes_all_affected_families_once() {
        let r = tempfile::tempdir().unwrap();
        let mut e = engine(r.path(), Mode::Auto);
        e.ingest(&fixture("FontcatchQA-Collection.ttc")).unwrap();
        let plan = e.uninstall_plan("Fontcatch QA").unwrap();
        assert_eq!(plan.families, ["Fontcatch Other", "Fontcatch QA"]);
        assert_eq!(plan.font_count, 2);
        assert_eq!(plan.file_count, 1);
        for font in e.library.installed().unwrap() {
            assert!(crate::inspector::validated_font(
                &e.preview(&font.face.postscript).unwrap(),
                0
            )
            .is_ok());
        }
        let outcome = e.uninstall(&plan.family, &plan.fingerprint, true).unwrap();
        assert!(outcome.error.is_none());
        assert_eq!(outcome.removed_count, 2);
        assert!(outcome.snapshot.installed.is_empty());
        assert_eq!(
            std::fs::read_dir(r.path().join("fonts/Trash"))
                .unwrap()
                .count(),
            1
        );
    }
    #[test]
    fn partial_uninstall_preserves_changed_files_and_keeps_database_accurate() {
        let r = tempfile::tempdir().unwrap();
        let mut e = engine(r.path(), Mode::Auto);
        e.ingest(&fixture("FontcatchQA-Regular.ttf")).unwrap();
        e.ingest(&fixture("FontcatchQA-Variable.ttf")).unwrap();
        let variable = e
            .library
            .installed()
            .unwrap()
            .into_iter()
            .find(|f| f.face.variable)
            .unwrap();
        std::fs::write(&variable.path, b"user modified font").unwrap();
        let plan = e.uninstall_plan("Fontcatch QA").unwrap();
        let outcome = e.uninstall(&plan.family, &plan.fingerprint, true).unwrap();
        assert_eq!(outcome.error.as_deref(), Some("installed_font_changed"));
        assert_eq!(outcome.removed_count, 1);
        assert_eq!(outcome.snapshot.installed.len(), 1);
        assert_eq!(std::fs::read(variable.path).unwrap(), b"user modified font");
        assert!(outcome
            .snapshot
            .recent
            .iter()
            .any(|j| j.status == "uninstall_failed"));
    }
    #[test]
    fn interrupted_uninstall_reconciles_a_file_already_moved_to_trash() {
        let r = tempfile::tempdir().unwrap();
        let mut e = engine(r.path(), Mode::Auto);
        e.ingest(&fixture("FontcatchQA-Regular.ttf")).unwrap();
        let fonts = e.library.installed().unwrap();
        let mut job = e.empty_job(Path::new("Fontcatch QA"), "confirmed-plan".into());
        job.status = "uninstalling".into();
        job.retired = fonts.clone();
        e.library.save_job(&job).unwrap();
        installer::remove_owned(
            e.platform.as_ref(),
            &fonts[0].path,
            &fonts[0].hash,
            &[fonts[0].face.clone()],
        )
        .unwrap();
        drop(e);
        let e = engine(r.path(), Mode::Ask);
        assert!(e.library.installed().unwrap().is_empty());
        assert_eq!(e.find(&job.id).unwrap().status, "uninstalled");
        assert_eq!(e.find(&job.id).unwrap().removed_count, 1);
    }
    #[test]
    fn registration_failure_rolls_back_new_files_and_keeps_metadata_empty() {
        let r = tempfile::tempdir().unwrap();
        let mut e = engine(r.path(), Mode::Ask);
        let id = e
            .ingest(&fixture("FontcatchQA-Regular.ttf"))
            .unwrap()
            .unwrap();
        e.platform = Box::new(Sandbox {
            root: r.path().join("fonts"),
            fail_register: true,
        });
        assert!(e.install_job(&id, false).is_err());
        assert!(e.library.installed().unwrap().is_empty());
        assert_eq!(e.find(&id).unwrap().status, "failed");
        assert_eq!(
            std::fs::read_dir(r.path().join("fonts/Trash"))
                .unwrap()
                .count(),
            1
        );
    }
    #[test]
    fn crashed_installation_is_rolled_back_from_journal() {
        let r = tempfile::tempdir().unwrap();
        let mut e = engine(r.path(), Mode::Ask);
        let id = e
            .ingest(&fixture("FontcatchQA-Regular.ttf"))
            .unwrap()
            .unwrap();
        let mut job = e.find(&id).unwrap();
        job.status = "installing".into();
        job.planned = job.files.clone();
        e.library.save_job(&job).unwrap();
        installer::install(e.platform.as_ref(), &job.files[0]).unwrap();
        drop(e);
        let e = engine(r.path(), Mode::Ask);
        assert_eq!(e.find(&id).unwrap().status, "failed");
        assert!(e.library.installed().unwrap().is_empty());
        assert_eq!(
            std::fs::read_dir(r.path().join("fonts/Trash"))
                .unwrap()
                .count(),
            1
        );
    }
}

#[cfg(test)]
mod archive_tests {
    use super::*;
    use installer::test_platform::Sandbox;
    use std::io::Write;
    use zip::{write::SimpleFileOptions, ZipWriter};
    fn archive(root: &Path, name: &str, entries: &[(&str, Vec<u8>)]) -> PathBuf {
        let path = root.join(name);
        let mut zip = ZipWriter::new(std::fs::File::create(&path).unwrap());
        for (n, bytes) in entries {
            zip.start_file(*n, SimpleFileOptions::default()).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        path
    }
    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name),
        )
        .unwrap()
    }
    fn engine(root: &Path) -> Engine {
        let settings = Settings {
            mode: Mode::Ask,
            trash_after_install: true,
            ..Settings::default()
        };
        Engine::new(
            &root.join("data"),
            settings,
            Box::new(Sandbox {
                root: root.join("fonts"),
                fail_register: false,
            }),
            None,
        )
        .unwrap()
    }
    #[test]
    fn empty_zip_has_no_history_but_web_fonts_are_reported() {
        let r = tempfile::tempdir().unwrap();
        let mut e = engine(r.path());
        let p = archive(r.path(), "empty.zip", &[("readme.md", b"hi".to_vec())]);
        assert!(e.ingest(&p).unwrap().is_none());
        assert!(e.library.jobs().unwrap().is_empty());
        let p = archive(r.path(), "web.zip", &[("web.woff2", b"web".to_vec())]);
        let id = e.ingest(&p).unwrap().unwrap();
        assert_eq!(e.find(&id).unwrap().warnings, vec!["web_fonts_only"]);
    }
    #[test]
    fn variable_static_zip_installs_only_variable_and_trashes_source() {
        let r = tempfile::tempdir().unwrap();
        let mut e = engine(r.path());
        let p = archive(
            r.path(),
            "mixed.zip",
            &[
                ("Variable.ttf", fixture("FontcatchQA-Variable.ttf")),
                ("static/Regular.ttf", fixture("FontcatchQA-Regular.ttf")),
            ],
        );
        let id = e.ingest(&p).unwrap().unwrap();
        let job = e.find(&id).unwrap();
        assert_eq!(job.files.len(), 1);
        assert!(job.files[0].faces[0].variable);
        e.install_job(&id, false).unwrap();
        assert!(!p.exists());
        assert!(r.path().join("fonts/Trash/mixed.zip").exists());
    }
    #[test]
    fn changed_original_zip_is_never_trashed() {
        let r = tempfile::tempdir().unwrap();
        let mut e = engine(r.path());
        let p = archive(
            r.path(),
            "font.zip",
            &[("font.ttf", fixture("FontcatchQA-Regular.ttf"))],
        );
        let id = e.ingest(&p).unwrap().unwrap();
        std::fs::write(&p, b"user replacement").unwrap();
        e.install_job(&id, false).unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"user replacement");
        assert!(e
            .find(&id)
            .unwrap()
            .warnings
            .contains(&"source_changed_not_trashed".into()));
    }
    #[test]
    fn unsafe_archive_cannot_install_or_escape() {
        let r = tempfile::tempdir().unwrap();
        let mut e = engine(r.path());
        let p = archive(
            r.path(),
            "evil.zip",
            &[("../../../outside.ttf", fixture("FontcatchQA-Regular.ttf"))],
        );
        let id = e.ingest(&p).unwrap().unwrap();
        assert_eq!(e.find(&id).unwrap().status, "failed");
        assert_eq!(
            e.find(&id).unwrap().error.as_deref(),
            Some("unsafe_archive_path")
        );
        assert!(e.library.installed().unwrap().is_empty());
    }
}

#[cfg(all(test, target_os = "macos"))]
mod macos_smoke {
    use super::*;
    #[test]
    #[ignore = "Installs one original synthetic test font in the current user's Fonts folder, then moves it to Trash."]
    fn macos_native_install_and_trash() {
        assert_eq!(std::env::var("FONTCATCH_NATIVE_SMOKE").as_deref(), Ok("1"));
        let root = tempfile::tempdir().unwrap();
        let mut e = Engine::new(
            root.path(),
            Settings {
                mode: Mode::Ask,
                ..Settings::default()
            },
            installer::native().unwrap(),
            None,
        )
        .unwrap();
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/FontcatchQA-Regular.ttf");
        let id = e.ingest(&source).unwrap().unwrap();
        assert_eq!(
            e.find(&id).unwrap().status,
            "pending",
            "A test font already exists; do not modify it"
        );
        e.install_job(&id, false).unwrap();
        let installed = e.library.installed().unwrap();
        assert_eq!(installed.len(), 1);
        let font = &installed[0];
        assert_eq!(font.face.family, "Fontcatch QA");
        assert!(font
            .path
            .starts_with(dirs::home_dir().unwrap().join("Library/Fonts")));
        assert_eq!(
            extractor::hash_file(&font.path, extractor::MAX_FILE).unwrap(),
            font.hash
        );
        println!("Native user-font install verified: {}", font.path.display());
        assert!(
            crate::inspector::validated_font(&e.preview(&font.face.postscript).unwrap(), 0).is_ok()
        );
        let plan = e.uninstall_plan(&font.face.family).unwrap();
        let outcome = e.uninstall(&plan.family, &plan.fingerprint, true).unwrap();
        assert!(outcome.error.is_none());
        assert!(outcome.snapshot.installed.is_empty());
        assert!(!font.path.exists());
        println!("Synthetic test font moved to macOS Trash; source fixture preserved.");
    }
}
