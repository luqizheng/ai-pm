/**
 * 项目成员（Project Command Center）展示数据契约。
 *
 * 成员以头像点 + 名字的 pill 呈现（只读名单，非管理入口）。
 */

/** 单个成员 pill */
export interface MemberChip {
  /** 稳定键 */
  id: string;
  /** 显示名 */
  name: string;
}
