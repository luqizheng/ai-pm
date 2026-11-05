/**
 * 知识快照（Project Command Center）展示数据契约。
 *
 * 只读摘要：四类知识计数 pill + 同步时间脚注。
 * 文案由调用方翻译后经 props 传入。
 */

/** 单类知识计数（Pixso 3:7825 系） */
export interface KnowledgeStat {
  /** 类别名（如「规范文档」） */
  label: string;
  /** 计数 */
  count: number;
}
