//! Bounded, local WOFF2 export. Originals never enter a write path.
//!
//! Null table transforms preserve outlines, variation and licensing tables while
//! avoiding an additional glyph interpreter. WOFF2 §5 still requires dropping
//! DSIG and setting head.flags bit 11, so normalize a private copy first.
//! https://www.w3.org/TR/WOFF2/#DataTables
use std::collections::BTreeSet;

const MAX_TABLES: usize = 256;
pub const MAX_INPUT: usize = 32 * 1024 * 1024;
const MAX_OUTPUT: usize = MAX_INPUT + 65_536;
const INVALID: &str = "woff2_invalid_font";

struct Table<'a> {
    tag: [u8; 4],
    data: &'a [u8],
}

fn u16_at(data: &[u8], offset: usize) -> Result<u16, String> {
    let bytes = data.get(offset..offset + 2).ok_or(INVALID)?;
    Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn u32_at(data: &[u8], offset: usize) -> Result<u32, String> {
    let bytes = data.get(offset..offset + 4).ok_or(INVALID)?;
    Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0u32, |sum, bytes| {
        let mut word = [0; 4];
        word[..bytes.len()].copy_from_slice(bytes);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}

fn put32(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn tables(data: &[u8]) -> Result<Vec<Table<'_>>, String> {
    if data.len() > MAX_INPUT {
        return Err("woff2_input_too_large".into());
    }
    if !matches!(u32_at(data, 0)?, 0x0001_0000 | 0x4f54_544f) {
        return Err(INVALID.into());
    }
    let count = u16_at(data, 4)? as usize;
    if count == 0 || count > MAX_TABLES {
        return Err(INVALID.into());
    }
    let directory_end = 12 + count * 16;
    if directory_end > data.len() {
        return Err(INVALID.into());
    }
    let mut total = 0usize;
    let mut ranges = Vec::with_capacity(count);
    let mut tags = BTreeSet::new();
    let mut tables = Vec::with_capacity(count);
    for index in 0..count {
        let record = 12 + index * 16;
        let tag: [u8; 4] = data[record..record + 4].try_into().map_err(|_| INVALID)?;
        if !tags.insert(tag) {
            return Err(INVALID.into());
        }
        let start = u32_at(data, record + 8)? as usize;
        let length = u32_at(data, record + 12)? as usize;
        let end = start.checked_add(length).ok_or(INVALID)?;
        total = total.checked_add(length).ok_or(INVALID)?;
        if total > MAX_INPUT {
            return Err("woff2_input_too_large".into());
        }
        if start < directory_end || !start.is_multiple_of(4) || end > data.len() {
            return Err(INVALID.into());
        }
        if length > 0 {
            ranges.push((start, end));
        }
        tables.push(Table {
            tag,
            data: &data[start..end],
        });
    }
    ranges.sort_unstable();
    if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err(INVALID.into());
    }
    crate::inspector::validated_font(data, 0).map_err(|_| INVALID)?;
    tables.sort_by_key(|table| table.tag);
    Ok(tables)
}

fn normalized(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut tables = tables(data)?;
    tables.retain(|table| table.tag != *b"DSIG");
    let count = tables.len();
    let mut output = vec![0; 12 + count * 16];
    put32(&mut output, 0, u32_at(data, 0)?);
    output[4..6].copy_from_slice(&(count as u16).to_be_bytes());
    let exponent = count.ilog2() as u16;
    let range = (1u16 << exponent) * 16;
    output[6..8].copy_from_slice(&range.to_be_bytes());
    output[8..10].copy_from_slice(&exponent.to_be_bytes());
    output[10..12].copy_from_slice(&((count as u16 * 16) - range).to_be_bytes());
    let mut head_offset = None;
    for (index, table) in tables.iter().enumerate() {
        let start = output.len();
        let end = start.checked_add(table.data.len()).ok_or(INVALID)?;
        let aligned_end = end.checked_add(3).ok_or(INVALID)? & !3;
        if aligned_end > MAX_OUTPUT {
            return Err("woff2_input_too_large".into());
        }
        output.extend_from_slice(table.data);
        output.resize(aligned_end, 0);
        if table.tag == *b"head" {
            let flags = u16_at(table.data, 16)? | 0x0800;
            output[start + 16..start + 18].copy_from_slice(&flags.to_be_bytes());
            put32(&mut output, start + 8, 0);
            head_offset = Some(start);
        }
        let record = 12 + index * 16;
        output[record..record + 4].copy_from_slice(&table.tag);
        let sum = checksum(&output[start..end]);
        put32(&mut output, record + 4, sum);
        put32(&mut output, record + 8, start as u32);
        put32(&mut output, record + 12, table.data.len() as u32);
    }
    let adjustment = 0xb1b0_afbau32.wrapping_sub(checksum(&output));
    put32(&mut output, head_offset.ok_or(INVALID)? + 8, adjustment);
    Ok(output)
}

pub fn encode(data: &[u8]) -> Result<Vec<u8>, String> {
    let normalized = normalized(data)?;
    // Quality 8 avoids the long maximum-quality pass, and the default single
    // compression thread keeps memory and CPU use predictable. Work is dispatched
    // off the UI thread by the command layer.
    let output = ttf2woff2::encode_no_transform(&normalized, 8u8.into())
        .map_err(|_| "woff2_conversion_failed")?;
    if output.len() > MAX_OUTPUT {
        return Err("woff2_output_too_large".into());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use read_fonts::{types::Tag, TableProvider};

    const REGULAR: &[u8] = include_bytes!("../tests/fixtures/FontcatchQA-Regular.ttf");
    const VARIABLE: &[u8] = include_bytes!("../tests/fixtures/FontcatchQA-Variable.ttf");
    const CFF: &[u8] = include_bytes!("../tests/fixtures/FontcatchQA-CFF.otf");
    const COLLECTION: &[u8] = include_bytes!("../tests/fixtures/FontcatchQA-Collection.ttc");

    fn roundtrip(input: &[u8]) -> Vec<u8> {
        let before = input.to_vec();
        let encoded = encode(input).unwrap();
        assert_eq!(&encoded[..4], b"wOF2");
        assert_eq!(u32_at(&encoded, 8).unwrap() as usize, encoded.len());
        let decoded = wuff::decompress_woff2(&encoded).unwrap();
        let face = crate::inspector::validated_font(&decoded, 0).unwrap();
        assert_eq!(checksum(&decoded), 0xb1b0_afba);
        assert_eq!(face.head().unwrap().flags().bits() & 0x0800, 0x0800);
        assert!(face.table_data(Tag::new(b"DSIG")).is_none());
        for original in tables(input).unwrap() {
            if original.tag == *b"DSIG" {
                continue;
            }
            let actual = face.table_data(Tag::new(&original.tag)).unwrap();
            let mut expected = original.data.to_vec();
            let mut actual = actual.as_bytes().to_vec();
            if original.tag == *b"head" {
                let flags = u16_at(&expected, 16).unwrap() | 0x0800;
                expected[16..18].copy_from_slice(&flags.to_be_bytes());
                expected[8..12].fill(0);
                actual[8..12].fill(0);
            }
            assert_eq!(actual, expected, "table {:?}", original.tag);
        }
        assert_eq!(input, before);
        decoded
    }

    #[test]
    fn static_variable_and_cff_fonts_preserve_tables_with_independent_decoder() {
        for input in [REGULAR, VARIABLE, CFF] {
            let decoded = roundtrip(input);
            let original = crate::inspector::validated_font(input, 0).unwrap();
            let decoded = crate::inspector::validated_font(&decoded, 0).unwrap();
            assert_eq!(
                original.table_data(Tag::new(b"fvar")).is_some(),
                decoded.table_data(Tag::new(b"fvar")).is_some()
            );
        }
    }

    #[test]
    fn collections_are_converted_one_selected_face_at_a_time() {
        assert_eq!(encode(COLLECTION).unwrap_err(), INVALID);
        for index in 0..2 {
            let standalone = crate::preview::standalone(COLLECTION, index).unwrap();
            roundtrip(&standalone);
        }
    }

    #[test]
    fn rejects_truncated_duplicate_overlapping_and_header_pointing_tables() {
        for end in [0, 3, 6, 12, 20] {
            assert_eq!(encode(&REGULAR[..end]).unwrap_err(), INVALID);
        }
        let mut duplicate = REGULAR.to_vec();
        duplicate.copy_within(12..16, 28);
        assert_eq!(encode(&duplicate).unwrap_err(), INVALID);

        let first_offset = u32_at(REGULAR, 20).unwrap();
        let mut overlap = REGULAR.to_vec();
        put32(&mut overlap, 36, first_offset);
        assert_eq!(encode(&overlap).unwrap_err(), INVALID);

        let mut header = REGULAR.to_vec();
        put32(&mut header, 20, 12);
        assert_eq!(encode(&header).unwrap_err(), INVALID);

        let mut out_of_bounds = REGULAR.to_vec();
        put32(&mut out_of_bounds, 20, u32::MAX - 3);
        assert_eq!(encode(&out_of_bounds).unwrap_err(), INVALID);

        let mut too_many = REGULAR.to_vec();
        too_many[4..6].copy_from_slice(&257u16.to_be_bytes());
        assert_eq!(encode(&too_many).unwrap_err(), INVALID);
    }

    #[test]
    fn rejects_file_and_declared_table_allocation_limits_before_compression() {
        let oversized = vec![0; MAX_INPUT + 1];
        assert_eq!(encode(&oversized).unwrap_err(), "woff2_input_too_large");
        let mut declared = REGULAR.to_vec();
        put32(&mut declared, 24, MAX_INPUT as u32 + 1);
        assert_eq!(encode(&declared).unwrap_err(), "woff2_input_too_large");
    }

    #[test]
    fn export_removes_digital_signatures_and_marks_lossless_modification() {
        // Add an independently indexed DSIG payload without modifying existing
        // table bodies. A signature cannot survive sfnt reconstruction.
        let source_tables = tables(REGULAR).unwrap();
        let count = source_tables.len() + 1;
        let mut signed = vec![0; 12 + count * 16];
        signed[..12].copy_from_slice(&REGULAR[..12]);
        signed[4..6].copy_from_slice(&(count as u16).to_be_bytes());
        let mut payloads: Vec<_> = source_tables
            .iter()
            .map(|table| (table.tag, table.data.to_vec()))
            .collect();
        payloads.push((*b"DSIG", vec![0, 0, 0, 1, 0, 0, 0, 0]));
        payloads.sort_by_key(|(tag, _)| *tag);
        for (index, (tag, payload)) in payloads.iter().enumerate() {
            let offset = signed.len();
            signed.extend_from_slice(payload);
            signed.resize(signed.len().div_ceil(4) * 4, 0);
            let record = 12 + index * 16;
            signed[record..record + 4].copy_from_slice(tag);
            put32(&mut signed, record + 4, checksum(payload));
            put32(&mut signed, record + 8, offset as u32);
            put32(&mut signed, record + 12, payload.len() as u32);
        }
        roundtrip(&signed);
    }

    #[test]
    #[ignore = "Writes synthetic WOFF2 fixtures to the explicit FONTCATCH_WOFF2_QA_DIR for independent fontTools/browser checks."]
    fn write_woff2_qa_fixtures() {
        let directory = std::path::PathBuf::from(
            std::env::var_os("FONTCATCH_WOFF2_QA_DIR").expect("Set the explicit QA directory"),
        );
        assert!(directory.is_absolute());
        std::fs::create_dir_all(&directory).unwrap();
        let collection = crate::preview::standalone(COLLECTION, 1).unwrap();
        for (name, input) in [
            ("FontcatchQA-Regular", REGULAR),
            ("FontcatchQA-Variable", VARIABLE),
            ("FontcatchQA-CFF", CFF),
            ("FontcatchQA-Collection-1", collection.as_slice()),
        ] {
            std::fs::write(
                directory.join(format!("{name}.woff2")),
                encode(input).unwrap(),
            )
            .unwrap();
        }
    }

    #[test]
    #[ignore = "Measures encoding time and memory at the 32 MiB input limit with original synthetic font data."]
    fn benchmark_bounded_encoder() {
        fn measure(label: &str, input: &[u8]) {
            let start = std::time::Instant::now();
            let output = encode(input).unwrap();
            eprintln!(
                "{label}: {} -> {} bytes, {:?}",
                input.len(),
                output.len(),
                start.elapsed()
            );
        }
        for (label, input) in [("static", REGULAR), ("variable", VARIABLE), ("CFF", CFF)] {
            measure(label, input);
        }
        // Keep valid glyphs and metadata and add an incompressible private table.
        // This exercises peak allocations, rather than a file full of ignored
        // trailing padding that disappears during normalization.
        let source = tables(REGULAR).unwrap();
        let count = source.len() + 1;
        let mut stress = Vec::with_capacity(MAX_INPUT);
        stress.resize(12 + count * 16, 0);
        stress[..12].copy_from_slice(&REGULAR[..12]);
        stress[4..6].copy_from_slice(&(count as u16).to_be_bytes());
        for (index, table) in source.iter().enumerate() {
            let offset = stress.len();
            stress.extend_from_slice(table.data);
            stress.resize(stress.len().div_ceil(4) * 4, 0);
            let record = 12 + index * 16;
            stress[record..record + 4].copy_from_slice(&table.tag);
            put32(&mut stress, record + 4, checksum(table.data));
            put32(&mut stress, record + 8, offset as u32);
            put32(&mut stress, record + 12, table.data.len() as u32);
        }
        let offset = stress.len();
        let record = 12 + source.len() * 16;
        stress[record..record + 4].copy_from_slice(b"zzzz");
        put32(&mut stress, record + 8, offset as u32);
        put32(&mut stress, record + 12, (MAX_INPUT - offset) as u32);
        let mut state = 0x739a_2d65u32;
        while stress.len() < MAX_INPUT {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            stress.extend_from_slice(&state.to_le_bytes());
        }
        measure("32 MiB incompressible table", &stress);
    }
}
