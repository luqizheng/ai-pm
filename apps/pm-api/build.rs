//! pm-api build script：把仓库根 `skills/` 打包为 zip，供 `include_bytes!` 内嵌。
//!
//! 白名单（设计稿 §3.1 目录结构）：四份 SKILL.md + scripts/verify-install.*；
//! 排除临时文件 / .DS_Store / 任何非白名单内容。每次 `skills/` 变化触发重打包
//! （cargo:rerun-if-changed）。
//!
//! 依赖：标准库 + 手写 STORE（无压缩）ZIP writer——避免为打包引入 zip crate
//! 依赖（构建依赖面最小；无压缩对纯文本包无害）。

use std::fs;
use std::path::{Path, PathBuf};

const ENTRIES: &[&str] = &[
    "skills/pm-ai-bootstrap/SKILL.md",
    "skills/pm-task-execution/SKILL.md",
    "skills/pm-knowledge/SKILL.md",
    "skills/pm-learn/SKILL.md",
    "skills/scripts/verify-install.ps1",
    "skills/scripts/verify-install.sh",
];

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf();
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("out dir"));

    println!("cargo:rerun-if-changed=skills/");
    for entry in ENTRIES {
        println!("cargo:rerun-if-changed=../{entry}");
    }

    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for entry in ENTRIES {
        let path = workspace_root.join(entry);
        let bytes = fs::read(&path)
            .unwrap_or_else(|e| panic!("skills package entry {entry} unreadable: {e}"));
        files.push((entry.to_string(), bytes));
    }

    let zip = build_store_zip(&files);
    let dest = out_dir.join("pm-ai-skills.zip");
    fs::write(&dest, zip).expect("write pm-ai-skills.zip");
}

/// 最小 STORE（无压缩）ZIP writer：local file headers + central directory。
/// CRC32 用标准多项式表驱动实现。
fn build_store_zip(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut central: Vec<u8> = Vec::new();

    for (name, data) in files {
        let crc = crc32(data);
        let offset = out.len() as u32;
        let name_bytes = name.as_bytes();

        // Local file header
        out.extend_from_slice(&0x04034b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0u16.to_le_bytes()); // flags
        out.extend_from_slice(&0u16.to_le_bytes()); // method = store
        out.extend_from_slice(&0u16.to_le_bytes()); // mtime
        out.extend_from_slice(&0x21u16.to_le_bytes()); // mdate (1980-01-01)
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra len
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(data);

        // Central directory entry
        central.extend_from_slice(&0x02014b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes()); // version made by
        central.extend_from_slice(&20u16.to_le_bytes()); // version needed
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes()); // extra
        central.extend_from_slice(&0u16.to_le_bytes()); // comment
        central.extend_from_slice(&0u16.to_le_bytes()); // disk
        central.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        central.extend_from_slice(&0u32.to_le_bytes()); // external attrs
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name_bytes);
    }

    let central_offset = out.len() as u32;
    let central_size = central.len() as u32;
    out.extend_from_slice(&central);

    // End of central directory
    out.extend_from_slice(&0x06054b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&central_size.to_le_bytes());
    out.extend_from_slice(&central_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, slot) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *slot = c;
    }
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}
