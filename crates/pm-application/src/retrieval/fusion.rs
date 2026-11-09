//! RRF 融合（WP-4 Q-A）。
//!
//! **裁决纪律**：
//! - RRF 只融合**不同候选来源**的排序——V1 三通道 `pc_entity` / `pc_fts` /
//!   `kb_section`（k=60，专家实测先例）；**同源 FTS 与 PageIndex 不重复计入**
//!   （PageIndex 是 KB 通道内部的定位/锚点加成，不是独立 RRF 输入）。
//! - **遍历边不参与 RRF**——关系是结构化事实，不是相似度竞争者，直进证据集。
//! - **RRF 只排序，不裁决权威性**：只接收已过硬过滤的候选；RRF 分数只用于内部
//!   排序，不得包装成 Evidence 的综合质量分。
//! - **确定性 tie-breaker**：同分按 通道优先级 → 实体类型 → ID 排序，防 DB 返回
//!   顺序漂移导致 Evidence 顺序不稳定。

use std::collections::BTreeMap;

use kb_pc_contracts::retrieval::{RetrievalChannel, RetrievalEvidence};

/// RRF 平滑参数（WP-4 定版：k=60）。
pub const RRF_K: f64 = 60.0;

/// 通道优先级（tie-breaker 第一序；数值小者优先保留）。
fn channel_priority(channel: RetrievalChannel) -> u8 {
    match channel {
        RetrievalChannel::PcEntity => 0,
        RetrievalChannel::KbSection => 1,
        RetrievalChannel::PcFts => 2,
        RetrievalChannel::KbVector | RetrievalChannel::PcVector => 3,
        RetrievalChannel::Relationship => 4,
    }
}

/// 去重键：同一（domain, id, revision, locator）归并——多通道命中合并通道来源。
fn dedup_key(e: &RetrievalEvidence) -> String {
    format!(
        "{}|{}|{}|{}",
        e.source_domain.as_str(),
        e.source_id,
        e.revision_id.as_deref().unwrap_or(""),
        e.locator.as_deref().unwrap_or("")
    )
}

/// RRF 融合 + 去重。
///
/// 输入 = 各通道已过硬过滤的命中（顺序 = 通道内排名）；输出 = 融合去重后的
/// 证据列表（RRF 分写入 `relevance_score`——仅 Relevance 维，非综合质量分）。
pub fn fuse(hits: Vec<RetrievalEvidence>) -> Vec<RetrievalEvidence> {
    // 按通道分组取排名（输入顺序即排名；按通道优先级序处理保证确定性）。
    let mut by_channel: Vec<(u8, RetrievalChannel, Vec<&RetrievalEvidence>)> = Vec::new();
    for hit in &hits {
        if let Some(entry) = by_channel
            .iter_mut()
            .find(|(_, ch, _)| *ch == hit.retrieval_channel)
        {
            entry.2.push(hit);
        } else {
            by_channel.push((
                channel_priority(hit.retrieval_channel),
                hit.retrieval_channel,
                vec![hit],
            ));
        }
    }
    by_channel.sort_by_key(|(prio, _, _)| *prio);

    // RRF 计分。
    let mut rrf: BTreeMap<String, f64> = BTreeMap::new();
    for (_, _, channel_hits) in &by_channel {
        for (rank, hit) in channel_hits.iter().enumerate() {
            *rrf.entry(dedup_key(hit)).or_default() += 1.0 / (RRF_K + (rank as f64 + 1.0));
        }
    }

    // 去重合并（保留首个 = 通道优先级最高者；合并 via_channels 与最高 RRF 分）。
    let mut merged: Vec<RetrievalEvidence> = Vec::new();
    let mut index_of: BTreeMap<String, usize> = BTreeMap::new();
    for hit in hits {
        let key = dedup_key(&hit);
        match index_of.get(&key) {
            Some(&i) => {
                let kept = &mut merged[i];
                if !kept.via_channels.contains(&hit.retrieval_channel) {
                    kept.via_channels.push(hit.retrieval_channel);
                }
            }
            None => {
                index_of.insert(key, merged.len());
                merged.push(hit);
            }
        }
    }

    // RRF 分（Relevance 维）+ 确定性排序：RRF 降序 → 通道优先级 → 稳定 ID。
    for e in &mut merged {
        if let Some(score) = rrf.get(&dedup_key(e)) {
            e.relevance_score = Some(*score as f32);
        }
    }
    merged.sort_by(|a, b| {
        let sa = rrf.get(&dedup_key(a)).copied().unwrap_or(0.0);
        let sb = rrf.get(&dedup_key(b)).copied().unwrap_or(0.0);
        sb.partial_cmp(&sa)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                channel_priority(a.retrieval_channel).cmp(&channel_priority(b.retrieval_channel))
            })
            .then_with(|| a.evidence_id.cmp(&b.evidence_id))
    });
    merged
}

