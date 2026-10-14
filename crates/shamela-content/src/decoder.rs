//! Substitution-cipher decoder for Shamela desktop service databases.
//!
//! The desktop `S1.db` table `b` stores narrator names, biographies and
//! metadata as BLOBs where each byte maps to an Arabic character, a
//! structural marker, or a stripable control byte.
//!
//! Mapping ported from the MIT-licensed
//! [ragaeeb/shamela](https://github.com/ragaeeb/shamela/blob/main/scripts/shamela-decoder.ts)
//! (`SHAMELA_CHAR_MAP` / `METADATA_CHAR_MAP`), re-implemented in Rust.

/// Map one encoded byte to its output. `None` means unmapped.
fn map_byte(byte: u8, metadata: bool) -> Option<&'static str> {
    Some(match byte {
        // Control bytes (stripped).
        20 | 74 | 75 | 127 | 129 | 132 | 133 | 134 | 135 | 136 | 138 | 139 | 149 | 153 | 162
        | 164 | 166 | 168 | 192 | 208 => "",
        // Space variants.
        64 | 155 => " ",
        // Core letters.
        67 => "ن",
        69 => "ل",
        70 => "م",
        71 => "ه",
        85 => "ي",
        88 => "ى",
        89 => "ك",
        96 | 148 | 156 | 171 => "و",
        // Punctuation / structure.
        77 => "(",
        79 => "\"",
        90 | 107 | 224 => {
            if metadata && byte == 107 {
                ":"
            } else {
                "\n"
            }
        }
        91 => "[",
        93 => ")",
        97 => "/",
        122 => ":",
        // Hamza variants.
        98 => "آ",
        99 => "ؤ",
        158 => "ئ",
        102 => "أ",
        103 => "إ",
        // Remaining letters.
        101 | 113 => "ة",
        104 => "ا",
        105 => "ر",
        114 => "ت",
        115 | 121 => "ث",
        116 => "ب",
        117 => "ح",
        118 => "خ",
        119 => "د",
        120 => "ج",
        128 => "ط",
        170 => "،",
        172 => "ذ",
        173 => "ف",
        174 => "ق",
        235 => "ش",
        236 => "ض",
        237 => "ز",
        238 => "س",
        239 => "ص",
        251 | 255 => "غ",
        252 => "ـ",
        253 => "ظ",
        254 => "ع",
        // Tashkeel.
        112 => "\u{651}",
        204 => "\u{650}",
        205 => "\u{64d}",
        206 => "\u{64e}",
        207 => "\u{64f}",
        222 => "\u{652}",
        // Digits.
        240..=249 => match byte {
            240 => "0",
            241 => "1",
            242 => "2",
            243 => "3",
            244 => "4",
            245 => "5",
            246 => "6",
            247 => "7",
            248 => "8",
            _ => "9",
        },
        _ => return None,
    })
}

fn decode_bytes_inner(blob: Option<&[u8]>, metadata: bool) -> String {
    let Some(bytes) = blob else {
        return String::new();
    };
    let mut out = String::new();
    for byte in bytes {
        match map_byte(*byte, metadata) {
            Some(mapped) => out.push_str(mapped),
            None => out.push_str(&format!("[{byte:02x}]")),
        }
    }
    out.trim().to_string()
}

/// Decode a Shamela-encoded blob (names, biography text).
///
/// Unmapped bytes are preserved as lowercase `[xx]` placeholders so gaps
/// stay visible, mirroring the reference implementation.
pub fn decode_bytes(blob: Option<&[u8]>) -> String {
    decode_bytes_inner(blob, false)
}

/// Alias for [`decode_bytes`] for call sites handling prose text.
pub fn decode_text(blob: Option<&[u8]>) -> String {
    decode_bytes(blob)
}

/// Decode metadata blobs, where `0x6b` separates field labels, then clean
/// the structural artifacts (`": "` at the start, `"\n: "` separators).
pub fn decode_metadata(blob: Option<&[u8]>) -> String {
    let Some(bytes) = blob else {
        return String::new();
    };
    if bytes.is_empty() {
        return String::new();
    }
    let mut result = decode_bytes_inner(Some(bytes), true);
    result = result
        .strip_prefix(':')
        .map(|rest| rest.trim_start().to_string())
        .unwrap_or(result);
    result = result.replace("\n:", "\n").replace("\n ", "\n");
    result.trim().to_string()
}

/// True when decoded text still holds `[xx]` placeholders.
pub fn has_unmapped(decoded: &str) -> bool {
    find_placeholders(decoded).next().is_some()
}

/// Unique unmapped byte values present as `[xx]` placeholders, in order.
pub fn unmapped_bytes(decoded: &str) -> Vec<u8> {
    let mut seen = [false; 256];
    let mut out = Vec::new();
    for byte in find_placeholders(decoded) {
        if !seen[byte as usize] {
            seen[byte as usize] = true;
            out.push(byte);
        }
    }
    out
}

fn find_placeholders(decoded: &str) -> impl Iterator<Item = u8> + '_ {
    let bytes = decoded.as_bytes();
    let mut items = Vec::new();
    let mut index = 0;
    while index + 3 < bytes.len() {
        if bytes[index] == b'['
            && bytes[index + 3] == b']'
            && bytes[index + 1].is_ascii_hexdigit()
            && bytes[index + 2].is_ascii_hexdigit()
        {
            if let Ok(value) = u8::from_str_radix(&decoded[index + 1..index + 3], 16) {
                items.push(value);
            }
            index += 4;
        } else {
            index += 1;
        }
    }
    items.into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_allah() {
        // Reference vector: الله == [0x68, 0x45, 0x45, 0x47].
        assert_eq!(decode_bytes(Some(&[0x68, 0x45, 0x45, 0x47])), "الله");
    }

    #[test]
    fn decodes_adam_son_pattern() {
        // آدم == [0x62, 0x77, 0x46]; بن == [0x74, 0x43].
        assert_eq!(
            decode_bytes(Some(&[0x62, 0x77, 0x46, 0x40, 0x74, 0x43])),
            "آدم بن"
        );
    }

    #[test]
    fn preserves_unmapped_as_placeholders() {
        assert_eq!(decode_bytes(Some(&[0x01])), "[01]");
        assert!(has_unmapped("نص [ab]"));
        assert!(!has_unmapped("نص سليم"));
        assert_eq!(unmapped_bytes("x [ab] y [AB] z [cd]"), vec![0xAB, 0xCD]);
    }

    #[test]
    fn metadata_separator_behavior() {
        // 0x6b is a newline in prose but ':' in metadata.
        assert_eq!(decode_bytes(Some(&[0x74, 0x6B, 0x74])), "ب\nب");
        assert_eq!(decode_metadata(Some(&[0x74, 0x6B, 0x74])), "ب:ب");
    }

    #[test]
    fn null_and_empty() {
        assert_eq!(decode_bytes(None), "");
        assert_eq!(decode_metadata(None), "");
        assert_eq!(decode_metadata(Some(&[])), "");
    }
}
