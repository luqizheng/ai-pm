//! 项目级密钥材料：ed25519 生成 / 指纹 / AES-256-GCM 密文盒。
//!
//! 与 `llm_config.rs` 同口径：主密钥 = `sha256(AI_PM_SECRET_KEY)`，密文格式为
//! `nonce(12) || ciphertext || tag(16)`，AAD 绑定项目 id 防止跨项目搬运。
//! 私钥明文只在生成进程内短暂存在，临时目录用后即删。

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const NONCE_LEN: usize = 12;

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("AI_PM_SECRET_KEY is not set")]
    KeyMissing,
    #[error("secret could not be encrypted or decrypted")]
    Crypto,
    #[error("ssh-keygen is not available: {0}")]
    SshKeygen(String),
    /// 导入的外部私钥无效（格式错误 / 带口令）——用户可修正的输入错误，
    /// 与「ssh-keygen 二进制不可用」（SshKeygen）区分开。
    #[error("private key is invalid or passphrase-protected: {0}")]
    InvalidKey(String),
    #[error("ssh-keygen produced no fingerprint")]
    Fingerprint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedKey {
    pub public_key: String,
    pub private_key: String,
    pub fingerprint: String,
}

/// 从导入的外部私钥推导出的公钥材料。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedKey {
    pub public_key: String,
    pub fingerprint: String,
    /// 推导公钥的首 token，如 `ssh-ed25519` / `ssh-rsa` / `ecdsa-sha2-nistp256`。
    pub key_type: String,
}

fn cipher() -> Result<Aes256Gcm, SecretError> {
    let raw = std::env::var("AI_PM_SECRET_KEY").map_err(|_| SecretError::KeyMissing)?;
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(SecretError::KeyMissing);
    }
    let key: [u8; 32] = Sha256::digest(raw.as_bytes()).into();
    Aes256Gcm::new_from_slice(&key).map_err(|_| SecretError::Crypto)
}

/// 加密任意字节；AAD 为项目 id。
pub fn seal_text(project_id: Uuid, plaintext: &[u8]) -> Result<Vec<u8>, SecretError> {
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce).map_err(|_| SecretError::Crypto)?;
    let encrypted = cipher()?
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: project_id.as_bytes(),
            },
        )
        .map_err(|_| SecretError::Crypto)?;
    let mut out = Vec::with_capacity(NONCE_LEN + encrypted.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&encrypted);
    Ok(out)
}

/// 解密 [`seal_text`] 的产物；项目 id 必须与加密时一致。
pub fn open_text(project_id: Uuid, payload: &[u8]) -> Result<Vec<u8>, SecretError> {
    if payload.len() <= NONCE_LEN {
        return Err(SecretError::Crypto);
    }
    let (nonce, ciphertext) = payload.split_at(NONCE_LEN);
    cipher()?
        .decrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad: project_id.as_bytes(),
            },
        )
        .map_err(|_| SecretError::Crypto)
}

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 生成 ed25519 密钥对；返回后临时目录（含私钥文件）已删除。
pub fn generate_ed25519(comment: &str) -> Result<GeneratedKey, SecretError> {
    let dir = TempDir(std::env::temp_dir().join(format!("ai-pm-ssh-{}", Uuid::new_v4())));
    std::fs::create_dir_all(&dir.0).map_err(|e| SecretError::SshKeygen(e.to_string()))?;
    let key_path = dir.0.join("id_ed25519");

    let output = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-C", comment, "-f"])
        .arg(&key_path)
        .output()
        .map_err(|e| SecretError::SshKeygen(e.to_string()))?;
    if !output.status.success() {
        return Err(SecretError::SshKeygen(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }

    let pub_path = key_path.with_extension("pub");
    let public_key = std::fs::read_to_string(&pub_path)
        .map_err(|e| SecretError::SshKeygen(e.to_string()))?
        .trim()
        .to_string();
    let private_key =
        std::fs::read_to_string(&key_path).map_err(|e| SecretError::SshKeygen(e.to_string()))?;
    let fingerprint = fingerprint_of(&pub_path)?;

    Ok(GeneratedKey {
        public_key,
        private_key,
        fingerprint,
    })
}

/// 规范粘贴内容：CRLF → LF、去首尾空白、补尾换行（ssh-keygen 逐行解析）。
pub fn normalize_private_key(private_key: &str) -> String {
    let trimmed = private_key.replace("\r\n", "\n").trim().to_string();
    format!("{trimmed}\n")
}

/// 从外部私钥推导公钥材料（导入粘贴私钥的校验通道）：`ssh-keygen -y`。
/// 带口令的私钥在 stdin 关闭时推导失败（不接受交互），视同无效。
/// 返回后临时目录（含私钥明文文件）已删除。
pub fn derive_public_key(private_key: &str) -> Result<DerivedKey, SecretError> {
    let normalized = normalize_private_key(private_key);
    if !normalized.contains("PRIVATE KEY") {
        return Err(SecretError::InvalidKey("not a private key".to_string()));
    }
    let dir = TempDir(std::env::temp_dir().join(format!("ai-pm-ssh-{}", Uuid::new_v4())));
    std::fs::create_dir_all(&dir.0).map_err(|e| SecretError::SshKeygen(e.to_string()))?;
    let key_path = dir.0.join("id_imported");
    std::fs::write(&key_path, normalized.as_bytes())
        .map_err(|e| SecretError::SshKeygen(e.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| SecretError::SshKeygen(e.to_string()))?;
    }

    let output = Command::new("ssh-keygen")
        .arg("-y")
        .arg("-f")
        .arg(&key_path)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| SecretError::SshKeygen(e.to_string()))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(SecretError::InvalidKey(detail));
    }
    let public_key = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let key_type = public_key
        .split_whitespace()
        .next()
        .filter(|t| !t.is_empty())
        .ok_or_else(|| SecretError::InvalidKey("ssh-keygen -y produced no key".to_string()))?
        .to_string();

    let pub_path = dir.0.join("id_imported.pub");
    std::fs::write(&pub_path, format!("{public_key}\n"))
        .map_err(|e| SecretError::SshKeygen(e.to_string()))?;
    let fingerprint = fingerprint_of(&pub_path)?;

    Ok(DerivedKey {
        public_key,
        fingerprint,
        key_type,
    })
}

