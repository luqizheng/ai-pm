export interface NavigationGroupProps {
  title: string;
  collapsed?: boolean;
  defaultCollapsed?: boolean;
  badge?: number;
  /** 无标题行：仅渲染子项（用于区域顶部的单条目组 / 侧栏折叠态） */
  headerless?: boolean;
}
