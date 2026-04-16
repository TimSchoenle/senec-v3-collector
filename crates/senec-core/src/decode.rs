pub fn decode_numeric_values(raw: &str) -> Vec<f64> {
    if raw.trim().is_empty() {
        return Vec::new();
    }

    let mut decoded = Vec::new();

    for token in typed_tokens(raw) {
        if let Some(value) = decode_token(&token) {
            decoded.push(value);
        }
    }

    decoded
}

fn typed_tokens(raw: &str) -> Vec<String> {
    raw.split(|c: char| c.is_whitespace() || matches!(c, '[' | ']' | '"' | ','))
        .filter_map(|token| {
            let cleaned = token.trim();
            if cleaned.is_empty() {
                None
            } else {
                Some(cleaned.to_string())
            }
        })
        .collect()
}

fn decode_token(token: &str) -> Option<f64> {
    let (kind, value) = token.split_once('_')?;
    match kind {
        "fl" => u32::from_str_radix(value, 16)
            .ok()
            .map(|bits| f32::from_bits(bits) as f64),
        "u8" | "u1" | "u3" | "u6" => u64::from_str_radix(value, 16).ok().map(|n| n as f64),
        "i8" => decode_signed_hex(value, 8),
        "i1" => decode_signed_hex(value, 16),
        "i3" => decode_signed_hex(value, 32),
        _ => None,
    }
}

fn decode_signed_hex(value: &str, bits: u8) -> Option<f64> {
    let raw = i128::from_str_radix(value, 16).ok()?;
    let sign_bit = 1i128 << (bits - 1);
    let full_range = 1i128 << bits;
    let signed = if raw & sign_bit != 0 {
        raw - full_range
    } else {
        raw
    };
    Some(signed as f64)
}

#[cfg(test)]
mod tests {
    use super::decode_numeric_values;

    #[test]
    fn decodes_float() {
        let values = decode_numeric_values("fl_3F800000");
        assert_eq!(values, vec![1.0]);
    }

    #[test]
    fn decodes_unsigned_values() {
        let values = decode_numeric_values("u8_0A u1_0010 u3_000000FF u6_0000000000000001");
        assert_eq!(values, vec![10.0, 16.0, 255.0, 1.0]);
    }

    #[test]
    fn decodes_signed_values() {
        let values = decode_numeric_values("i1_FFFF i3_FFFFFFFE i8_FF");
        assert_eq!(values, vec![-1.0, -2.0, -1.0]);
    }

    #[test]
    fn ignores_non_numeric_tokens() {
        let values = decode_numeric_values("st_text FORBIDDEN");
        assert!(values.is_empty());
    }

    #[test]
    fn decodes_array_encoded_tokens() {
        let values = decode_numeric_values("[\"fl_3F800000\",\"u1_0002\",\"i1_FFFF\"]");
        assert_eq!(values, vec![1.0, 2.0, -1.0]);
    }
}
