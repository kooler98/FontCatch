use crate::{
    extractor::MAX_FILE,
    model::{FontFace, FontFile},
    settings::FontPreference,
};
use read_fonts::{types::Tag, FileRef, FontRef, TableProvider};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

pub fn inspect(path: &Path) -> Result<FontFile, String> {
    use sha2::{Digest, Sha256};
    let bytes = read_font(path)?;
    let faces = parse_faces(&bytes)?;
    Ok(FontFile {
        path: path.into(),
        hash: format!("{:x}", Sha256::digest(&bytes)),
        faces,
    })
}
pub fn metadata(path: &Path) -> Result<Vec<FontFace>, String> {
    parse_faces(&read_font(path)?)
}
fn read_font(path: &Path) -> Result<Vec<u8>, String> {
    crate::files::read_regular(path, MAX_FILE)
}
pub fn validated_font(bytes: &[u8], index: u32) -> Result<FontRef<'_>, String> {
    let face = FontRef::from_index(bytes, index).map_err(|_| "invalid_font")?;
    let head = face.head().map_err(|_| "invalid_font")?;
    if head.magic_number() != 0x5F0F3CF5
        || !(16..=16384).contains(&head.units_per_em())
        || face.maxp().map_err(|_| "invalid_font")?.num_glyphs() == 0
    {
        return Err("invalid_font".into());
    }
    face.hhea().map_err(|_| "invalid_font")?;
    let records = face.table_directory.table_records();
    if records.is_empty() || records.len() > 4095 {
        return Err("invalid_font".into());
    }
    let mut tags = HashSet::new();
    for record in records {
        if !tags.insert(record.tag()) || face.table_data(record.tag()).is_none() {
            return Err("invalid_font".into());
        }
    }
    Ok(face)
}
fn parse_faces(bytes: &[u8]) -> Result<Vec<FontFace>, String> {
    let file = FileRef::new(bytes).map_err(|_| "invalid_font")?;
    let count = match &file {
        FileRef::Font(_) => 1,
        FileRef::Collection(c) => c.len(),
    };
    if count == 0 || count > 256 {
        return Err("invalid_font".into());
    }
    let mut faces = Vec::new();
    let mut postscript_names = HashSet::new();
    for i in 0..count {
        let face = validated_font(bytes, i)?;
        let names = face.name().map_err(|_| "invalid_font")?;
        if names.count() > 4096 {
            return Err("invalid_font".into());
        }
        let name = |id: u16| -> Option<String> {
            let records = names.name_record();
            records
                .iter()
                .filter(|n| n.is_unicode())
                .chain(records.iter().filter(|n| !n.is_unicode()))
                .filter(|n| n.name_id().to_u16() == id)
                .filter_map(|n| n.string(names.string_data()).ok())
                .map(|s| {
                    s.chars()
                        .filter(|c| {
                            (!c.is_control() || *c == '\n')
                                && !matches!(*c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
                        })
                        .take(if id == 13 { 32768 } else { 2048 })
                        .collect::<String>()
                })
                .find(|s| !s.trim().is_empty())
        };
        let family = name(16).or_else(|| name(1)).ok_or("missing_family_name")?;
        let postscript = name(6).ok_or("missing_postscript_name")?;
        if !postscript_names.insert(postscript.clone()) {
            return Err("invalid_font".into());
        }
        let revision = face
            .head()
            .map_err(|_| "invalid_font")?
            .font_revision()
            .to_bits() as u32;
        let variable = face.table_data(Tag::new(b"fvar")).is_some();
        let axes = if variable {
            let fvar = face.fvar().map_err(|_| "invalid_font")?;
            let axes = fvar.axes().map_err(|_| "invalid_font")?;
            if axes.len() > 64 {
                return Err("invalid_font".into());
            }
            axes.iter()
                .map(|a| {
                    format!(
                        "{} {}–{}",
                        a.axis_tag(),
                        a.min_value().to_f32(),
                        a.max_value().to_f32()
                    )
                })
                .collect()
        } else {
            Vec::new()
        };
        faces.push(FontFace {
            family,
            style: name(17)
                .or_else(|| name(2))
                .unwrap_or_else(|| "Regular".into()),
            postscript,
            version: name(5).unwrap_or_else(|| {
                format!(
                    "{}.{:03}",
                    revision >> 16,
                    (revision & 0xffff) * 1000 / 65536
                )
            }),
            revision,
            foundry: name(8).unwrap_or_default(),
            designer: name(9).unwrap_or_default(),
            license: name(13),
            license_url: name(14).filter(|s| crate::urls::external(s).is_some()),
            variable,
            face_index: i,
            axes,
        });
    }
    Ok(faces)
}
// Preference applies only when both kinds exist in the SAME family.
// TTCs are indivisible: never install unselected faces as a side effect.
pub fn select(files: Vec<FontFile>, preference: &FontPreference) -> (Vec<FontFile>, Vec<String>) {
    let variable: HashSet<_> = files
        .iter()
        .flat_map(|f| &f.faces)
        .filter(|f| f.variable)
        .map(|f| f.family.clone())
        .collect();
    let statics: HashSet<_> = files
        .iter()
        .flat_map(|f| &f.faces)
        .filter(|f| !f.variable)
        .map(|f| f.family.clone())
        .collect();
    let mut warnings = Vec::new();
    let mut selected: Vec<_> = files
        .into_iter()
        .filter(|file| {
            let selected = file
                .faces
                .iter()
                .filter(|face| match preference {
                    FontPreference::Both => true,
                    FontPreference::Variable => face.variable || !variable.contains(&face.family),
                    FontPreference::Static => !face.variable || !statics.contains(&face.family),
                })
                .count();
            if selected > 0 && selected < file.faces.len() {
                warnings.push("mixed_collection".into());
            }
            selected == file.faces.len()
        })
        .collect();
    // Resolve duplicate PS names inside the archive deterministically: newest first,
    // then variable. Do not silently install two resources with the same identity.
    selected.sort_by(|a, b| {
        b.faces
            .iter()
            .map(|f| (f.revision, f.variable))
            .max()
            .cmp(&a.faces.iter().map(|f| (f.revision, f.variable)).max())
            .then_with(|| a.path.cmp(&b.path))
    });
    let mut seen = HashMap::new();
    selected.retain(|file| {
        let unique = file.faces.iter().all(|f| !seen.contains_key(&f.postscript));
        if unique {
            for face in &file.faces {
                seen.insert(face.postscript.clone(), file.hash.clone());
            }
        } else {
            warnings.push("archive_duplicate".into());
        }
        unique
    });
    warnings.sort();
    warnings.dedup();
    (selected, warnings)
}
#[cfg(test)]
mod tests {
    use super::*;
    pub fn fake(family: &str, variable: bool, ps: &str) -> FontFile {
        FontFile {
            path: format!("{ps}.ttf").into(),
            hash: ps.into(),
            faces: vec![FontFace {
                family: family.into(),
                style: "Regular".into(),
                postscript: ps.into(),
                version: "Version 1".into(),
                revision: 65536,
                foundry: "Test".into(),
                designer: String::new(),
                license: None,
                license_url: None,
                variable,
                face_index: 0,
                axes: vec![],
            }],
        }
    }
    #[test]
    fn variable_selection_keeps_unrelated_static_families() {
        let (fs, _) = select(
            vec![
                fake("A", true, "A-V"),
                fake("A", false, "A-R"),
                fake("B", false, "B-R"),
            ],
            &FontPreference::Variable,
        );
        assert_eq!(fs.len(), 2);
        assert!(fs.iter().any(|f| f.faces[0].family == "B"));
    }
    #[test]
    fn static_selection_keeps_variable_only_families() {
        let (fs, _) = select(
            vec![
                fake("A", true, "A-V"),
                fake("A", false, "A-R"),
                fake("B", true, "B-V"),
            ],
            &FontPreference::Static,
        );
        assert_eq!(fs.len(), 2);
        assert!(!fs.iter().any(|f| f.faces[0].postscript == "A-V"));
    }
    #[test]
    fn both_preserves_distinct_postscript_names() {
        assert_eq!(
            select(
                vec![fake("A", true, "A-V"), fake("A", false, "A-R")],
                &FontPreference::Both
            )
            .0
            .len(),
            2
        );
    }
    #[test]
    fn duplicate_postscript_keeps_newest() {
        let a = fake("A", false, "A");
        let mut b = a.clone();
        b.faces[0].revision = 131072;
        let (files, warnings) = select(vec![a, b], &FontPreference::Both);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].faces[0].revision, 131072);
        assert!(warnings.contains(&"archive_duplicate".to_string()));
    }
    #[test]
    fn mixed_collection_cannot_install_filtered_out_faces() {
        let mut c = fake("A", true, "AV");
        c.faces.push(fake("A", false, "AR").faces.remove(0));
        assert!(select(vec![c], &FontPreference::Variable).0.is_empty());
    }
    #[test]
    fn invalid_font_rejected() {
        let r = tempfile::tempdir().unwrap();
        let p = r.path().join("fake.ttf");
        std::fs::write(&p, b"not a font").unwrap();
        assert_eq!(inspect(&p).unwrap_err(), "invalid_font");
    }
}

