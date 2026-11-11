//! Document Processing 比较/导出算子（TD-02 §3.6-3.7 的 V1 实现）。
//!
//! 比较 = 行级 LCS diff（added/removed/changed 三类，表格级差异列警告——
//! U-10 边界）；导出 = 最小 Markdown→HTML 渲染（零新依赖；DOCX/PDF 导出
//! 是 P1，规格 §11）。

/// 行级差异块。
#[derive(Debug, Clone, serde::Serialize)]
pub struct DiffBlock {
    pub kind: &'static str, // added / removed / changed
    pub left_line: Option<usize>,
    pub right_line: Option<usize>,
    pub left: Option<String>,
    pub right: Option<String>,
}

/// 行级 LCS diff。输入两段 Markdown 文本（按行切分）。
pub fn diff_markdown(left: &str, right: &str) -> Vec<DiffBlock> {
    let a: Vec<&str> = left.lines().collect();
    let b: Vec<&str> = right.lines().collect();
    let n = a.len();
    let m = b.len();

    // LCS 长度表（行数上限保护：>20k 行降级为逐行对比标记）
    if n.saturating_mul(m) > 400_000_000 {
        return trivial_diff(&a, &b);
    }
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }

    let mut out = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if a[i] == b[j] {
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            // a[i] 删除；若 b[j] 是相邻替换则合并为 changed
            if j < m && lcs[i + 1][j + 1] >= lcs[i + 1][j].max(lcs[i][j + 1]) {
                // 保持简单：removed 后紧跟 added 的由下方循环自然合并
            }
            out.push(DiffBlock {
                kind: "removed",
                left_line: Some(i + 1),
                right_line: None,
                left: Some(a[i].to_string()),
                right: None,
            });
            i += 1;
        } else {
            out.push(DiffBlock {
                kind: "added",
                left_line: None,
                right_line: Some(j + 1),
                left: None,
                right: Some(b[j].to_string()),
            });
            j += 1;
        }
    }
    while i < n {
        out.push(DiffBlock {
            kind: "removed",
            left_line: Some(i + 1),
            right_line: None,
            left: Some(a[i].to_string()),
            right: None,
        });
        i += 1;
    }
    while j < m {
        out.push(DiffBlock {
            kind: "added",
            left_line: None,
            right_line: Some(j + 1),
            left: None,
            right: Some(b[j].to_string()),
        });
        j += 1;
    }

    // 相邻 removed+added 合并为 changed（同源行替换语义）
    merge_to_changed(out)
}

fn merge_to_changed(blocks: Vec<DiffBlock>) -> Vec<DiffBlock> {
    let mut out = Vec::with_capacity(blocks.len());
    let mut idx = 0;
    while idx < blocks.len() {
        let cur = &blocks[idx];
        let next = blocks.get(idx + 1);
        match (cur.kind, next.map(|b| b.kind)) {
            ("removed", Some("added")) => {
                out.push(DiffBlock {
                    kind: "changed",
                    left_line: cur.left_line,
                    right_line: next.and_then(|b| b.right_line),
                    left: cur.left.clone(),
                    right: next.and_then(|b| b.right.clone()),
                });
                idx += 2;
            }
            _ => {
                out.push(cur.clone());
                idx += 1;
            }
        }
    }
    out
}

fn trivial_diff(a: &[&str], b: &[&str]) -> Vec<DiffBlock> {
    let mut out = Vec::new();
    for (i, line) in a.iter().enumerate() {
        out.push(DiffBlock {
            kind: "removed",
            left_line: Some(i + 1),
            right_line: None,
            left: Some((*line).to_string()),
            right: None,
        });
    }
    for (j, line) in b.iter().enumerate() {
        out.push(DiffBlock {
            kind: "added",
            left_line: None,
            right_line: Some(j + 1),
            left: None,
            right: Some((*line).to_string()),
        });
    }
    out
}

