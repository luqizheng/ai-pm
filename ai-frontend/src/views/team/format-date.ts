/**
 * 加入时间格式化（Figma T2「Joined」列口径：03 Jun 2025）。
 *
 * 日期取成员 created_at 的日粒度；非法 / 缺失值原样返回占位符。
 * 英文走 en-GB（日-月-年，与设计稿一致），中文走本地顺序。
 */
export function formatMembershipDate(
  value: string | null | undefined,
  locale = "en"
): string {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  const tag = locale.toLowerCase().startsWith("zh") ? "zh-CN" : "en-GB";
  return new Intl.DateTimeFormat(tag, {
    day: "2-digit",
    month: "short",
    year: "numeric"
  }).format(date);
}
