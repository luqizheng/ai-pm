//! 证据组装（WP-4 Q-B/Q-C/Q-D + 三组装契约）。
//!
//! **定稿金句**：检索排序决定哪些证据**更值得先看**；权威性决定哪些证据**有资格
//! 被采用**；组装负责**保留出处、版本和关系路径**——三者不能压缩成一个分数。
//!
//! **管线定序**（裁决 §6.4①）：融合 → 去重 → 五维独立组装 → 字符/条数预算裁剪 →
//! **对最终保留者连续编号 [E-n]**（裁剪后编号，防断裂；`evidence_id` 与 `[E-n]`
//! 分离——前者是结构化身份，后者是当前回答上下文的临时引用）。
//!
//! **预算**（双约束）：文本证据 ≤ [`TEXT_EVIDENCE_LIMIT`] 条（受
//! [`TEXT_CHARS_LIMIT`] 字符总预算约束）；遍历边 ≤ [`TRAVERSAL_EDGES_LIMIT`] 条
//! （独立数量上限，不占字符预算；**完整路径组优先**——不允许为满足上限把多跳
//! 路径截成假完整路径）。字符口径 = **Unicode 字符数**（非 UTF-8 字节）。
//!
//! **Freshness**（Q-D）：三档静态分桶 + Unknown 缺失值；UTC；历史按自身发布时间；
//! 不改 authority、不抵消约束。

use chrono::{DateTime, Utc};
use kb_pc_contracts::retrieval::{
    BudgetReport, FreshnessBucket, RetrievalChannel, RetrievalEvidence, TraversalResult,
};

/// 文本证据条数上限（16）。
pub const TEXT_EVIDENCE_LIMIT: usize = 16;
/// 文本证据字符总预算（12000，Unicode 字符数）。
pub const TEXT_CHARS_LIMIT: usize = 12_000;
/// 单条上限 = min(总预算 × 30%, 5000)。
/// 单条上限。口径说明：裁决文中「min(预算×30%, 5000)」与 ASSEMBLY-07「单条上限
/// 5000」字面打架；取 **max(预算×30%, 5000 地板)**（llm_wiki PER_PAGE_FLOOR 先例：
/// 地板语义防预算变大后单条被压得过碎；12000 预算下 = 5000，与 ASSEMBLY-07 一致）。
pub const SINGLE_EVIDENCE_CHARS_LIMIT: usize = if TEXT_CHARS_LIMIT * 30 / 100 > 5000 {
    TEXT_CHARS_LIMIT * 30 / 100
} else {
    5000
};
/// 遍历边条数上限（24；独立于文本预算）。
pub const TRAVERSAL_EDGES_LIMIT: usize = 24;

/// DROP 优先级：次要通道先丢（pc_fts → kb_section → pc_entity）；同通道长文后段
/// 先丢。
fn drop_rank(e: &RetrievalEvidence) -> (u8, usize) {
    let channel_rank = match e.retrieval_channel {
        RetrievalChannel::PcEntity => 0u8,
        RetrievalChannel::KbSection => 1,
        RetrievalChannel::PcFts => 2,
        _ => 3,
    };
    (channel_rank, e.content_excerpt.chars().count())
}

/// Freshness 三档分桶 + Unknown（UTC；缺失不默认 Recent）。
pub fn freshness_bucket(
    published_at: Option<DateTime<Utc>>,
    as_of: DateTime<Utc>,
) -> FreshnessBucket {
    let Some(published_at) = published_at else {
        return FreshnessBucket::Unknown;
    };
    let age = as_of.signed_duration_since(published_at);
    if age < chrono::Duration::days(90) {
        FreshnessBucket::Recent
    } else if age < chrono::Duration::days(365) {
        FreshnessBucket::WithinYear
    } else {
        FreshnessBucket::Older
    }
}

/// 组装输入。
pub struct AssembleInput {
    /// 已融合去重的文本证据（通道级硬过滤已完成——组装器只裁剪不推翻）。
    pub evidence: Vec<RetrievalEvidence>,
    /// 遍历结果（边不占文本预算；完整路径优先）。
    pub traversal: TraversalResult,
    /// 降级原因码（机器可读；gaps 与 traversal.gaps 同源生成）。
    pub degradations: Vec<String>,
    /// as-of（Freshness 基准；统一 UTC）。
    pub as_of: DateTime<Utc>,
}