/// 最小 Markdown → HTML（标题/段落/列表/管道表/代码围栏/引用块；安全：
/// 文本节点 HTML 转义，无脚本注入面——规格 §9.9）。
pub fn render_markdown_html(md: &str, title: &str) -> String {
    fn esc(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    }

    let mut out = String::new();
    out.push_str("<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"><title>");
    out.push_str(&esc(title));
    out.push_str("</title></head><body>\n");

    let mut in_ul = false;
    let mut in_ol = false;
    let mut in_table = false;
    let mut in_code = false;
    let mut table_row_idx = 0usize;

    for raw in md.lines() {
        let line = raw.trim_end();
        if in_code {
            if line.trim_start().starts_with("```") {
                in_code = false;
                out.push_str("</code></pre>\n");
            } else {
                out.push_str(&esc(line));
                out.push('\n');
            }
            continue;
        }
        if line.trim_start().starts_with("```") {
            close_lists_tables(&mut out, &mut in_ul, &mut in_ol, &mut in_table);
            in_code = true;
            out.push_str("<pre><code>");
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            close_lists_tables(&mut out, &mut in_ul, &mut in_ol, &mut in_table);
            continue;
        }
        // 标题
        if let Some(rest) = trimmed.strip_prefix('#') {
            let level = rest.chars().take_while(|c| *c == '#').count() + 1;
            let level = level.min(6);
            let text = rest.trim_start_matches('#').trim();
            close_lists_tables(&mut out, &mut in_ul, &mut in_ol, &mut in_table);
            out.push_str(&format!("<h{level}>{}</h{level}>\n", esc(text)));
            continue;
        }
        // 引用
        if let Some(rest) = trimmed.strip_prefix('>') {
            close_lists_tables(&mut out, &mut in_ul, &mut in_ol, &mut in_table);
            out.push_str(&format!(
                "<blockquote><p>{}</p></blockquote>\n",
                esc(rest.trim())
            ));
            continue;
        }
        // 列表
        if let Some(rest) = trimmed.strip_prefix("- ") {
            if in_table {
                close_lists_tables(&mut out, &mut in_ul, &mut in_ol, &mut in_table);
            }
            if !in_ul {
                if in_ol {
                    out.push_str("</ol>\n");
                    in_ol = false;
                }
                out.push_str("<ul>\n");
                in_ul = true;
            }
            out.push_str(&format!("<li>{}</li>\n", esc(rest)));
            continue;
        }
        if let Some(rest) = parse_ordered_item(trimmed) {
            if !in_ol {
                if in_ul {
                    out.push_str("</ul>\n");
                    in_ul = false;
                }
                out.push_str("<ol>\n");
                in_ol = true;
            }
            out.push_str(&format!("<li>{}</li>\n", esc(rest)));
            continue;
        }
        // 管道表
        if trimmed.starts_with('|') && trimmed.ends_with('|') {
            let cells: Vec<&str> = trimmed[1..trimmed.len() - 1].split('|').collect();
            let is_sep = cells.iter().all(|c| {
                let t = c.trim();
                t == "---" || t.is_empty()
            });
            if is_sep {
                continue;
            }
            if !in_table {
                close_lists_tables(&mut out, &mut in_ul, &mut in_ol, &mut in_table);
                out.push_str("<table>\n");
                in_table = true;
                table_row_idx = 0;
            }
            if table_row_idx == 0 {
                out.push_str("<thead><tr>");
                for c in &cells {
                    out.push_str(&format!("<th>{}</th>", esc(c.trim())));
                }
                out.push_str("</tr></thead>\n<tbody>\n");
            } else {
                out.push_str("<tr>");
                for c in &cells {
                    out.push_str(&format!("<td>{}</td>", esc(c.trim())));
                }
                out.push_str("</tr>\n");
            }
            table_row_idx += 1;
            continue;
        }
        // 段落
        close_lists_tables(&mut out, &mut in_ul, &mut in_ol, &mut in_table);
        out.push_str(&format!("<p>{}</p>\n", esc(trimmed)));
    }
    close_lists_tables(&mut out, &mut in_ul, &mut in_ol, &mut in_table);
    if in_code {
        out.push_str("</code></pre>\n");
    }
    out.push_str("</body></html>\n");
    out
}

fn parse_ordered_item(line: &str) -> Option<&str> {
    let dot = line.find(". ")?;
    let digits = &line[..dot];
    if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
        Some(&line[dot + 2..])
    } else {
        None
    }
}

fn close_lists_tables(out: &mut String, in_ul: &mut bool, in_ol: &mut bool, in_table: &mut bool) {
    if *in_table {
        out.push_str("</tbody></table>\n");
        *in_table = false;
    }
    if *in_ul {
        out.push_str("</ul>\n");
        *in_ul = false;
    }
    if *in_ol {
        out.push_str("</ol>\n");
        *in_ol = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_detects_changed_line() {
        let blocks = diff_markdown("a\nb\nc", "a\nB\nc");
        assert!(blocks
            .iter()
            .any(|b| b.kind == "changed" && b.left.as_deref() == Some("b")));
    }

    #[test]
    fn diff_detects_added() {
        let blocks = diff_markdown("a", "a\nnew");
        assert!(blocks.iter().any(|b| b.kind == "added"));
    }

    #[test]
    fn html_render_escapes() {
        let html = render_markdown_html("# T\n\nhello <script>alert(1)</script>", "t");
        assert!(html.contains("<h1>T</h1>"));
        assert!(!html.contains("<script>alert"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn html_renders_table() {
        let html = render_markdown_html("| a | b |\n| --- | --- |\n| 1 | 2 |", "t");
        assert!(html.contains("<table>"));
        assert!(html.contains("<th>a</th>"));
    }
}
