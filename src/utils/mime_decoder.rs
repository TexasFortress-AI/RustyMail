use regex::Regex;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

/// Decode MIME encoded-word headers (RFC 2047)
/// Supports both Q-encoding and B-encoding
/// Format: =?charset?encoding?encoded-text?=
pub fn decode_mime_header(input: &str) -> String {
    lazy_static::lazy_static! {
        static ref ENCODED_WORD_RE: Regex = Regex::new(
            r"=\?([^?]+)\?([BbQq])\?([^?]+)\?="
        ).unwrap();
    }

    let mut result = String::new();
    let mut last_end = 0;

    for cap in ENCODED_WORD_RE.captures_iter(input) {
        let (full_match, [_charset, encoding, encoded_text]) = cap.extract();
        let start = cap.get(0).unwrap().start();
        let end = cap.get(0).unwrap().end();

        // Add any text before the encoded word
        if start > last_end {
            result.push_str(&input[last_end..start]);
        }

        // Decode the encoded word
        let decoded = match encoding.to_uppercase().as_str() {
            "B" => decode_base64(encoded_text),
            "Q" => decode_quoted_printable(encoded_text),
            _ => full_match.to_string(),
        };

        result.push_str(&decoded);
        last_end = end;
    }

    // Add any remaining text after the last encoded word
    if last_end < input.len() {
        result.push_str(&input[last_end..]);
    }

    // If no encoded words were found, return the original string
    if last_end == 0 {
        input.to_string()
    } else {
        result
    }
}

fn decode_base64(encoded: &str) -> String {
    BASE64.decode(encoded)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_else(|| encoded.to_string())
}

fn decode_quoted_printable(encoded: &str) -> String {
    let mut result = Vec::new();
    let bytes = encoded.bytes().collect::<Vec<_>>();
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'=' if i + 2 < bytes.len() => {
                // Check if it's a hex sequence
                if let (Ok(h1), Ok(h2)) = (
                    std::str::from_utf8(&[bytes[i + 1]]),
                    std::str::from_utf8(&[bytes[i + 2]])
                ) {
                    if let Ok(byte) = u8::from_str_radix(&format!("{}{}", h1, h2), 16) {
                        result.push(byte);
                        i += 3;
                        continue;
                    }
                }
                // Not a valid hex sequence, treat as literal
                result.push(bytes[i]);
                i += 1;
            }
            b'_' => {
                // In Q-encoding, underscore represents space
                result.push(b' ');
                i += 1;
            }
            _ => {
                result.push(bytes[i]);
                i += 1;
            }
        }
    }

    String::from_utf8(result).unwrap_or_else(|_| encoded.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_q_encoding() {
        let input = "=?UTF-8?Q?We=E2=80=99re_Updating_our_Consumer_Terms_and_Privacy_Policy?=";
        let expected = "We're Updating our Consumer Terms and Privacy Policy";
        assert_eq!(decode_mime_header(input), expected);
    }

    #[test]
    fn test_decode_b_encoding() {
        let input = "=?UTF-8?B?V2XigJlyZSBVcGRhdGluZyBvdXIgQ29uc3VtZXIgVGVybXMgYW5kIFByaXZhY3kgUG9saWN5?=";
        let expected = "We're Updating our Consumer Terms and Privacy Policy";
        assert_eq!(decode_mime_header(input), expected);
    }

    #[test]
    fn test_plain_text() {
        let input = "This is plain text";
        assert_eq!(decode_mime_header(input), input);
    }

    #[test]
    fn test_mixed_encoded_plain() {
        let input = "Re: =?UTF-8?Q?Test=20Message?= from sender";
        let expected = "Re: Test Message from sender";
        assert_eq!(decode_mime_header(input), expected);
    }
}