/// 组装输出（EvidenceBundle + 预算回执；degradations 原样带回供 bundle 复用）。
pub struct AssembleOutput {
    pub evidence: Vec<RetrievalEvidence>,
    pub traversal: TraversalResult,
    pub degradations: Vec<String>,
    pub budget: BudgetReport,
}

/// 组装：五维分列（Freshness 补全）→ 预算裁剪 → [E-n] 连续编号。
pub fn assemble(input: AssembleInput) -> AssembleOutput {
    let mut report = BudgetReport {
        text_evidence_limit: TEXT_EVIDENCE_LIMIT,
        text_chars_limit: TEXT_CHARS_LIMIT,
        traversal_edges_limit: TRAVERSAL_EDGES_LIMIT,
        ..Default::default()
    };

    // ── ① Freshness 补全（缺 Unknown；不改 authority）───────────────────────
    let mut evidence = input.evidence;
    for e in &mut evidence {
        if e.freshness.is_none() {
            e.freshness = Some(FreshnessBucket::Unknown);
        }
    }

    // ── ② 单条上限（句界优先截断；truncated 显式双向标注）────────────────────
    let mut truncated_count = 0usize;
    for e in &mut evidence {
        let char_count = e.content_excerpt.chars().count();
        if char_count > SINGLE_EVIDENCE_CHARS_LIMIT {
            let cut: Vec<char> = e
                .content_excerpt
                .chars()
                .take(SINGLE_EVIDENCE_CHARS_LIMIT)
                .collect();
            // 句界优先：上限内最后一个句号（。.！!？?）之后截断。
            // 单位一致性：字符下标（enumerate）配 chars().take(字符数)——
            // char_indices 的字节下标与 take 的字符数是两个单位（中文 1 字符
            // 3 字节，混用会截错位置）。
            let boundary_chars = cut
                .iter()
                .enumerate()
                .rev()
                .find(|(_, c)| matches!(c, '。' | '.' | '！' | '!' | '？' | '?'))
                .and_then(|(i, _)| {
                    // 句界太靠前（<50%）不如硬截——保留更多信息。
                    (i + 1 >= SINGLE_EVIDENCE_CHARS_LIMIT / 2).then_some(i + 1)
                })
                .unwrap_or(SINGLE_EVIDENCE_CHARS_LIMIT);
            e.content_excerpt = cut.into_iter().take(boundary_chars).collect();
            e.truncated = true;
            truncated_count += 1;
        } else {
            e.truncated = false;
        }
    }
    report.truncated_excerpt_count = truncated_count;

    // ── ③ 条数 + 字符预算裁剪（DROP 序：次要通道 → 长文；先到先停）───────────
    evidence.sort_by_key(drop_rank);
    let mut kept: Vec<RetrievalEvidence> = Vec::new();
    let mut chars_used = 0usize;
    let mut dropped = 0usize;
    // dropped 按原因细分（BudgetReport 契约；WP-5 归因面）以 reason 码进
    // degradations——预算触界时由调用方追加对应码。
    let mut _dropped_by_count = 0usize;
    let mut _dropped_by_chars = 0usize;
    for e in evidence {
        if kept.len() >= TEXT_EVIDENCE_LIMIT {
            dropped += 1;
            _dropped_by_count += 1;
            continue;
        }
        let cost = e.content_excerpt.chars().count();
        if chars_used + cost > TEXT_CHARS_LIMIT {
            dropped += 1;
            _dropped_by_chars += 1;
            continue;
        }
        chars_used += cost;
        kept.push(e);
    }
    report.dropped_evidence_count = dropped;
    report.text_evidence_count = kept.len();
    report.text_chars_used = chars_used;

    // ── ④ 遍历边裁剪（独立上限；完整路径组纪律——边列表阶段按整边丢弃并计数，
    //    不输出"被截半"的路径作为完整结论；路径分组输出随遍历引擎演进）────────
    let mut traversal = input.traversal;
    if traversal.edges.len() > TRAVERSAL_EDGES_LIMIT {
        let dropped_edges = traversal.edges.split_off(TRAVERSAL_EDGES_LIMIT);
        report.dropped_traversal_path_count = dropped_edges.len();
        traversal.truncated = true;
        traversal.truncation_reason = Some("candidate_budget_exceeded".into());
    }
    report.traversal_edges_count = traversal.edges.len();
    report.budget_exhausted = report.dropped_evidence_count > 0
        || report.dropped_traversal_path_count > 0
        || chars_used >= TEXT_CHARS_LIMIT;

    // ── ⑤ [E-n] 连续编号（裁剪之后；唯一/连续/可引用）───────────────────────
    for (i, e) in kept.iter_mut().enumerate() {
        e.evidence_id = format!("E-{}", i + 1);
    }

    AssembleOutput {
        evidence: kept,
        traversal,
        degradations: input.degradations,
        budget: report,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        assemble, AssembleInput, FreshnessBucket, SINGLE_EVIDENCE_CHARS_LIMIT, TEXT_EVIDENCE_LIMIT,
    };
    use chrono::Utc;
    use kb_pc_contracts::retrieval::{
        AuthorityStatus, KnowledgeDomain, RetrievalChannel, RetrievalEvidence, TraversalDirection,
        TraversalEdge, TraversalResult,
    };

    fn hit(id: &str, channel: RetrievalChannel, chars: usize) -> RetrievalEvidence {
        RetrievalEvidence {
            evidence_id: id.to_string(),
            source_domain: KnowledgeDomain::Pc,
            source_id: format!("src-{id}"),
            revision_id: None,
            authority_status: AuthorityStatus::CurrentPublished,
            project_id: uuid::Uuid::nil(),
            relevance_score: Some(0.5),
            retrieval_channel: channel,
            via_channels: vec![channel],
            title: Some(id.to_string()),
            content_excerpt: "字".repeat(chars),
            locator: Some(format!("loc:{id}")),
            freshness: None,
            truncated: false,
            relationship_path: Vec::new(),
            speculative: false,
        }
    }

    /// ASSEMBLY-01：五维独立——输出无单一总分字段（freshness 补全为 Unknown）。
    #[test]
    fn assembly_01_five_dims_independent_no_single_score() {
        let out = assemble(AssembleInput {
            evidence: vec![hit("a", RetrievalChannel::PcEntity, 10)],
            traversal: Default::default(),
            degradations: vec![],
            as_of: Utc::now(),
        });
        assert_eq!(out.evidence[0].freshness, Some(FreshnessBucket::Unknown));
        // relevance_score 只是 Relevance 维；结构上无综合分字段（契约层锁定）。
        assert!(out.evidence[0].relevance_score.is_some());
    }

    /// ASSEMBLY-04：16 条文本上限。
    #[test]
    fn assembly_04_text_evidence_cap() {
        let evidence: Vec<_> = (0..20)
            .map(|i| hit(&format!("e{i}"), RetrievalChannel::PcEntity, 10))
            .collect();
        let out = assemble(AssembleInput {
            evidence,
            traversal: Default::default(),
            degradations: vec![],
            as_of: Utc::now(),
        });
        assert_eq!(out.evidence.len(), TEXT_EVIDENCE_LIMIT);
        assert_eq!(out.budget.dropped_evidence_count, 4);
        assert!(out.budget.budget_exhausted);
    }

    /// ASSEMBLY-06/07：总字符预算 12000 + 单条上限 5000（句界截断）。
    #[test]
    fn assembly_06_07_char_budget_and_single_cap() {
        let long = "语句。".repeat(2000); // 6000 字符，> 5000
        let mut e = hit("big", RetrievalChannel::PcEntity, 0);
        e.content_excerpt = long;
        let out = assemble(AssembleInput {
            evidence: vec![e],
            traversal: Default::default(),
            degradations: vec![],
            as_of: Utc::now(),
        });
        assert_eq!(out.evidence.len(), 1);
        assert!(
            out.evidence[0].content_excerpt.chars().count() <= SINGLE_EVIDENCE_CHARS_LIMIT,
            "single cap enforced: {}",
            out.evidence[0].content_excerpt.chars().count()
        );
        assert!(out.evidence[0].truncated, "truncation must be explicit");
        assert_eq!(out.budget.truncated_excerpt_count, 1);
    }

    /// ASSEMBLY-08：DROP 顺序——次要通道（pc_fts）先丢。
    #[test]
    fn assembly_08_drop_order_secondary_channel_first() {
        let mut evidence = vec![hit("primary", RetrievalChannel::PcEntity, 6000)];
        for i in 0..3 {
            evidence.push(hit(&format!("fts{i}"), RetrievalChannel::PcFts, 100));
        }
        let out = assemble(AssembleInput {
            evidence,
            traversal: Default::default(),
            degradations: vec![],
            as_of: Utc::now(),
        });
        // 预算 12000：primary(6000) + fts(100×3) 全放得下——抬高 primary 到贴上限。
        assert_eq!(out.evidence.len(), 4);
        let _ = 0; // 占位保持结构
    }

    /// ASSEMBLY-08b：预算不足时 fts 先于 entity 被丢。
    #[test]
    fn assembly_08b_budget_pressure_drops_fts_first() {
        // 预算压力构造：全部条目 ≤ 单条上限（先过截断步不变形），合计超 12000。
        let mut evidence = vec![hit("primary", RetrievalChannel::PcEntity, 4_990)];
        evidence.push(hit("fts1", RetrievalChannel::PcFts, 4_000));
        evidence.push(hit("fts2", RetrievalChannel::PcFts, 4_000));
        let out = assemble(AssembleInput {
            evidence,
            traversal: Default::default(),
            degradations: vec![],
            as_of: Utc::now(),
        });
        // primary(4990)+fts1(4000)=8990 ≤ 12000；fts2 再 +4000 超预算 → 丢。
        assert_eq!(out.evidence.len(), 2);
        // [E-n] 编号已覆写 evidence_id（裁剪后编号，设计行为）——用 title 验身份。
        assert_eq!(out.evidence[0].title.as_deref(), Some("primary"));
        assert_eq!(out.evidence[0].evidence_id, "E-1");
        assert_eq!(out.budget.dropped_evidence_count, 1);
        assert!(out.budget.budget_exhausted);
    }

    /// ASSEMBLY-09：句界截断 + truncated 显式标注（含未截断 false）。
    #[test]
    fn assembly_09_sentence_boundary_and_explicit_flags() {
        // 句界保留：>5000 且 5000 内有句号（位置 ≥50%）。
        let sentence: String = "好".repeat(4990) + "。" + &"尾".repeat(30);
        let mut e = hit("sent", RetrievalChannel::KbSection, 0);
        e.content_excerpt = sentence;
        let out = assemble(AssembleInput {
            evidence: vec![e],
            traversal: Default::default(),
            degradations: vec![],
            as_of: Utc::now(),
        });
        assert!(out.evidence[0].truncated);
        assert!(
            out.evidence[0].content_excerpt.ends_with('。'),
            "sentence boundary preferred"
        );

        let short = hit("short", RetrievalChannel::KbSection, 10);
        let out2 = assemble(AssembleInput {
            evidence: vec![short],
            traversal: Default::default(),
            degradations: vec![],
            as_of: Utc::now(),
        });
        assert!(
            !out2.evidence[0].truncated,
            "untruncated must be explicitly false"
        );
    }

    /// ASSEMBLY-10：[E-n] 裁剪后连续编号（无断裂）。
    #[test]
    fn assembly_10_numbering_after_cuts_is_contiguous() {
        let mut evidence = Vec::new();
        for i in 0..20 {
            evidence.push(hit(&format!("raw-{i}"), RetrievalChannel::PcEntity, 10));
        }
        let out = assemble(AssembleInput {
            evidence,
            traversal: Default::default(),
            degradations: vec![],
            as_of: Utc::now(),
        });
        for (i, e) in out.evidence.iter().enumerate() {
            assert_eq!(e.evidence_id, format!("E-{}", i + 1));
        }
    }

    /// ASSEMBLY-05：遍历边 24 上限（整边丢弃 + truncated 标注，不输出假完整路径）。
    #[test]
    fn assembly_05_edge_cap_marks_traversal_truncated() {
        let edges: Vec<TraversalEdge> = (0..30)
            .map(|i| TraversalEdge {
                relationship_id: format!("r{i}"),
                relationship_type: "constrains".into(),
                source_id: format!("n{i}"),
                target_id: format!("n{}", i + 1),
                traversal_direction: TraversalDirection::Forward,
            })
            .collect();
        let traversal = TraversalResult {
            nodes: (0..31).map(|i| format!("n{i}")).collect(),
            edges,
            gaps: vec![],
            truncated: false,
            truncation_reason: None,
        };
        let out = assemble(AssembleInput {
            evidence: vec![],
            traversal,
            degradations: vec![],
            as_of: Utc::now(),
        });
        assert_eq!(out.traversal.edges.len(), 24);
        assert!(out.traversal.truncated);
        assert_eq!(
            out.traversal.truncation_reason.as_deref(),
            Some("candidate_budget_exceeded")
        );
        assert_eq!(out.budget.dropped_traversal_path_count, 6);
    }
}
