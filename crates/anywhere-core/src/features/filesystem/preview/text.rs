//! Bounded decoding only. Detection is a hint, never a reason to treat binary as text.
use chardetng::{EncodingDetector, Iso2022JpDetection, Utf8Detection};
use encoding_rs::{DecoderResult, Encoding, UTF_8};

pub(super) struct DecodedText {
    pub text: String,
    pub encoding: &'static str,
    pub guessed: bool,
    pub truncated: bool,
}

pub(super) fn decode(bytes: &[u8], truncated: bool, allow_legacy: bool) -> Option<DecodedText> {
    // encoding_rs intentionally does not support UTF-32. Do not mistake LE's
    // prefix for a UTF-16 BOM and produce a misleading partial preview.
    if bytes.starts_with(&[0xff, 0xfe, 0, 0]) || bytes.starts_with(&[0, 0, 0xfe, 0xff]) {
        return None;
    }
    let (encoding, skip, guessed) = if let Some((encoding, skip)) = Encoding::for_bom(bytes) {
        (encoding, skip, false)
    } else {
        match std::str::from_utf8(bytes) {
            Ok(_) => (UTF_8, 0, false),
            Err(error) if truncated && error.error_len().is_none() => (UTF_8, 0, false),
            Err(_)
                if allow_legacy
                    && !bytes
                        .iter()
                        .any(|b| b.is_ascii_control() && !matches!(b, b'\n' | b'\r' | b'\t')) =>
            {
                let mut detector = EncodingDetector::new(Iso2022JpDetection::Deny);
                detector.feed(bytes, !truncated);
                (detector.guess(None, Utf8Detection::Deny), 0, true)
            }
            Err(_) => return None,
        }
    };
    let mut decoder = encoding.new_decoder_without_bom_handling();
    // Bound output as well as input: a legacy or UTF-16 source can expand in UTF-8.
    let mut output = vec![0; bytes.len().saturating_mul(3).min(super::MAX_PREVIEW_BYTES)];
    let (result, _, written) =
        decoder.decode_to_utf8_without_replacement(&bytes[skip..], &mut output, !truncated);
    if matches!(result, DecoderResult::Malformed(_, _)) {
        return None;
    }
    output.truncate(written);
    let text = String::from_utf8(output).ok()?;
    if text
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return None;
    }
    Some(DecodedText {
        text,
        encoding: encoding.name(),
        guessed,
        truncated: truncated || matches!(result, DecoderResult::OutputFull),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use encoding_rs::{GBK, UTF_16BE, UTF_16LE};
    fn utf16(text: &str, little_endian: bool) -> Vec<u8> {
        let mut bytes = if little_endian {
            vec![0xff, 0xfe]
        } else {
            vec![0xfe, 0xff]
        };
        for unit in text.encode_utf16() {
            bytes.extend(if little_endian {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            });
        }
        bytes
    }
    #[test]
    fn bom_utf16_and_utf8_are_explicit_and_unicode_safe() {
        let source = "项目配置 café 😀\r\nnext";
        for (little, encoding) in [(true, UTF_16LE), (false, UTF_16BE)] {
            let result = decode(&utf16(source, little), false, false).unwrap();
            assert_eq!(result.text, source);
            assert_eq!(result.encoding, encoding.name());
            assert!(!result.guessed && !result.truncated);
        }
        let result = decode("\u{feff}你好".as_bytes(), false, false).unwrap();
        assert_eq!(result.text, "你好");
        assert_eq!(result.encoding, "UTF-8");
        assert!(decode(b"", false, false).unwrap().text.is_empty());
    }
    #[test]
    fn guesses_gbk_only_when_the_file_is_a_text_candidate() {
        let source = "这是项目的配置文件，包含中文路径和数据库连接信息。\n".repeat(10);
        let (bytes, _, errors) = GBK.encode(&source);
        assert!(!errors);
        assert!(decode(&bytes, false, false).is_none());
        let result = decode(&bytes, false, true).unwrap();
        assert_eq!(result.encoding, "GBK");
        assert_eq!(result.text, source);
        assert!(result.guessed);
    }
    #[test]
    fn binary_and_malformed_bom_sources_are_not_lossily_rendered() {
        for bytes in [
            &[0, 1, 2, 255][..],
            &[0xff, 0xfe, 0x61][..],
            &[0xff, 0xfe, 0, 0, 65, 0, 0, 0][..],
            &[0, 0, 0xfe, 0xff][..],
            b"PK\x03\x04binary",
            &[0xff, 0xfe, 0, 0xd8][..],
        ] {
            assert!(decode(bytes, false, true).is_none(), "{bytes:?}");
        }
        assert!(decode(&utf16("a\0b", true), false, true).is_none());
    }
    #[test]
    fn input_cut_and_expanding_output_remain_bounded_without_replacement_chars() {
        let mut bytes = utf16(&"中".repeat(super::super::MAX_PREVIEW_BYTES), true);
        bytes.truncate(super::super::MAX_PREVIEW_BYTES);
        let result = decode(&bytes, true, false).unwrap();
        assert!(result.text.len() <= super::super::MAX_PREVIEW_BYTES);
        assert!(result.truncated && !result.text.contains('�'));
        let bytes = [0xff, 0xfe, b'a', 0, 0x3d, 0xd8];
        assert_eq!(decode(&bytes, true, false).unwrap().text, "a");
        assert!(decode(&bytes, false, false).is_none());
        assert_eq!(decode(&[b'a', 0xe4, 0xb8], true, false).unwrap().text, "a");
    }
}