#[cfg(test)]
mod tests {
    use super::fuse;
    use kb_pc_contracts::retrieval::{
        AuthorityStatus, KnowledgeDomain, RetrievalChannel, RetrievalEvidence,
    };

    fn hit(channel: RetrievalChannel, id: &str, title: &str) -> RetrievalEvidence {
        RetrievalEvidence {
            evidence_id: format!("{channel:?}:{id}").to_lowercase(),
            source_domain: KnowledgeDomain::Pc,
            source_id: id.to_string(),
            revision_id: None,
            authority_status: AuthorityStatus::CurrentPublished,
            project_id: uuid::Uuid::nil(),
            relevance_score: None,
            retrieval_channel: channel,
            via_channels: vec![channel],
            title: Some(title.to_string()),
            content_excerpt: title.to_string(),
            locator: Some(format!("loc:{id}")),
            freshness: None,
            truncated: false,
            relationship_path: Vec::new(),
            speculative: false,
        }
    }

    /// FUSION-01：多通道 RRF 排序（k=60）——双通道命中者排在单通道之前。
    #[test]
    fn fusion_01_dual_channel_hit_ranks_first() {
        let a_entity = hit(RetrievalChannel::PcEntity, "obj-1", "订单状态");
        let a_fts = hit(RetrievalChannel::PcFts, "obj-1", "订单状态");
        let b_entity = hit(RetrievalChannel::PcEntity, "obj-2", "订单明细");
        let fused = fuse(vec![a_entity, b_entity, a_fts]);
        // obj-1 命中两通道（RRF 2/(60+1)×…) > obj-2 单通道。
        assert_eq!(fused[0].source_id, "obj-1");
        assert_eq!(fused[0].via_channels.len(), 2, "merged channel sources");
    }

    /// FUSION-03：四元组去重 + 通道来源合并（不产生重复条目）。
    #[test]
    fn fusion_03_dedup_merges_channel_sources() {
        let a1 = hit(RetrievalChannel::PcEntity, "obj-1", "订单状态");
        let mut a2 = hit(RetrievalChannel::PcEntity, "obj-1", "订单状态");
        a2.retrieval_channel = RetrievalChannel::PcFts;
        a2.via_channels = vec![RetrievalChannel::PcFts];
        let fused = fuse(vec![a1, a2]);
        assert_eq!(fused.len(), 1);
        assert_eq!(fused[0].via_channels.len(), 2);
    }

    /// FUSION-05：同分确定性排序——真并列 = 跨通道各自 rank1（等 RRF 分）；
    /// tie-breaker = 通道优先级（pc_entity > kb_section），与输入顺序无关。
    #[test]
    fn fusion_05_deterministic_tie_breaking() {
        let entity = hit(RetrievalChannel::PcEntity, "obj-a", "甲");
        let section = {
            let mut e = hit(RetrievalChannel::KbSection, "doc-9", "乙");
            e.source_domain = KnowledgeDomain::Kb;
            e
        };
        let fused1 = fuse(vec![entity.clone(), section.clone()]);
        let fused2 = fuse(vec![section, entity]);
        let ids1: Vec<_> = fused1.iter().map(|e| e.source_id.clone()).collect();
        let ids2: Vec<_> = fused2.iter().map(|e| e.source_id.clone()).collect();
        assert_eq!(
            ids1, ids2,
            "tie must break deterministically regardless of input order"
        );
        assert_eq!(
            ids1[0], "obj-a",
            "channel priority wins the tie (pc_entity first)"
        );
    }
}
