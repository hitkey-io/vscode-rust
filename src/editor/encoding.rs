//! File encodings (textfile/common/encoding.ts `SUPPORTED_ENCODINGS`,
//! editorStatus.ts `ChangeEncodingAction`): the ids, long / short labels and
//! order are VS Code's; decoding and encoding go through `encoding_rs`
//! (WHATWG tables). Not offered: the DOS code pages and KOI8-T, which
//! `encoding_rs` doesn't have (CP 437 / 850 / 852 / 857 / 865 / 1125,
//! KOI8-T). Unmappable characters are written as `?`, like iconv-lite.

use encoding_rs::Encoding;

pub const UTF8: &str = "utf8";
pub const UTF8_BOM: &str = "utf8bom";

enum Codec {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    /// ISO 8859-1 proper (the WHATWG "latin1" label is windows-1252).
    Latin1,
    Table(&'static Encoding),
}

struct Entry {
    id: &'static str,
    long: &'static str,
    short: &'static str,
    /// Only for saving (`encodeOnly`): hidden when reopening.
    encode_only: bool,
    codec: Codec,
}

macro_rules! entry {
    ($id:literal, $long:literal, $short:literal, $codec:expr) => {
        Entry { id: $id, long: $long, short: $short, encode_only: false, codec: $codec }
    };
}

/// In VS Code's `order`.
static ENTRIES: &[Entry] = &[
    entry!("utf8", "UTF-8", "UTF-8", Codec::Utf8),
    Entry { id: "utf8bom", long: "UTF-8 with BOM", short: "UTF-8 with BOM", encode_only: true, codec: Codec::Utf8Bom },
    entry!("utf16le", "UTF-16 LE", "UTF-16 LE", Codec::Utf16Le),
    entry!("utf16be", "UTF-16 BE", "UTF-16 BE", Codec::Utf16Be),
    entry!("windows1252", "Western (Windows 1252)", "Windows 1252", Codec::Table(encoding_rs::WINDOWS_1252)),
    entry!("iso88591", "Western (ISO 8859-1)", "ISO 8859-1", Codec::Latin1),
    entry!("iso88593", "Western (ISO 8859-3)", "ISO 8859-3", Codec::Table(encoding_rs::ISO_8859_3)),
    entry!("iso885915", "Western (ISO 8859-15)", "ISO 8859-15", Codec::Table(encoding_rs::ISO_8859_15)),
    entry!("macroman", "Western (Mac Roman)", "Mac Roman", Codec::Table(encoding_rs::MACINTOSH)),
    entry!("windows1256", "Arabic (Windows 1256)", "Windows 1256", Codec::Table(encoding_rs::WINDOWS_1256)),
    entry!("iso88596", "Arabic (ISO 8859-6)", "ISO 8859-6", Codec::Table(encoding_rs::ISO_8859_6)),
    entry!("windows1257", "Baltic (Windows 1257)", "Windows 1257", Codec::Table(encoding_rs::WINDOWS_1257)),
    entry!("iso88594", "Baltic (ISO 8859-4)", "ISO 8859-4", Codec::Table(encoding_rs::ISO_8859_4)),
    entry!("iso885914", "Celtic (ISO 8859-14)", "ISO 8859-14", Codec::Table(encoding_rs::ISO_8859_14)),
    entry!("windows1250", "Central European (Windows 1250)", "Windows 1250", Codec::Table(encoding_rs::WINDOWS_1250)),
    entry!("iso88592", "Central European (ISO 8859-2)", "ISO 8859-2", Codec::Table(encoding_rs::ISO_8859_2)),
    entry!("windows1251", "Cyrillic (Windows 1251)", "Windows 1251", Codec::Table(encoding_rs::WINDOWS_1251)),
    entry!("cp866", "Cyrillic (CP 866)", "CP 866", Codec::Table(encoding_rs::IBM866)),
    entry!("iso88595", "Cyrillic (ISO 8859-5)", "ISO 8859-5", Codec::Table(encoding_rs::ISO_8859_5)),
    entry!("koi8r", "Cyrillic (KOI8-R)", "KOI8-R", Codec::Table(encoding_rs::KOI8_R)),
    entry!("koi8u", "Cyrillic (KOI8-U)", "KOI8-U", Codec::Table(encoding_rs::KOI8_U)),
    entry!("iso885913", "Estonian (ISO 8859-13)", "ISO 8859-13", Codec::Table(encoding_rs::ISO_8859_13)),
    entry!("windows1253", "Greek (Windows 1253)", "Windows 1253", Codec::Table(encoding_rs::WINDOWS_1253)),
    entry!("iso88597", "Greek (ISO 8859-7)", "ISO 8859-7", Codec::Table(encoding_rs::ISO_8859_7)),
    entry!("windows1255", "Hebrew (Windows 1255)", "Windows 1255", Codec::Table(encoding_rs::WINDOWS_1255)),
    entry!("iso88598", "Hebrew (ISO 8859-8)", "ISO 8859-8", Codec::Table(encoding_rs::ISO_8859_8)),
    entry!("iso885910", "Nordic (ISO 8859-10)", "ISO 8859-10", Codec::Table(encoding_rs::ISO_8859_10)),
    entry!("iso885916", "Romanian (ISO 8859-16)", "ISO 8859-16", Codec::Table(encoding_rs::ISO_8859_16)),
    entry!("windows1254", "Turkish (Windows 1254)", "Windows 1254", Codec::Table(encoding_rs::WINDOWS_1254)),
    entry!("windows1258", "Vietnamese (Windows 1258)", "Windows 1258", Codec::Table(encoding_rs::WINDOWS_1258)),
    entry!("gbk", "Simplified Chinese (GBK)", "GBK", Codec::Table(encoding_rs::GBK)),
    entry!("gb18030", "Simplified Chinese (GB18030)", "GB18030", Codec::Table(encoding_rs::GB18030)),
    entry!("cp950", "Traditional Chinese (Big5)", "Big5", Codec::Table(encoding_rs::BIG5)),
    entry!("shiftjis", "Japanese (Shift JIS)", "Shift JIS", Codec::Table(encoding_rs::SHIFT_JIS)),
    entry!("eucjp", "Japanese (EUC-JP)", "EUC-JP", Codec::Table(encoding_rs::EUC_JP)),
    entry!("euckr", "Korean (EUC-KR)", "EUC-KR", Codec::Table(encoding_rs::EUC_KR)),
    entry!("windows874", "Thai (Windows 874)", "Windows 874", Codec::Table(encoding_rs::WINDOWS_874)),
];

fn entry(id: &str) -> &'static Entry {
    ENTRIES.iter().find(|e| e.id == id).unwrap_or(&ENTRIES[0])
}

