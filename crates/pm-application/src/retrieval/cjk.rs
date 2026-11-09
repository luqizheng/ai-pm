//! CJK bigram 预分词（WP-3 Q-A）。
//!
//! **双端一致性是硬纪律**：索引端与查询端**必须调用同一个 [`cjk_bigrams`]**——
//! 否则召回静默劣化且无从发现（单测锁定唯一实现）。
//!
//! 规则（裁决定版）：
//! - Unicode NFKC 规范化 + 小写化，两端同一流程；
//! - ≥2 个连续 CJK 字符 → 重叠双字组（「订单状态」→ 订单 单状 状态）；
//! - **单个 CJK 字符 → 独立 token**（防短查询静默丢失，如搜「单」）；
//! - 英文/数字连续段（含内部连字符的编号形态，如 `adr-006`）→ 保留为 token；
//! - 空串 / 纯标点 → 不产生有效检索表达式（空串返回）。
//!
//! 用法：写入端 `to_tsvector('simple', cjk_bigrams(text))`；查询端
//! `plainto_tsquery('simple', cjk_bigrams(query))`（'simple' 配置按空白切分并
//! AND 各 token——本函数已保证 token 规范化形态）。

/// 判定是否 CJK 统一表意字符（含扩展 A；不含假名/谚文——本仓语料以中英为主，
/// 假名按普通字母段处理即可满足检索）。
fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x4E00..=0x9FFF    // CJK 统一表意文字
        | 0x3400..=0x4DBF  // 扩展 A
        | 0xF900..=0xFAFF  // 兼容表意文字
        | 0x20000..=0x2A6DF // 扩展 B
    )
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || is_cjk(c)
}

/// 规范化单字符：NFKC 近似（本仓无 unicode-normalization 依赖；做大小写折叠 +
/// 全角 ASCII 折叠，覆盖中文文档实际出现的形态）。
fn normalize(c: char) -> char {
    // 全角 → 半角（差值 0xFEE0），再统一小写——两步都做，顺序不敏感。
    let folded = match c {
        '０'..='９' | 'Ａ'..='Ｚ' | 'ａ'..='ｚ' => {
            char::from_u32(c as u32 - 0xFEE0).unwrap_or(c)
        }
        other => other,
    };
    folded.to_ascii_lowercase()
}

/// CJK bigram 分词：返回**空格拼接**的 token 串（喂 `to_tsvector('simple', ...)` /
/// `plainto_tsquery('simple', ...)`）。
///
/// 纯标点/空白输入返回空串——调用方据此跳过无效检索表达式。
pub fn cjk_bigrams(input: &str) -> String {
    let chars: Vec<char> = input.chars().map(normalize).collect();
    let mut tokens: Vec<String> = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if !is_word(c) {
            i += 1;
            continue;
        }
        if is_cjk(c) {
            // 连续 CJK 段。
            let start = i;
            while i < chars.len() && is_cjk(chars[i]) {
                i += 1;
            }
            let seg = &chars[start..i];
            if seg.len() == 1 {
                tokens.push(seg[0].to_string());
            } else {
                for w in seg.windows(2) {
                    tokens.push(w.iter().collect());
                }
            }
        } else {
            // 连续英数段（连字符切分，编号两侧各自成 token——索引端与查询端
            // 同规则，"ADR-006" ↔ [adr, 006] 对齐）。
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '-') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            for part in word.split('-').filter(|p| !p.is_empty()) {
                tokens.push(part.to_string());
            }
        }
    }

    tokens.join(" ")
}

#[cfg(test)]
mod tests {
    use super::cjk_bigrams;

    /// CJK-BIGRAM-01：重叠双字组（3 字 → 2 组）。
    #[test]
    fn chinese_produces_overlapping_bigrams() {
        assert_eq!(cjk_bigrams("订单状态"), "订单 单状 状态");
    }

    #[test]
    fn exact_bigram_sequence() {
        // 「订单状态变更」→ 订单 单状 状态 态变 变更。
        assert_eq!(cjk_bigrams("订单状态变更"), "订单 单状 状态 态变 变更");
    }

    /// CJK-BIGRAM-03：单汉字短查询保留为独立 token。
    #[test]
    fn single_cjk_char_is_kept_as_token() {
        assert_eq!(cjk_bigrams("单"), "单");
        assert_eq!(cjk_bigrams("订单"), "订单");
    }

    /// CJK-BIGRAM-02：中英数混合。
    #[test]
    fn mixed_cjk_latin_digits() {
        assert_eq!(cjk_bigrams("ADR-006 订单状态"), "adr 006 订单 单状 状态");
    }

    #[test]
    fn mixed_digits_and_cjk() {
        assert_eq!(cjk_bigrams("v1.3 版本"), "v1 3 版本");
    }

    /// 空串 / 纯标点 → 空（不产生有效检索表达式）。
    #[test]
    fn empty_and_punctuation_only_inputs_yield_empty() {
        assert_eq!(cjk_bigrams(""), "");
        assert_eq!(cjk_bigrams("   "), "");
        assert_eq!(cjk_bigrams("！？。，；"), "");
    }

    /// 大小写与全角折叠（索引/查询同流程）。
    #[test]
    fn case_and_width_folding_is_stable() {
        assert_eq!(cjk_bigrams("ADR-006"), cjk_bigrams("adr-006"));
        assert_eq!(cjk_bigrams("ＡＢＣ"), cjk_bigrams("abc"));
    }

    /// 双端一致性：同文本多 次调用结果稳定（纯函数性质的可观察面）。
    #[test]
    fn pure_function_determinism() {
        let a = cjk_bigrams("财务制度 v1.3 修订");
        let b = cjk_bigrams("财务制度 v1.3 修订");
        assert_eq!(a, b);
        assert!(a.contains("财务") && a.contains("务制"));
    }
}
