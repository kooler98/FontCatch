use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FontFace {
    pub family: String,
    pub style: String,
    pub postscript: String,
    pub version: String,
    pub revision: u32,
    pub foundry: String,
    pub designer: String,
    pub license: Option<String>,
    pub license_url: Option<String>,
    pub variable: bool,
    pub face_index: u32,
    pub axes: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FontFile {
    pub path: PathBuf,
    pub hash: String,
    pub faces: Vec<FontFace>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub source_path: PathBuf,
    pub source_hash: String,
    pub source_url: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub status: String,
    pub files: Vec<FontFile>,
    pub warnings: Vec<String>,
    pub error: Option<String>,
    pub requires_update: bool,
    pub installed_count: usize,
    pub skipped_count: usize,
    #[serde(default)]
    pub removed_count: usize,
    #[serde(default)]
    pub planned: Vec<FontFile>,
    #[serde(default)]
    pub retired: Vec<InstalledFont>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledFont {
    pub face: FontFace,
    pub path: PathBuf,
    pub hash: String,
    pub installed_at: u64,
    pub source_archive: String,
    pub source_url: Option<String>,
}
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub recent: Vec<Job>,
    pub installed: Vec<InstalledFont>,
    pub processing: Option<String>,
    pub watcher_errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallPlan {
    pub family: String,
    pub fingerprint: String,
    pub families: Vec<String>,
    pub font_count: usize,
    pub file_count: usize,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallOutcome {
    pub snapshot: Snapshot,
    pub removed_count: usize,
    pub error: Option<String>,
}