/// `labelShort`, as the status bar shows it.
pub fn short_label(id: &str) -> &'static str {
    entry(id).short
}

/// The picks of "Select File Encoding to …": `(id, labelLong)` in VS Code's
/// order; `encodeOnly` ones are left out when `reopen` (decoding).
pub fn choices(reopen: bool) -> Vec<(&'static str, &'static str)> {
    ENTRIES.iter().filter(|e| !(reopen && e.encode_only)).map(|e| (e.id, e.long)).collect()
}

/// The id VS Code treats as the same pick (`alias`): `utf8` ↔ `utf8bom`.
pub fn alias(id: &str) -> Option<&'static str> {
    match id {
        UTF8 => Some(UTF8_BOM),
        UTF8_BOM => Some(UTF8),
        _ => None,
    }
}

const BOM_UTF8: [u8; 3] = [0xEF, 0xBB, 0xBF];
const BOM_LE: [u8; 2] = [0xFF, 0xFE];
const BOM_BE: [u8; 2] = [0xFE, 0xFF];

/// Reads a file's bytes the way VS Code's default does (no auto-guess): a BOM
/// picks UTF-8 with BOM / UTF-16 and is dropped from the text, anything
/// else is UTF-8, with U+FFFD for bytes that aren't.
pub fn decode(bytes: &[u8]) -> (String, &'static str) {
    if bytes.starts_with(&BOM_UTF8) {
        return (decode_as(bytes, UTF8_BOM), UTF8_BOM);
    }
    if bytes.starts_with(&BOM_LE) {
        return (decode_as(bytes, "utf16le"), "utf16le");
    }
    if bytes.starts_with(&BOM_BE) {
        return (decode_as(bytes, "utf16be"), "utf16be");
    }
    (decode_as(bytes, UTF8), UTF8)
}

/// Decodes `bytes` as encoding `id` (a leading BOM of that encoding is
/// dropped).
pub fn decode_as(bytes: &[u8], id: &str) -> String {
    match &entry(id).codec {
        Codec::Utf8 | Codec::Utf8Bom => String::from_utf8_lossy(bytes.strip_prefix(&BOM_UTF8).unwrap_or(bytes)).into_owned(),
        Codec::Utf16Le => utf16(bytes.strip_prefix(&BOM_LE).unwrap_or(bytes), u16::from_le_bytes),
        Codec::Utf16Be => utf16(bytes.strip_prefix(&BOM_BE).unwrap_or(bytes), u16::from_be_bytes),
        Codec::Latin1 => bytes.iter().map(|&b| b as char).collect(),
        Codec::Table(enc) => enc.decode_without_bom_handling(bytes).0.into_owned(),
    }
}

