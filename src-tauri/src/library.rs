use crate::model::{InstalledFont, Job};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
pub struct Library {
    pub path: PathBuf,
}
pub fn initialize(path: &Path) -> rusqlite::Result<()> {
    Library::new(path).map(|_| ())
}
impl Library {
    pub fn new(path: &Path) -> rusqlite::Result<Self> {
        let s = Self { path: path.into() };
        let c = s.connection()?;
        migrate(&c)?;
        Ok(s)
    }
    fn connection(&self) -> rusqlite::Result<Connection> {
        let c = Connection::open(&self.path)?;
        c.busy_timeout(std::time::Duration::from_secs(5))?;
        c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;")?;
        Ok(c)
    }
    pub fn jobs(&self) -> Result<Vec<Job>, String> {
        let c = self.connection().map_err(|e| e.to_string())?;
        let mut q = c
            .prepare("SELECT data FROM catch_jobs ORDER BY created_at DESC")
            .map_err(|e| e.to_string())?;
        let rows = q
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.map(|r| {
            serde_json::from_str(&r.map_err(|e| e.to_string())?).map_err(|e| e.to_string())
        })
        .collect()
    }
    pub fn recent(&self) -> Result<Vec<Job>, String> {
        let c = self.connection().map_err(|e| e.to_string())?;
        let mut q = c.prepare("SELECT data FROM catch_jobs WHERE id IN (SELECT id FROM catch_jobs ORDER BY created_at DESC LIMIT 500) OR json_extract(data,'$.status')='pending' ORDER BY created_at DESC").map_err(|e| e.to_string())?;
        let rows = q
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.map(|r| {
            serde_json::from_str(&r.map_err(|e| e.to_string())?).map_err(|e| e.to_string())
        })
        .collect()
    }
    pub fn save_job(&self, job: &Job) -> Result<(), String> {
        let c = self.connection().map_err(|e| e.to_string())?;
        write_job(&c, job).map_err(|e| e.to_string())
    }
    pub fn job(&self, id: &str) -> Result<Job, String> {
        let data: String = self
            .connection()
            .map_err(|e| e.to_string())?
            .query_row("SELECT data FROM catch_jobs WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .map_err(|_| "job_not_found")?;
        serde_json::from_str(&data).map_err(|e| e.to_string())
    }
    pub fn pending(&self) -> Result<Vec<Job>, String> {
        let c = self.connection().map_err(|e| e.to_string())?;
        let mut q = c.prepare("SELECT data FROM catch_jobs WHERE json_extract(data,'$.status')='pending' ORDER BY created_at DESC").map_err(|e| e.to_string())?;
        let rows = q
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.map(|r| {
            serde_json::from_str(&r.map_err(|e| e.to_string())?).map_err(|e| e.to_string())
        })
        .collect()
    }
    pub fn seen(&self, path: &Path, hash: &str) -> Result<bool, String> {
        let c = self.connection().map_err(|e| e.to_string())?;
        c.query_row(
            "SELECT EXISTS(SELECT 1 FROM processed_downloads WHERE path=?1 AND hash=?2)",
            params![path.to_string_lossy(), hash],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())
    }
    pub fn mark_seen(&self, path: &Path, hash: &str) -> Result<(), String> {
        self.connection()
            .map_err(|e| e.to_string())?
            .execute(
                "INSERT OR IGNORE INTO processed_downloads(path,hash)VALUES(?1,?2)",
                params![path.to_string_lossy(), hash],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn installed(&self) -> Result<Vec<InstalledFont>, String> {
        let c = self.connection().map_err(|e| e.to_string())?;
        let mut q = c
            .prepare("SELECT data FROM managed_fonts ORDER BY installed_at DESC")
            .map_err(|e| e.to_string())?;
        let rows = q
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.map(|r| {
            serde_json::from_str(&r.map_err(|e| e.to_string())?).map_err(|e| e.to_string())
        })
        .collect()
    }
    pub fn commit_install(&self, fonts: &[InstalledFont], job: &Job) -> Result<(), String> {
        let mut c = self.connection().map_err(|e| e.to_string())?;
        let tx = c.transaction().map_err(|e| e.to_string())?;
        for font in fonts {
            tx.execute("INSERT INTO managed_fonts(postscript,path,installed_at,data)VALUES(?1,?2,?3,?4) ON CONFLICT(postscript)DO UPDATE SET path=excluded.path,installed_at=excluded.installed_at,data=excluded.data",params![font.face.postscript,font.path.to_string_lossy(),font.installed_at,serde_json::to_string(font).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
        }
        write_job(&tx, job).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }
    pub fn commit_uninstall(&self, fonts: &[InstalledFont], job: &Job) -> Result<(), String> {
        let mut c = self.connection().map_err(|e| e.to_string())?;
        let tx = c.transaction().map_err(|e| e.to_string())?;
        for font in fonts {
            tx.execute("DELETE FROM managed_fonts WHERE postscript=?1 AND path=?2 AND json_extract(data,'$.hash')=?3", params![font.face.postscript, font.path.to_string_lossy(), font.hash]).map_err(|e| e.to_string())?;
        }
        // A queued update must not reinstall a family that the user just removed.
        // Keep unrelated files from multi-family archives available for approval.
        let jobs: Vec<String> = {
            let mut q = tx
                .prepare(
                    "SELECT data FROM catch_jobs WHERE json_extract(data,'$.status')='pending'",
                )
                .map_err(|e| e.to_string())?;
            let rows = q.query_map([], |r| r.get(0)).map_err(|e| e.to_string())?;
            rows.collect::<rusqlite::Result<_>>()
                .map_err(|e| e.to_string())?
        };
        for data in jobs {
            let mut pending: Job = serde_json::from_str(&data).map_err(|e| e.to_string())?;
            let before = pending.files.len();
            pending.files.retain(|file| {
                !file
                    .faces
                    .iter()
                    .any(|face| fonts.iter().any(|f| f.face.postscript == face.postscript))
            });
            if pending.files.len() != before {
                if pending.files.is_empty() {
                    pending.status = "ignored".into();
                }
                pending.warnings.push("removed_pending_cancelled".into());
                pending.updated_at = crate::model::now();
                write_job(&tx, &pending).map_err(|e| e.to_string())?;
            }
        }
        write_job(&tx, job).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }
}
fn write_job(c: &Connection, job: &Job) -> rusqlite::Result<()> {
    let mut stored = job.clone();
    if matches!(
        stored.status.as_str(),
        "installed" | "ignored" | "uninstalled"
    ) {
        for file in &mut stored.files {
            for face in &mut file.faces {
                face.license = None;
                face.license_url = None;
            }
        }
    }
    let data = serde_json::to_string(&stored)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    c.execute("INSERT INTO catch_jobs(id,created_at,data)VALUES(?1,?2,?3)ON CONFLICT(id)DO UPDATE SET data=excluded.data",params![job.id,job.created_at,data])?;
    Ok(())
}
fn migrate(c: &Connection) -> rusqlite::Result<()> {
    let version: i32 = c.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    c.execute_batch("PRAGMA foreign_keys=ON;
 BEGIN;
 CREATE TABLE IF NOT EXISTS families(id TEXT PRIMARY KEY,name TEXT NOT NULL,foundry TEXT,installed_at TEXT,source_archive TEXT,source_url TEXT,license_text TEXT,license_url TEXT);
 CREATE TABLE IF NOT EXISTS fonts(id TEXT PRIMARY KEY,family_id TEXT NOT NULL REFERENCES families(id),postscript_name TEXT NOT NULL UNIQUE,style TEXT NOT NULL,version TEXT,is_variable INTEGER NOT NULL CHECK(is_variable IN(0,1)),installed_path TEXT NOT NULL,face_index INTEGER NOT NULL DEFAULT 0);
 CREATE TABLE IF NOT EXISTS catch_jobs(id TEXT PRIMARY KEY,created_at INTEGER NOT NULL,data TEXT NOT NULL);
 CREATE TABLE IF NOT EXISTS processed_downloads(path TEXT NOT NULL,hash TEXT NOT NULL,PRIMARY KEY(path,hash));
 CREATE TABLE IF NOT EXISTS managed_fonts(postscript TEXT PRIMARY KEY,path TEXT NOT NULL,installed_at INTEGER NOT NULL,data TEXT NOT NULL);
 CREATE INDEX IF NOT EXISTS jobs_created ON catch_jobs(created_at DESC);
 CREATE INDEX IF NOT EXISTS jobs_status ON catch_jobs(json_extract(data,'$.status'));
 COMMIT;")?;
    if version < 3 {
        let tx = c.unchecked_transaction()?;
        for table in ["catch_jobs", "managed_fonts"] {
            let entries = {
                let mut q = tx.prepare(&format!("SELECT rowid,data FROM {table}"))?;
                let rows =
                    q.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
                rows.collect::<rusqlite::Result<Vec<_>>>()?
            };
            for (id, data) in entries {
                let mut value: serde_json::Value = serde_json::from_str(&data)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                if let Some(source) = value.get("sourceUrl").and_then(|v| v.as_str()) {
                    let clean = crate::origin::clean(source);
                    if clean.as_deref() != Some(source) {
                        value["sourceUrl"] = clean
                            .map(serde_json::Value::String)
                            .unwrap_or(serde_json::Value::Null);
                        tx.execute(
                            &format!("UPDATE {table} SET data=?1 WHERE rowid=?2"),
                            params![value.to_string(), id],
                        )?;
                    }
                }
            }
        }
        tx.execute_batch("PRAGMA user_version=3;")?;
        tx.commit()?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn job(id: &str, created_at: u64, status: &str) -> Job {
        let file = crate::inspector::inspect(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/FontcatchQA-Regular.ttf"),
        )
        .unwrap();
        Job {
            id: id.into(),
            source_path: "test.zip".into(),
            source_hash: "hash".into(),
            source_url: None,
            created_at,
            updated_at: created_at,
            status: status.into(),
            files: vec![file],
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
    #[test]
    fn privacy_migration_preserves_metadata_and_removes_url_secrets() {
        let c = Connection::open_in_memory().unwrap();
        migrate(&c).unwrap();
        let mut pending = job("pending", 1, "pending");
        pending.source_url =
            Some("https://user:secret@example.org/font?token=secret#private".into());
        write_job(&c, &pending).unwrap();
        let font = InstalledFont {
            face: pending.files[0].faces[0].clone(),
            path: "/fonts/test.ttf".into(),
            hash: "sha256".into(),
            installed_at: 2,
            source_archive: "test.zip".into(),
            source_url: pending.source_url.clone(),
        };
        c.execute(
            "INSERT INTO managed_fonts VALUES(?1,?2,?3,?4)",
            params![
                font.face.postscript,
                font.path.to_str(),
                font.installed_at,
                serde_json::to_string(&font).unwrap()
            ],
        )
        .unwrap();
        c.execute_batch("PRAGMA user_version=2").unwrap();
        migrate(&c).unwrap();
        for table in ["catch_jobs", "managed_fonts"] {
            let raw: String = c
                .query_row(&format!("SELECT data FROM {table}"), [], |r| r.get(0))
                .unwrap();
            assert!(!raw.contains("secret"));
            let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
            assert_eq!(value["sourceUrl"], "https://example.org/font");
            if table == "catch_jobs" {
                let migrated: Job = serde_json::from_str(&raw).unwrap();
                assert_eq!(
                    migrated.files[0].faces[0].license,
                    pending.files[0].faces[0].license
                );
                assert_eq!(migrated.status, "pending");
            } else {
                let migrated: InstalledFont = serde_json::from_str(&raw).unwrap();
                assert_eq!(migrated.face.license, font.face.license);
                assert_eq!(migrated.path, font.path);
                assert_eq!(migrated.hash, font.hash);
            }
        }
        assert_eq!(
            c.query_row("PRAGMA user_version", [], |r| r.get::<_, i32>(0))
                .unwrap(),
            3
        );
    }
    #[test]
    fn recent_is_bounded_without_hiding_old_pending_jobs() {
        let root = tempfile::tempdir().unwrap();
        let library = Library::new(&root.path().join("library.sqlite")).unwrap();
        let mut c = library.connection().unwrap();
        let tx = c.transaction().unwrap();
        let mut history = job("old-pending", 0, "pending");
        write_job(&tx, &history).unwrap();
        history.status = "installed".into();
        for index in 1..=510 {
            history.id = index.to_string();
            history.created_at = index;
            write_job(&tx, &history).unwrap();
        }
        tx.commit().unwrap();
        let recent = library.recent().unwrap();
        assert_eq!(recent.len(), 501);
        assert_eq!(recent[0].id, "510");
        assert_eq!(recent.last().unwrap().id, "old-pending");
        assert!(library.job("1").is_ok());
        assert_eq!(library.pending().unwrap().len(), 1);
    }
    #[test]
    fn completed_history_is_compact_but_installed_licenses_are_preserved() {
        let root = tempfile::tempdir().unwrap();
        let library = Library::new(&root.path().join("library.sqlite")).unwrap();
        let job = job("installed", 1, "installed");
        let font = InstalledFont {
            face: job.files[0].faces[0].clone(),
            path: "/fonts/test.ttf".into(),
            hash: "sha256".into(),
            installed_at: 2,
            source_archive: "test.zip".into(),
            source_url: None,
        };
        library
            .commit_install(std::slice::from_ref(&font), &job)
            .unwrap();
        assert!(library.job(&job.id).unwrap().files[0].faces[0]
            .license
            .is_none());
        let installed = library.installed().unwrap();
        assert!(installed[0].face.license.is_some());
        assert_eq!(installed[0].face.license, font.face.license);
        assert_eq!(installed[0].face.license_url, font.face.license_url);
    }
    #[test]
    fn migration_is_idempotent() {
        let c = Connection::open_in_memory().unwrap();
        migrate(&c).unwrap();
        migrate(&c).unwrap();
        assert_eq!(
            c.query_row("SELECT count(*) FROM families", [], |r| r.get::<_, i32>(0))
                .unwrap(),
            0
        );
    }
    #[test]
    fn orphaned_fonts_are_rejected() {
        let c = Connection::open_in_memory().unwrap();
        migrate(&c).unwrap();
        assert!(c.execute("INSERT INTO fonts VALUES('a','missing','Inter-Regular','Regular','1',0,'/tmp/inter.ttf',0)",[]).is_err());
    }
    #[test]
    fn source_seen_is_persistent_and_content_specific() {
        let r = tempfile::tempdir().unwrap();
        let db = r.path().join("test.sqlite");
        let s = Library::new(&db).unwrap();
        s.mark_seen(Path::new("a.zip"), "hash1").unwrap();
        drop(s);
        let s = Library::new(&db).unwrap();
        assert!(s.seen(Path::new("a.zip"), "hash1").unwrap());
        assert!(!s.seen(Path::new("a.zip"), "hash2").unwrap());
    }
}
