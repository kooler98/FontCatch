//! Local preview bytes. TTC faces are rebuilt as standalone sfnt resources in memory;
//! installed files are never rewritten. See OpenType's table-directory/checksum spec.
use crate::{extractor::MAX_FILE, model::InstalledFont};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn managed_path(font: &InstalledFont, root: &Path) -> Result<PathBuf, String> {
    if font.path.parent() != Some(root)
        || !font
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .starts_with("Fontcatch-")
    {
        return Err("unmanaged_font".into());
    }
    let metadata = fs::symlink_metadata(&font.path).map_err(|_| "font_file_missing")?;
    if !metadata.is_file() || metadata.len() > MAX_FILE {
        return Err("installed_font_changed".into());
    }
    Ok(font.path.clone())
}
pub fn read(font: &InstalledFont, root: &Path) -> Result<Vec<u8>, String> {
    let path = managed_path(font, root)?;
    let bytes = crate::files::read_regular(&path, MAX_FILE)?;
    if bytes.len() as u64 > MAX_FILE || format!("{:x}", Sha256::digest(&bytes)) != font.hash {
        return Err("installed_font_changed".into());
    }
    standalone(&bytes, font.face.face_index)
}
fn put32(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}
fn checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0u32, |sum, chunk| {
        let mut word = [0; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}
pub fn standalone(data: &[u8], index: u32) -> Result<Vec<u8>, String> {
    let face = crate::inspector::validated_font(data, index)?;
    if data.get(..4) != Some(b"ttcf") {
        return Ok(data.to_vec());
    }
    let mut tables = Vec::new();
    for record in face.table_directory.table_records() {
        let tag = u32::from_be_bytes(record.tag().to_be_bytes());
        if tag == u32::from_be_bytes(*b"DSIG") {
            continue;
        }
        let bytes = face
            .table_data(record.tag())
            .ok_or("invalid_font")?
            .as_bytes();
        tables.push((tag, bytes));
    }
    tables.sort_by_key(|(tag, _)| *tag);
    if tables.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err("invalid_font".into());
    }
    let n = tables.len();
    if n == 0 {
        return Err("invalid_font".into());
    }
    let mut output = vec![0; 12 + n * 16];
    put32(&mut output, 0, face.table_directory.sfnt_version());
    output[4..6].copy_from_slice(&(n as u16).to_be_bytes());
    let exponent = n.ilog2() as u16;
    let range = (1u16 << exponent) * 16;
    output[6..8].copy_from_slice(&range.to_be_bytes());
    output[8..10].copy_from_slice(&exponent.to_be_bytes());
    output[10..12].copy_from_slice(&((n as u16 * 16) - range).to_be_bytes());
    let mut head = None;
    for (i, (tag, bytes)) in tables.iter().enumerate() {
        let offset = output.len();
        if offset + bytes.len() + 3 > MAX_FILE as usize {
            return Err("file_limit".into());
        }
        output.extend_from_slice(bytes);
        output.resize(output.len().div_ceil(4) * 4, 0);
        if *tag == u32::from_be_bytes(*b"head") {
            if bytes.len() < 12 {
                return Err("invalid_font".into());
            }
            put32(&mut output, offset + 8, 0);
            head = Some(offset);
        }
        let sum = checksum(&output[offset..offset + bytes.len()]);
        let record = 12 + i * 16;
        put32(&mut output, record, *tag);
        put32(&mut output, record + 4, sum);
        put32(&mut output, record + 8, offset as u32);
        put32(&mut output, record + 12, bytes.len() as u32);
    }
    let adjustment = 0xB1B0AFBAu32.wrapping_sub(checksum(&output));
    put32(&mut output, head.ok_or("invalid_font")? + 8, adjustment);
    crate::inspector::validated_font(&output, 0)?;
    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::*;
    use read_fonts::TableProvider;
    #[test]
    fn each_collection_face_becomes_a_valid_independent_font() {
        let data = include_bytes!("../tests/fixtures/FontcatchQA-Collection.ttc");
        for index in 0..2 {
            let original = crate::inspector::validated_font(data, index).unwrap();
            let bytes = standalone(data, index).unwrap();
            let face = crate::inspector::validated_font(&bytes, 0).unwrap();
            assert_eq!(
                face.maxp().unwrap().num_glyphs(),
                original.maxp().unwrap().num_glyphs()
            );
            let name = |f: &read_fonts::FontRef<'_>| {
                let names = f.name().unwrap();
                names
                    .name_record()
                    .iter()
                    .find(|n| n.name_id().to_u16() == 6 && n.is_unicode())
                    .unwrap()
                    .string(names.string_data())
                    .unwrap()
                    .to_string()
            };
            assert_eq!(name(&face), name(&original));
            assert_eq!(checksum(&bytes), 0xB1B0AFBA);
        }
        assert!(standalone(data, 2).is_err());
        assert!(standalone(&data[..20], 0).is_err());
    }
    #[test]
    fn a_single_font_is_not_rewritten() {
        let data = include_bytes!("../tests/fixtures/FontcatchQA-Regular.ttf");
        assert_eq!(standalone(data, 0).unwrap(), data);
    }
}
