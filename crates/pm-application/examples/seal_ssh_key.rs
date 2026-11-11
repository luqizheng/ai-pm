//! E2E 测试助手：把外部私钥按平台信封格式加密，输出 hex 密文。
//!
//! 为什么存在：平台**没有**「导入外部私钥」的 API——`integration.project_ssh_key`
//! 的私钥只由 `ensure_ssh_key` 现场生成。E2E 要用本机真实私钥访问真实 Git 远端，
//! 必须把私钥按平台同一条加密路径（[`secret::seal_text`]）封装后经 SQL 装进密钥库。
//! 本例子的意义在于**不重实现加密**：Node 侧自己写 AES-256-GCM 信封 = 两套实现
//! 漂移 = 假绿或假红（`docs/e2e-spec/00-e2e-testing-framework-v1.md` §7.2 红线）。
//!
//! 用法：
//! ```text
//! cargo run -q -p pm-application --example seal_ssh_key -- <project_id> <key_file>
//! ```
//! 环境变量 `AI_PM_SECRET_KEY` 必须与被测 pm-api 进程一致（信封主密钥 = sha256(该值)）。
//! stdout 最后一行 = hex 密文；psql 侧用 `decode('<hex>', 'hex')` 落库。

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: seal_ssh_key <project_id> <key_file>");
        eprintln!("env:   AI_PM_SECRET_KEY must match the pm-api under test");
        return ExitCode::FAILURE;
    }
    let Ok(project_id) = uuid::Uuid::parse_str(&args[1]) else {
        eprintln!("seal_ssh_key: `<project_id>` is not a uuid: {}", args[1]);
        return ExitCode::FAILURE;
    };
    let Ok(private) = std::fs::read_to_string(&args[2]) else {
        eprintln!(
            "seal_ssh_key: cannot read key file {}",
            args[2]
        );
        return ExitCode::FAILURE;
    };

    match pm_application::secret::seal_text(project_id, private.as_bytes()) {
        Ok(ciphertext) => {
            let hex: String = ciphertext.iter().map(|b| format!("{b:02x}")).collect();
            println!("{hex}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("seal_ssh_key: seal failed: {err}");
            ExitCode::FAILURE
        }
    }
}