#[cfg(test)]
mod fixture_tests {
    use super::*;
    #[test]
    fn rejects_corrupt_offsets_duplicate_tables_and_bad_head() {
        let original = include_bytes!("../tests/fixtures/FontcatchQA-Regular.ttf");
        let mut corrupt = original.to_vec();
        corrupt[20..24].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(parse_faces(&corrupt).is_err());
        let mut corrupt = original.to_vec();
        let first_tag = corrupt[12..16].to_vec();
        corrupt[28..32].copy_from_slice(&first_tag);
        assert!(parse_faces(&corrupt).is_err());
        let head = FontRef::new(original)
            .unwrap()
            .table_directory
            .table_records()
            .iter()
            .find(|r| r.tag() == Tag::new(b"head"))
            .unwrap()
            .offset() as usize;
        let mut corrupt = original.to_vec();
        corrupt[head + 12..head + 16].fill(0);
        assert!(parse_faces(&corrupt).is_err());
        for end in [0, 4, 12, 64, original.len() / 2] {
            assert!(parse_faces(&original[..end]).is_err());
        }
    }
    fn font(name: &str) -> FontFile {
        inspect(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name),
        )
        .unwrap()
    }
    #[test]
    fn reads_real_name_license_version_and_fvar() {
        let f = font("FontcatchQA-Variable.ttf");
        assert_eq!(f.faces[0].family, "Fontcatch QA");
        assert!(f.faces[0].variable);
        assert_eq!(f.faces[0].axes, vec!["wght 100–900"]);
        assert!(f.faces[0]
            .license
            .as_ref()
            .unwrap()
            .contains("Original test"));
        assert_eq!(
            f.faces[0].license_url.as_deref(),
            Some("https://example.org/fontcatch-test-license")
        );
    }
    #[test]
    fn parses_all_collection_faces() {
        let f = font("FontcatchQA-Collection.ttc");
        assert_eq!(f.faces.len(), 2);
        assert_eq!(f.faces[1].face_index, 1);
        assert_eq!(f.faces[1].family, "Fontcatch Other");
    }
}
