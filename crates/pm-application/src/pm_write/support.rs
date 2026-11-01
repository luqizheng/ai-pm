use super::*;

/// 小写 hex 的 sha256（pm-domain 同款摘要约定，无 hex 依赖时手写编码）。
pub(crate) fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::hex_sha256;

    #[test]
    fn hex_sha256_is_lowercase_hex_of_sha256() {
        // echo -n "abc" | sha256sum
        assert_eq!(
            hex_sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}

