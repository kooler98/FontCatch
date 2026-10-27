use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
};
use tempfile::TempDir;

pub const MAX_FILE: u64 = 64 * 1024 * 1024;
pub const MAX_ARCHIVE: u64 = 256 * 1024 * 1024;
const MAX_ENTRIES: usize = 10_000;
const MAX_FONTS: usize = 512;
const MAX_DIRECTORY: u64 = 16 * 1024 * 1024;
pub fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}
pub fn is_font(path: &Path) -> bool {
    matches!(extension(path).as_str(), "ttf" | "otf" | "ttc")
}
pub fn is_download(path: &Path) -> bool {
    if path.components().any(|c| {
        let part = Path::new(c.as_os_str());
        matches!(
            extension(part).as_str(),
            "crdownload" | "download" | "part" | "tmp"
        ) || c.as_os_str() == "__MACOSX"
            || c.as_os_str().to_string_lossy().starts_with("._")
    }) {
        return false;
    }
    is_font(path) || extension(path) == "zip"
}
fn metadata_file(path: &Path) -> bool {
    path.components()
        .any(|c| c.as_os_str() == "__MACOSX" || c.as_os_str().to_string_lossy().starts_with("._"))
}
pub fn hash_file(path: &Path, limit: u64) -> Result<String, String> {
    let mut reader = crate::files::open_regular(path, limit)?.take(limit + 1);
    let mut digest = Sha256::new();
    let mut size = 0;
    let mut buffer = [0u8; 65536];
    loop {
        let n = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        size += n as u64;
        if size > limit {
            return Err("file_limit".into());
        }
        digest.update(&buffer[..n]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
fn safe_name(name: &str) -> Result<PathBuf, String> {
    // Reject Windows separators/streams on every host so archives stay portable.
    if name.len() > 1024 || name.split('/').count() > 16 || name.contains(['\\', ':', '\0']) {
        return Err("unsafe_archive_path".into());
    }
    let path = PathBuf::from(name);
    if path.is_absolute()
        || path.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err("unsafe_archive_path".into());
    }
    Ok(path)
}
pub struct Extracted {
    pub dir: TempDir,
    pub files: Vec<PathBuf>,
    pub web_only: bool,
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

// ZipArchive allocates its metadata before exposing len(), and duplicate names are
// collapsed by the library. Check the raw directory without allocating per entry.
fn preflight_zip(reader: &mut (impl Read + Seek)) -> Result<(), String> {
    let file_len = reader
        .seek(SeekFrom::End(0))
        .map_err(|_| "invalid_archive")?;
    if file_len > MAX_ARCHIVE {
        return Err("archive_limit".into());
    }
    let tail_len = file_len.min(22 + u16::MAX as u64) as usize;
    let tail_start = file_len - tail_len as u64;
    reader
        .seek(SeekFrom::Start(tail_start))
        .map_err(|_| "invalid_archive")?;
    let mut tail = vec![0; tail_len];
    reader
        .read_exact(&mut tail)
        .map_err(|_| "invalid_archive")?;
    // Use the last signature, just as the ZIP reader does. Reject ambiguous end
    // records or trailing data instead of allowing a fallback to another directory.
    let end = tail
        .windows(4)
        .rposition(|bytes| bytes == b"PK\x05\x06")
        .ok_or("invalid_archive")?;
    if tail.len() - end < 22 || end + 22 + u16_at(&tail, end + 20) as usize != tail.len() {
        return Err("invalid_archive".into());
    }
    let eocd = &tail[end..end + 22];
    let count = u16_at(eocd, 10);
    let directory_size = u32_at(eocd, 12);
    let directory_start = u32_at(eocd, 16);
    // ZIP64 and split archives are unnecessary within our file/count limits.
    if u16_at(eocd, 4) != 0
        || u16_at(eocd, 6) != 0
        || u16_at(eocd, 8) != count
        || count == u16::MAX
        || directory_size == u32::MAX
        || directory_start == u32::MAX
    {
        return Err("invalid_archive".into());
    }
    if count as usize > MAX_ENTRIES || directory_size as u64 > MAX_DIRECTORY {
        return Err("archive_limit".into());
    }
    let directory_start = directory_start as u64;
    let directory_end = directory_start + directory_size as u64;
    if directory_end != tail_start + end as u64 {
        return Err("invalid_archive".into());
    }
    let mut position = directory_start;
    let mut actual_count = 0;
    let mut extra_buffer = vec![0; u16::MAX as usize];
    while position < directory_end {
        actual_count += 1;
        if actual_count > MAX_ENTRIES {
            return Err("archive_limit".into());
        }
        if directory_end - position < 46 {
            return Err("invalid_archive".into());
        }
        reader
            .seek(SeekFrom::Start(position))
            .map_err(|_| "invalid_archive")?;
        let mut header = [0; 46];
        reader
            .read_exact(&mut header)
            .map_err(|_| "invalid_archive")?;
        if &header[..4] != b"PK\x01\x02"
            || u16_at(&header, 34) != 0
            || u32_at(&header, 20) == u32::MAX
            || u32_at(&header, 24) == u32::MAX
            || u32_at(&header, 42) == u32::MAX
        {
            return Err("invalid_archive".into());
        }
        let extra_start = position + 46 + u16_at(&header, 28) as u64;
        let extra_end = extra_start + u16_at(&header, 30) as u64;
        let next = extra_end + u16_at(&header, 32) as u64;
        if next > directory_end {
            return Err("invalid_archive".into());
        }
        let extra_length = u16_at(&header, 30) as usize;
        reader
            .seek(SeekFrom::Start(extra_start))
            .map_err(|_| "invalid_archive")?;
        reader
            .read_exact(&mut extra_buffer[..extra_length])
            .map_err(|_| "invalid_archive")?;
        let extra = &extra_buffer[..extra_length];
        let mut extra_position = 0;
        while extra_position < extra_length {
            if extra_length - extra_position < 4 {
                return Err("invalid_archive".into());
            }
            if u16_at(extra, extra_position) == 0x0001 {
                return Err("invalid_archive".into());
            }
            extra_position += 4 + u16_at(extra, extra_position + 2) as usize;
            if extra_position > extra_length {
                return Err("invalid_archive".into());
            }
        }
        position = next;
    }
    if actual_count != count as usize {
        return Err("invalid_archive".into());
    }
    Ok(())
}

fn snapshot_zip(mut source: impl Read, directory: &Path, limit: u64) -> Result<fs::File, String> {
    let mut snapshot = tempfile::tempfile_in(directory).map_err(|e| e.to_string())?;
    std::io::copy(&mut source.by_ref().take(limit), &mut snapshot).map_err(|e| e.to_string())?;
    let mut overflow = [0];
    if source.read(&mut overflow).map_err(|e| e.to_string())? != 0 {
        return Err("archive_limit".into());
    }
    Ok(snapshot)
}

pub fn extract(path: &Path, cache: &Path) -> Result<Extracted, String> {
    fs::create_dir_all(cache).map_err(|e| e.to_string())?;
    let dir = tempfile::Builder::new()
        .prefix("staging-")
        .tempdir_in(cache)
        .map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    let mut web = false;
    if extension(path) != "zip" {
        if !is_font(path) {
            return Err("unsupported_format".into());
        }
        let dest = dir.path().join(format!("font.{}", extension(path)));
        let before = hash_file(path, MAX_FILE)?;
        bounded_copy(crate::files::open_regular(path, MAX_FILE)?, &dest, MAX_FILE)?;
        if hash_file(&dest, MAX_FILE)? != before {
            return Err("download_changed".into());
        }
        files.push(dest);
    } else {
        let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        if !metadata.is_file() || metadata.len() > MAX_ARCHIVE {
            return Err("archive_limit".into());
        }
        // Downloads may still change after the watcher observes them. Preflight
        // and parsing must use the same private, stable bytes. The anonymous file
        // is dropped with ZipArchive and is never retained with pending fonts.
        let mut source = snapshot_zip(
            crate::files::open_regular(path, MAX_ARCHIVE)?,
            dir.path(),
            MAX_ARCHIVE,
        )?;
        preflight_zip(&mut source)?;
        let mut zip = zip::ZipArchive::with_config(
            zip::read::Config {
                archive_offset: zip::read::ArchiveOffset::Known(0),
            },
            source,
        )
        .map_err(|_| "invalid_archive")?;
        if zip.len() > MAX_ENTRIES {
            return Err("archive_limit".into());
        }
        if zip.has_duplicate_names() {
            return Err("duplicate_archive_path".into());
        }
        let mut total = 0u64;
        let mut names = std::collections::HashSet::new();
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|_| "invalid_archive")?;
            let rel = safe_name(&entry.name().map_err(|_| "invalid_archive")?)?;
            if entry.enclosed_name().is_none() {
                return Err("unsafe_archive_path".into());
            }
            if entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
            {
                return Err("archive_symlink".into());
            }
            if entry.is_dir() || metadata_file(&rel) {
                continue;
            }
            if matches!(extension(&rel).as_str(), "woff" | "woff2") {
                web = true;
            }
            if !is_font(&rel) {
                continue;
            }
            total = total.checked_add(entry.size()).ok_or("archive_limit")?;
            if entry.size() > MAX_FILE || total > MAX_ARCHIVE || files.len() >= MAX_FONTS {
                return Err("archive_limit".into());
            }
            if !names.insert(rel.to_string_lossy().to_lowercase()) {
                return Err("duplicate_archive_path".into());
            }
            let dest = dir
                .path()
                .join(format!("font-{}.{}", files.len(), extension(&rel)));
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            // CRC is checked when the reader reaches EOF; limits also cover falsified sizes.
            let limit = entry_limit(MAX_FILE, entry.size());
            let copied = bounded_copy(&mut entry, &dest, limit)?;
            if copied != entry.size() {
                return Err("invalid_archive".into());
            }
            files.push(dest);
        }
    }
    let web_only = files.is_empty() && web;
    Ok(Extracted {
        dir,
        files,
        web_only,
    })
}
fn entry_limit(max: u64, declared: u64) -> u64 {
    max.min(declared)
}
fn bounded_copy(mut source: impl Read, dest: &Path, limit: u64) -> Result<u64, String> {
    let mut dest = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dest)
        .map_err(|e| e.to_string())?;
    let count = std::io::copy(&mut source.by_ref().take(limit + 1), &mut dest)
        .map_err(|e| e.to_string())?;
    if count > limit {
        return Err("file_limit".into());
    }
    dest.flush().map_err(|e| e.to_string())?;
    Ok(count)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use zip::{write::SimpleFileOptions, ZipWriter};
    fn archive(root: &Path, entries: &[(&str, &[u8])]) -> PathBuf {
        let path = root.join("download.zip");
        let mut zip = ZipWriter::new(File::create(&path).unwrap());
        for (name, data) in entries {
            zip.start_file(*name, SimpleFileOptions::default()).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
        path
    }
    fn repeated_directory(root: &Path, count: u16, declared_count: u16) -> PathBuf {
        let path = archive(root, &[("a.ttf", b"font")]);
        let original = fs::read(&path).unwrap();
        let end = original.len() - 22;
        let directory_start = u32_at(&original, end + 16) as usize;
        let record = &original[directory_start..end];
        let mut bytes = original[..directory_start].to_vec();
        for _ in 0..count {
            bytes.extend_from_slice(record);
        }
        let mut eocd = original[end..].to_vec();
        eocd[8..10].copy_from_slice(&declared_count.to_le_bytes());
        eocd[10..12].copy_from_slice(&declared_count.to_le_bytes());
        eocd[12..16].copy_from_slice(&(record.len() as u32 * count as u32).to_le_bytes());
        bytes.extend_from_slice(&eocd);
        fs::write(&path, bytes).unwrap();
        path
    }
    #[test]
    fn zip_snapshot_stays_stable_when_download_changes_and_is_removed_on_drop() {
        let root = tempfile::tempdir().unwrap();
        let staging = tempfile::tempdir().unwrap();
        let path = archive(root.path(), &[("a.ttf", b"font")]);
        let original = fs::read(&path).unwrap();
        let mut snapshot =
            snapshot_zip(File::open(&path).unwrap(), staging.path(), MAX_ARCHIVE).unwrap();
        assert_eq!(fs::read(&path).unwrap(), original);
        fs::write(&path, b"download changed after the snapshot").unwrap();
        preflight_zip(&mut snapshot).unwrap();
        let mut zip = zip::ZipArchive::new(snapshot).unwrap();
        let mut content = String::new();
        zip.by_index(0)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert_eq!(content, "font");
        drop(zip);
        assert_eq!(fs::read_dir(staging.path()).unwrap().count(), 0);
    }
    #[test]
    fn zip_snapshot_enforces_its_limit_and_cleans_up_rejected_copies() {
        let root = tempfile::tempdir().unwrap();
        let snapshot = snapshot_zip(&b"12345678"[..], root.path(), 8).unwrap();
        assert_eq!(snapshot.metadata().unwrap().len(), 8);
        drop(snapshot);
        assert_eq!(
            snapshot_zip(&b"123456789"[..], root.path(), 8)
                .err()
                .as_deref(),
            Some("archive_limit")
        );
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }
    #[test]
    fn raw_entry_limit_cannot_be_bypassed_with_duplicate_names_or_a_forged_count() {
        for declared_count in [MAX_ENTRIES as u16 + 1, 1] {
            let root = tempfile::tempdir().unwrap();
            let path = repeated_directory(root.path(), MAX_ENTRIES as u16 + 1, declared_count);
            // The library's public length hides the repeated names.
            let zip = zip::ZipArchive::new(File::open(&path).unwrap()).unwrap();
            assert_eq!(zip.len(), 1);
            assert_eq!(
                extract(&path, root.path()).err().as_deref(),
                Some("archive_limit")
            );
        }
    }
    #[test]
    fn exact_duplicate_names_are_rejected_without_changing_the_source() {
        let root = tempfile::tempdir().unwrap();
        let path = repeated_directory(root.path(), 2, 2);
        let original = fs::read(&path).unwrap();
        assert_eq!(
            extract(&path, root.path()).err().as_deref(),
            Some("duplicate_archive_path")
        );
        assert_eq!(fs::read(&path).unwrap(), original);
    }
    #[test]
    fn forged_directory_size_and_truncated_directory_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let path = archive(root.path(), &[("a.ttf", b"font")]);
        let original = fs::read(&path).unwrap();
        let end = original.len() - 22;
        let mut oversized = original.clone();
        oversized[end + 12..end + 16].copy_from_slice(&(MAX_DIRECTORY as u32 + 1).to_le_bytes());
        assert_eq!(
            preflight_zip(&mut std::io::Cursor::new(oversized))
                .err()
                .as_deref(),
            Some("archive_limit")
        );

        let directory_start = u32_at(&original, end + 16) as usize;
        let mut truncated = original[..directory_start + 45].to_vec();
        let mut eocd = original[end..].to_vec();
        eocd[12..16].copy_from_slice(&45u32.to_le_bytes());
        truncated.extend_from_slice(&eocd);
        assert_eq!(
            preflight_zip(&mut std::io::Cursor::new(truncated))
                .err()
                .as_deref(),
            Some("invalid_archive")
        );

        let mut beyond_end = original.clone();
        beyond_end[end + 16..end + 20].copy_from_slice(&(u32::MAX - 1).to_le_bytes());
        assert!(preflight_zip(&mut std::io::Cursor::new(beyond_end)).is_err());
        let mut forged_record = original;
        forged_record[directory_start + 32..directory_start + 34]
            .copy_from_slice(&u16::MAX.to_le_bytes());
        assert!(preflight_zip(&mut std::io::Cursor::new(forged_record)).is_err());
    }
    #[test]
    fn zip64_split_archives_and_mismatched_counts_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let path = archive(root.path(), &[("a.ttf", b"font")]);
        let original = fs::read(&path).unwrap();
        let end = original.len() - 22;
        for offset in [4, 6, 8, 10] {
            let mut bytes = original.clone();
            bytes[end + offset..end + offset + 2].copy_from_slice(&u16::MAX.to_le_bytes());
            assert!(preflight_zip(&mut std::io::Cursor::new(bytes)).is_err());
        }
        for offset in [12, 16] {
            let mut bytes = original.clone();
            bytes[end + offset..end + offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
            assert!(preflight_zip(&mut std::io::Cursor::new(bytes)).is_err());
        }
        let path = repeated_directory(root.path(), 2, 1);
        assert_eq!(
            extract(&path, root.path()).err().as_deref(),
            Some("invalid_archive")
        );
    }
    #[test]
    fn zip64_and_malformed_extra_fields_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let path = archive(root.path(), &[("a.ttf", b"font")]);
        let original = fs::read(&path).unwrap();
        let directory_start = u32_at(&original, original.len() - 6) as usize;
        let extra_start = directory_start + 46 + u16_at(&original, directory_start + 28) as usize;
        let old_extra_length = u16_at(&original, directory_start + 30) as usize;
        for (extra, valid) in [
            (&b"\xfe\xca\x03\x00abc"[..], true),
            (&b"\x01\x00\x00\x00"[..], false),
            (&b"\xfe\xca\x00"[..], false),
            (&b"\xfe\xca\x20\x00"[..], false),
        ] {
            let mut bytes = original.clone();
            bytes.splice(
                extra_start..extra_start + old_extra_length,
                extra.iter().copied(),
            );
            bytes[directory_start + 30..directory_start + 32]
                .copy_from_slice(&(extra.len() as u16).to_le_bytes());
            let end = bytes.len() - 22;
            bytes[end + 12..end + 16]
                .copy_from_slice(&((end - directory_start) as u32).to_le_bytes());
            assert_eq!(
                preflight_zip(&mut std::io::Cursor::new(bytes)).is_ok(),
                valid
            );
        }
    }
    #[test]
    fn ordinary_comments_and_empty_archives_work_but_ambiguous_ends_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("comment.zip");
        let mut zip = ZipWriter::new(File::create(&path).unwrap());
        zip.start_file("a.ttf", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"font").unwrap();
        zip.set_comment("A font archive").unwrap();
        zip.finish().unwrap();
        assert_eq!(extract(&path, root.path()).unwrap().files.len(), 1);
        let mut bytes = fs::read(&path).unwrap();
        bytes.extend_from_slice(b"PK\x05\x06");
        assert!(preflight_zip(&mut std::io::Cursor::new(bytes)).is_err());
        let empty = archive(root.path(), &[]);
        assert!(extract(&empty, root.path()).unwrap().files.is_empty());
    }
    #[test]
    fn nested_fonts_only_and_apple_metadata_ignored() {
        let root = tempfile::tempdir().unwrap();
        let a = archive(
            root.path(),
            &[
                ("fonts/a.TTF", b"font"),
                ("__MACOSX/b.ttf", b"ignore"),
                ("._a.ttf", b"ignore"),
                ("LICENSE.txt", b"text"),
            ],
        );
        let e = extract(&a, root.path()).unwrap();
        assert_eq!(e.files.len(), 1);
        assert!(!e.web_only);
    }
    #[test]
    fn empty_zip_is_silent_and_web_only_is_distinct() {
        let r = tempfile::tempdir().unwrap();
        let a = archive(r.path(), &[("readme.txt", b"a")]);
        let e = extract(&a, r.path()).unwrap();
        assert!(e.files.is_empty() && !e.web_only);
        drop(e);
        let a = archive(r.path(), &[("fonts/a.woff2", b"a")]);
        assert!(extract(&a, r.path()).unwrap().web_only);
    }
    #[test]
    fn zip_slip_is_rejected() {
        for name in [
            "../escaped.ttf",
            "/abs.ttf",
            "a/../../b.ttf",
            "C:/evil.ttf",
            "a\\..\\b.ttf",
        ] {
            let r = tempfile::tempdir().unwrap();
            let a = archive(r.path(), &[(name, b"x")]);
            assert!(extract(&a, r.path()).is_err(), "{name}");
            assert!(!r.path().join("escaped.ttf").exists());
        }
    }
    #[test]
    fn excessive_paths_are_rejected_and_files_are_staged_without_archive_permissions() {
        let r = tempfile::tempdir().unwrap();
        let deep = format!("{}font.ttf", "dir/".repeat(20));
        let a = archive(r.path(), &[(deep.as_str(), b"font")]);
        assert!(extract(&a, r.path()).is_err());
        let a = archive(r.path(), &[("nested/CON.ttf", b"font")]);
        let e = extract(&a, r.path()).unwrap();
        assert_eq!(e.files[0].file_name().unwrap(), "font-0.ttf");
        assert_eq!(e.files[0].parent().unwrap(), e.dir.path());
    }
    #[test]
    fn case_collisions_are_rejected() {
        let r = tempfile::tempdir().unwrap();
        let a = archive(r.path(), &[("a.ttf", b"x"), ("A.ttf", b"y")]);
        assert!(extract(&a, r.path()).is_err());
    }
    #[test]
    fn symlink_zip_entry_is_rejected() {
        let r = tempfile::tempdir().unwrap();
        let p = r.path().join("link.zip");
        let mut z = ZipWriter::new(File::create(&p).unwrap());
        z.add_symlink("link.ttf", "../../escape", SimpleFileOptions::default())
            .unwrap();
        z.finish().unwrap();
        assert!(extract(&p, r.path()).is_err());
    }
    #[test]
    fn partial_and_web_extensions_never_enter_pipeline() {
        for p in [
            "font.ttf.part",
            "a.crdownload",
            "a.download",
            "a.zip.tmp",
            "a.woff2",
        ] {
            assert!(!is_download(Path::new(p)));
        }
        assert!(is_download(Path::new("font.OTF")));
    }
}
