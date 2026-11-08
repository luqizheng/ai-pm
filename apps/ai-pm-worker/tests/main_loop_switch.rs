//! Batch G 遗留⑤（Batch H 清偿）：第七条循环（assistant 会话回复）的
//! `AI_PM_CONVERSATION_REPLY` 开关冒烟。
//!
//! 边界说明：`ai-pm-worker` 没有测试目录，`main.rs` 的循环启动是薄壳——
//! 开关的「读取 → 判定 → 是否进入循环」发生在 `tokio::spawn` 的 async 块
//! 内部，不是可测纯函数；本测试**不为此重构 main**（挂账裁决），只对
//! main.rs 实际组合的纯函数面（`pm_ai::conversation_reply::
//! ConversationReplyMode::parse` + `enabled`，pm-ai 是本二进制的直接依赖）
//! 在 worker 集成边界上钉住契约：
//!
//! - 缺省 off（升级不得静默启动计费生产者）；
//! - `on` 唯一开启；`off` 显式关闭；
//! - typo（如 `ture` / `ON` / 空串）一律 off——绝不静默开启计费循环。
//!
//! 循环体本身（扫描/防重入/provider 解析/落账）在 `pm-ai` 的
//! `conversation_reply` 模块测试覆盖；spawn/轮询节奏属启动装配，只能真实
//! 启动进程冒烟（超出本批范围）。

use pm_ai::conversation_reply::ConversationReplyMode;

#[test]
fn default_is_off_a_new_billable_loop_must_not_start_silently() {
    assert_eq!(ConversationReplyMode::parse(None), ConversationReplyMode::Off);
    assert!(!ConversationReplyMode::parse(None).enabled());
}

#[test]
fn explicit_values_on_and_off_parse_as_the_switch_contract() {
    assert_eq!(
        ConversationReplyMode::parse(Some("on")),
        ConversationReplyMode::On
    );
    assert!(ConversationReplyMode::parse(Some("on")).enabled());
    assert_eq!(
        ConversationReplyMode::parse(Some("off")),
        ConversationReplyMode::Off
    );
    assert!(!ConversationReplyMode::parse(Some("off")).enabled());
}

#[test]
fn typos_stay_off_never_silently_enable() {
    // main.rs 的日志文本承诺「unknown value ... staying off」——这里钉住
    // 常见 typo 与大小写变体都不开启循环。
    for typo in ["ture", "true", "ON", "On", "enable", "1", ""] {
        let mode = ConversationReplyMode::parse(Some(typo));
        assert_eq!(mode, ConversationReplyMode::Off, "typo {typo:?} must stay off");
        assert!(!mode.enabled(), "typo {typo:?} must not enable the loop");
    }
}
