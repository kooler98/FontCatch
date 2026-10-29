//! User-selected WOFF2 exports. Paths are chosen by native dialogs, never IPC.
use crate::{engine, files, installer, preview, settings, woff2};
use serde::Serialize;
use std::{
    fs,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOutcome {
    file_name: String,
    input_bytes: usize,
    output_bytes: usize,
}

fn read_external(path: &Path) -> Result<Vec<u8>, String> {
    if !matches!(crate::extractor::extension(path).as_str(), "ttf" | "otf") {
        return Err("woff2_unsupported_file".into());
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| "woff2_read_failed")?;
    if !metadata.is_file() {
        return Err("woff2_read_failed".into());
    }
    if metadata.len() > woff2::MAX_INPUT as u64 {
        return Err("woff2_input_too_large".into());
    }
    let bytes = files::read_regular(path, woff2::MAX_INPUT as u64).map_err(|error| {
        if error == "file_limit" {
            "woff2_input_too_large"
        } else {
            "woff2_read_failed"
        }
    })?;
    if bytes.starts_with(b"ttcf") {
        return Err("woff2_unsupported_file".into());
    }
    Ok(bytes)
}

fn suggested_name(name: &str) -> String {
    // Metadata is untrusted. A suggestion must be a single portable filename,
    // without path separators, control characters or Windows device names.
    let mut stem: String = name
        .chars()
        .take(100)
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') {
                c
            } else {
                '-'
            }
        })
        .collect();
    stem = stem.trim_matches('-').to_string();
    if stem.is_empty() {
        stem = "font".into();
    }
    let upper = stem.to_ascii_uppercase();
    if matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (upper.len() == 4
            && (upper.starts_with("COM") || upper.starts_with("LPT"))
            && matches!(upper.as_bytes()[3], b'1'..=b'9'))
    {
        stem.insert_str(0, "font-");
    }
    format!("{stem}.woff2")
}

fn destination(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("woff2_destination_invalid".into());
    }
    let mut path = path.to_path_buf();
    if path.extension().is_none() {
        path.set_extension("woff2");
    }
    if crate::extractor::extension(&path) != "woff2" {
        return Err("woff2_destination_invalid".into());
    }
    let filename = path.file_name().ok_or("woff2_destination_invalid")?;
    let parent = path
        .parent()
        .ok_or("woff2_destination_invalid")?
        .canonicalize()
        .map_err(|_| "woff2_destination_invalid")?;
    if !parent.is_dir() {
        return Err("woff2_destination_invalid".into());
    }
    Ok(parent.join(filename))
}

fn save_new(path: &Path, bytes: &[u8]) -> Result<PathBuf, String> {
    let path = destination(path)?;
    // This early check gives a clear error. persist_noclobber is the atomic
    // final authority, including when another file appears during the write.
    match fs::symlink_metadata(&path) {
        Ok(_) => return Err("woff2_destination_exists".into()),
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(_) => return Err("woff2_save_failed".into()),
    }
    let mut temporary = tempfile::Builder::new()
        .prefix(".fontcatch-export-")
        .tempfile_in(path.parent().ok_or("woff2_destination_invalid")?)
        .map_err(|_| "woff2_save_failed")?;
    temporary
        .write_all(bytes)
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|_| "woff2_save_failed")?;
    temporary.persist_noclobber(&path).map_err(|error| {
        if error.error.kind() == ErrorKind::AlreadyExists {
            "woff2_destination_exists"
        } else {
            "woff2_save_failed"
        }
    })?;
    Ok(path)
}