fn utf16(bytes: &[u8], unit: fn([u8; 2]) -> u16) -> String {
    let units: Vec<u16> = bytes.chunks_exact(2).map(|c| unit([c[0], c[1]])).collect();
    String::from_utf16_lossy(&units)
}

/// The bytes written for `text` in encoding `id` (BOM included for
/// `utf8bom` / `utf16*`).
pub fn encode(text: &str, id: &str) -> Vec<u8> {
    match &entry(id).codec {
        Codec::Utf8 => text.as_bytes().to_vec(),
        Codec::Utf8Bom => [&BOM_UTF8[..], text.as_bytes()].concat(),
        Codec::Utf16Le => [&BOM_LE[..], &text.encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<_>>()].concat(),
        Codec::Utf16Be => [&BOM_BE[..], &text.encode_utf16().flat_map(u16::to_be_bytes).collect::<Vec<_>>()].concat(),
        Codec::Latin1 => text.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect(),
        Codec::Table(enc) => {
            let mut out = Vec::with_capacity(text.len());
            let mut encoder = enc.new_encoder();
            // Per character: unmappable ones become `?` instead of `&#…;`.
            let mut one = String::new();
            for c in text.chars() {
                one.clear();
                one.push(c);
                let mut bytes = Vec::with_capacity(16);
                let (result, _) = encoder.encode_from_utf8_to_vec_without_replacement(&one, &mut bytes, true);
                if matches!(result, encoding_rs::EncoderResult::InputEmpty) {
                    out.extend_from_slice(&bytes);
                } else {
                    out.push(b'?');
                }
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boms_pick_the_encoding_and_leave_the_text() {
        assert_eq!(decode(b"\xEF\xBB\xBFhi"), ("hi".to_string(), UTF8_BOM));
        assert_eq!(decode(b"\xFF\xFEh\0i\0"), ("hi".to_string(), "utf16le"));
        assert_eq!(decode(b"\xFE\xFF\0h\0i"), ("hi".to_string(), "utf16be"));
        assert_eq!(decode("héllo".as_bytes()), ("héllo".to_string(), UTF8));
        // Not UTF-8: replacement characters, still UTF-8 (files.autoGuessEncoding off).
        assert_eq!(decode(b"a\xFFb").0, "a\u{FFFD}b");
    }

    #[test]
    fn round_trips_through_the_picked_encoding() {
        let cases = [
            ("utf8bom", "Привет"),
            ("utf16le", "Привет 😀"),
            ("utf16be", "Привет 😀"),
            ("windows1251", "Привет"),
            ("cp866", "Привет"),
            ("koi8r", "Привет"),
            ("iso88591", "Café"),
            ("shiftjis", "日本語"),
            ("gbk", "中文"),
        ];
        for (id, text) in cases {
            assert_eq!(decode_as(&encode(text, id), id), text, "{id}");
        }
        assert_eq!(encode("Привет", "windows1251"), [0xCF, 0xF0, 0xE8, 0xE2, 0xE5, 0xF2]);
        assert_eq!(encode("é", "iso88591"), [0xE9]);
    }

    #[test]
    fn unmappable_characters_become_question_marks() {
        assert_eq!(encode("a€b", "iso88591"), b"a?b");
        assert_eq!(encode("aЖb", "windows1252"), b"a?b");
    }

    #[test]
    fn utf16_files_start_with_their_bom() {
        assert_eq!(encode("a", "utf16le"), [0xFF, 0xFE, b'a', 0]);
        assert_eq!(encode("a", "utf16be"), [0xFE, 0xFF, 0, b'a']);
        assert_eq!(encode("a", "utf8bom"), [0xEF, 0xBB, 0xBF, b'a']);
        assert_eq!(encode("a", "utf8"), [b'a']);
    }

    #[test]
    fn reopening_hides_the_encode_only_pick() {
        assert!(choices(false).iter().any(|(id, _)| *id == "utf8bom"));
        assert!(!choices(true).iter().any(|(id, _)| *id == "utf8bom"));
        assert_eq!(short_label("windows1251"), "Windows 1251");
    }
}
