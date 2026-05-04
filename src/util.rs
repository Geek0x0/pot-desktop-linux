const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-";

fn next_random(bytes: &mut [u8]) {
    getrandom::fill(bytes).unwrap_or_else(|e| {
        // Fallback: time-based if getrandom fails
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = ((t >> (i % 16)) as u8).wrapping_add(i as u8);
        }
        let _ = e;
    });
}

pub fn nanoid(len: usize) -> String {
    let mut buf = vec![0u8; len];
    next_random(&mut buf);
    buf.iter()
        .map(|&byte| ALPHABET[byte as usize % ALPHABET.len()] as char)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_correct_length() {
        assert_eq!(nanoid(6).len(), 6);
        assert_eq!(nanoid(8).len(), 8);
        assert_eq!(nanoid(21).len(), 21);
    }

    #[test]
    fn generates_unique() {
        assert_ne!(nanoid(8), nanoid(8));
    }

    #[test]
    fn uses_valid_chars() {
        let id = nanoid(100);
        for c in id.chars() {
            assert!(c.is_ascii_alphanumeric() || c == '_' || c == '-');
        }
    }
}
