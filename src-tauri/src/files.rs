//! Bounded reads of regular files. Opening must not follow a swapped final symlink.
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    path::Path,
};

pub fn open_regular(path: &Path, limit: u64) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // NONBLOCK prevents a replacement FIFO from blocking before metadata is checked.
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    let file = options.open(path).map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("file_limit".into());
        }
    }
    if !metadata.is_file() || metadata.len() > limit {
        return Err("file_limit".into());
    }
    Ok(file)
}

pub fn read_regular(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let file = open_regular(path, limit)?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("file_limit".into());
    }
    Ok(bytes)
}

pub fn private_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| e.to_string())?;
    if !fs::symlink_metadata(path)
        .map_err(|e| e.to_string())?
        .is_dir()
    {
        return Err("invalid_data_directory".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_directories_and_oversized_files() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("font.ttf");
        fs::write(&path, b"12345").unwrap();
        assert!(read_regular(root.path(), 10).is_err());
        assert!(read_regular(&path, 4).is_err());
        assert_eq!(read_regular(&path, 5).unwrap(), b"12345");
    }
    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_and_keeps_cache_private() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        fs::write(&source, b"font").unwrap();
        let link = root.path().join("link");
        symlink(&source, &link).unwrap();
        assert!(read_regular(&link, 20).is_err());
        let cache = root.path().join("private");
        private_dir(&cache).unwrap();
        assert_eq!(
            fs::metadata(&cache).unwrap().permissions().mode() & 0o777,
            0o700
        );
        let dir_link = root.path().join("dir-link");
        symlink(&cache, &dir_link).unwrap();
        assert!(private_dir(&dir_link).is_err());
    }
}