pub fn run(
    app: &tauri::AppHandle,
    postscript: Option<String>,
) -> Result<Option<ExportOutcome>, String> {
    let fr = settings::read(app)?.language == settings::Language::Fr;
    let window = app.get_webview_window("main").ok_or("woff2_save_failed")?;
    let (bytes, name, directory) = if let Some(postscript) = postscript {
        let handle = app.state::<engine::EngineHandle>().inner().clone();
        let font = handle.request(|reply| engine::Message::Font(postscript, reply))?;
        let platform = installer::native()?;
        let path = preview::managed_path(&font, platform.root())?;
        if fs::metadata(path).map_err(|_| "font_file_missing")?.len() > woff2::MAX_INPUT as u64 {
            return Err("woff2_input_too_large".into());
        }
        // The managed hash is verified and a TTC's selected face is extracted
        // in memory. Conversion itself stays off the detection/installation queue.
        (
            preview::read(&font, platform.root())?,
            suggested_name(&font.face.postscript),
            dirs::download_dir(),
        )
    } else {
        let selected = app
            .dialog()
            .file()
            .set_parent(&window)
            .set_title(if fr {
                "Choisir une police"
            } else {
                "Choose a font"
            })
            .add_filter("TrueType / OpenType", &["ttf", "otf"])
            .blocking_pick_file();
        let Some(selected) = selected else {
            return Ok(None);
        };
        let path = selected.into_path().map_err(|_| "woff2_read_failed")?;
        let name = suggested_name(&path.file_stem().unwrap_or_default().to_string_lossy());
        (
            read_external(&path)?,
            name,
            path.parent().map(Path::to_path_buf),
        )
    };
    let output = woff2::encode(&bytes)?;
    // A macOS save sheet needs a visible parent even if the user navigated away
    // while compression was running.
    crate::panel::show(app, None);
    let mut dialog = app
        .dialog()
        .file()
        .set_parent(&window)
        .set_title(if fr {
            "Exporter en WOFF2"
        } else {
            "Export WOFF2"
        })
        .add_filter("WOFF2", &["woff2"])
        .set_file_name(name);
    if let Some(directory) = directory {
        dialog = dialog.set_directory(directory);
    }
    let Some(selected) = dialog.blocking_save_file() else {
        return Ok(None);
    };
    let path = selected
        .into_path()
        .map_err(|_| "woff2_destination_invalid")?;
    let saved = save_new(&path, &output)?;
    Ok(Some(ExportOutcome {
        file_name: saved
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        input_bytes: bytes.len(),
        output_bytes: output.len(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    const FONT: &[u8] = include_bytes!("../tests/fixtures/FontcatchQA-Regular.ttf");

    #[test]
    fn external_conversion_preserves_the_source_and_existing_exports() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("My font.TTF");
        fs::write(&source, FONT).unwrap();
        let input = read_external(&source).unwrap();
        let output = woff2::encode(&input).unwrap();
        let saved = save_new(&dir.path().join("My font"), &output).unwrap();
        assert_eq!(saved.extension().unwrap(), "woff2");
        assert_eq!(fs::read(&saved).unwrap(), output);
        assert_eq!(fs::read(&source).unwrap(), FONT);
        assert_eq!(
            save_new(&saved, b"replacement").unwrap_err(),
            "woff2_destination_exists"
        );
        assert_eq!(
            save_new(&source, &output).unwrap_err(),
            "woff2_destination_invalid"
        );
        assert_eq!(fs::read(&saved).unwrap(), output);
        assert_eq!(fs::read(&source).unwrap(), FONT);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn export_paths_and_source_types_are_restricted() {
        let dir = tempfile::tempdir().unwrap();
        assert!(destination(Path::new("relative.woff2")).is_err());
        assert!(destination(&dir.path().join("missing/font.woff2")).is_err());
        assert!(destination(&dir.path().join("font.txt")).is_err());
        assert_eq!(
            read_external(&dir.path().join("font.woff2")).unwrap_err(),
            "woff2_unsupported_file"
        );
        let collection = dir.path().join("collection.ttf");
        fs::write(&collection, b"ttcf").unwrap();
        assert_eq!(
            read_external(&collection).unwrap_err(),
            "woff2_unsupported_file"
        );
        let large = dir.path().join("large.ttf");
        fs::File::create(&large)
            .unwrap()
            .set_len(woff2::MAX_INPUT as u64 + 1)
            .unwrap();
        assert_eq!(read_external(&large).unwrap_err(), "woff2_input_too_large");
        fs::create_dir(dir.path().join("existing.woff2")).unwrap();
        assert_eq!(
            save_new(&dir.path().join("existing.woff2"), b"font").unwrap_err(),
            "woff2_destination_exists"
        );
    }

    #[test]
    fn file_suggestions_cannot_contain_paths_or_reserved_device_names() {
        for name in ["../font", "C:\\Fonts\\font", "\0\n", "...", "CON", "LPT9"] {
            let name = suggested_name(name);
            assert_eq!(Path::new(&name).components().count(), 1);
            assert!(name.ends_with(".woff2"));
            assert!(!name.contains(['/', '\\', ':', '\0', '\n']));
        }
        assert_eq!(suggested_name("CON"), "font-CON.woff2");
        assert_eq!(
            suggested_name("FontcatchQA-Regular"),
            "FontcatchQA-Regular.woff2"
        );
    }

    #[cfg(unix)]
    #[test]
    fn source_and_destination_symlinks_are_not_followed() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("font.ttf");
        fs::write(&source, FONT).unwrap();
        let alias = dir.path().join("alias.ttf");
        symlink(&source, &alias).unwrap();
        assert_eq!(read_external(&alias).unwrap_err(), "woff2_read_failed");
        let target = dir.path().join("font.woff2");
        symlink(&source, &target).unwrap();
        assert_eq!(
            save_new(&target, b"replacement").unwrap_err(),
            "woff2_destination_exists"
        );
        assert_eq!(fs::read(source).unwrap(), FONT);
    }
}