/// `ssh-keygen -lf <pub>` 的第二列，例如 `SHA256:...`。
pub fn fingerprint_of(pub_path: &Path) -> Result<String, SecretError> {
    let output = Command::new("ssh-keygen")
        .arg("-lf")
        .arg(pub_path)
        .output()
        .map_err(|e| SecretError::SshKeygen(e.to_string()))?;
    if !output.status.success() {
        return Err(SecretError::SshKeygen(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .nth(1)
        .map(str::to_string)
        .ok_or(SecretError::Fingerprint)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_secret<T>(value: Option<&str>, f: impl FnOnce() -> T) -> T {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let previous = std::env::var("AI_PM_SECRET_KEY").ok();
        match value {
            Some(v) => std::env::set_var("AI_PM_SECRET_KEY", v),
            None => std::env::remove_var("AI_PM_SECRET_KEY"),
        }
        let out = f();
        match previous {
            Some(v) => std::env::set_var("AI_PM_SECRET_KEY", v),
            None => std::env::remove_var("AI_PM_SECRET_KEY"),
        }
        out
    }

    fn project() -> Uuid {
        Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap()
    }

    #[test]
    fn seal_and_open_roundtrip() {
        with_secret(Some("test-secret-key"), || {
            let sealed = seal_text(project(), b"private-key-material").unwrap();
            let opened = open_text(project(), &sealed).unwrap();
            assert_eq!(opened, b"private-key-material");
        });
    }

    #[test]
    fn tampering_one_byte_fails() {
        with_secret(Some("test-secret-key"), || {
            let mut sealed = seal_text(project(), b"private-key-material").unwrap();
            let last = sealed.len() - 1;
            sealed[last] ^= 0xff;
            assert!(matches!(
                open_text(project(), &sealed),
                Err(SecretError::Crypto)
            ));
        });
    }

    #[test]
    fn different_project_aad_fails() {
        with_secret(Some("test-secret-key"), || {
            let sealed = seal_text(project(), b"private-key-material").unwrap();
            let other = Uuid::parse_str("22222222-2222-4222-8222-222222222222").unwrap();
            assert!(matches!(
                open_text(other, &sealed),
                Err(SecretError::Crypto)
            ));
        });
    }

    #[test]
    fn missing_master_key_fails_closed() {
        with_secret(None, || {
            assert!(matches!(
                seal_text(project(), b"x"),
                Err(SecretError::KeyMissing)
            ));
        });
    }

    #[test]
    fn generated_key_shape_and_fingerprint() {
        if Command::new("ssh-keygen").arg("-V").output().is_err() {
            eprintln!("ssh-keygen unavailable; skipping");
            return;
        }
        let key = generate_ed25519("ai-pm:TEST-001").unwrap();
        assert!(key.public_key.starts_with("ssh-ed25519 "));
        assert!(key.fingerprint.starts_with("SHA256:"));
        assert!(key.private_key.contains("PRIVATE KEY"));
        assert!(key.public_key.contains("ai-pm:TEST-001"));
    }

    #[test]
    fn derive_public_key_matches_generated_fingerprint() {
        if Command::new("ssh-keygen").arg("-V").output().is_err() {
            eprintln!("ssh-keygen unavailable; skipping");
            return;
        }
        let generated = generate_ed25519("ai-pm:IMPORT-001").unwrap();
        let derived = derive_public_key(&generated.private_key).unwrap();
        assert_eq!(derived.key_type, "ssh-ed25519");
        assert_eq!(derived.public_key, generated.public_key);
        assert_eq!(derived.fingerprint, generated.fingerprint);
    }

    #[test]
    fn derive_public_key_normalizes_crlf() {
        if Command::new("ssh-keygen").arg("-V").output().is_err() {
            eprintln!("ssh-keygen unavailable; skipping");
            return;
        }
        let generated = generate_ed25519("ai-pm:CRLF-001").unwrap();
        let crlf = format!("  {}\r\n", generated.private_key.replace('\n', "\r\n"));
        assert!(derive_public_key(&crlf).is_ok());
    }

    #[test]
    fn derive_public_key_rejects_garbage() {
        assert!(matches!(
            derive_public_key("definitely not a key"),
            Err(SecretError::InvalidKey(_))
        ));
    }

    #[test]
    fn derive_public_key_rejects_passphrase_key() {
        if Command::new("ssh-keygen").arg("-V").output().is_err() {
            eprintln!("ssh-keygen unavailable; skipping");
            return;
        }
        let dir = TempDir(std::env::temp_dir().join(format!("ai-pm-ssh-pp-{}", Uuid::new_v4())));
        std::fs::create_dir_all(&dir.0).unwrap();
        let key_path = dir.0.join("id_encrypted");
        let output = Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "secret-pass", "-C", "pp", "-f"])
            .arg(&key_path)
            .output()
            .unwrap();
        assert!(output.status.success());
        let pem = std::fs::read_to_string(&key_path).unwrap();
        assert!(matches!(
            derive_public_key(&pem),
            Err(SecretError::InvalidKey(_))
        ));
    }
}
