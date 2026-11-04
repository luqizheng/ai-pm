import { useI18n } from "vue-i18n";

/**
 * 相对时间格式化（活动流、项目最近活跃时间共用）。
 *
 * 只做"多久之前"的呈现，不参与任何紧急度判定 ——
 * 紧急度只能来自后端给出的逾期/截止字段（高保真稿 §6 禁止把更新时间当紧急度）。
 */
export function useRelativeTime() {
  const { t, locale } = useI18n();

  function relative(iso: string | null | undefined): string {
    if (!iso) return t("pm.personalHome.time.unknown");
    const target = Date.parse(iso);
    if (Number.isNaN(target)) return t("pm.personalHome.time.unknown");
    const minutes = Math.round((Date.now() - target) / 60_000);
    if (minutes < 1) return t("pm.personalHome.time.justNow");
    if (minutes < 60)
      return t("pm.personalHome.time.minutesAgo", { n: minutes });
    const hours = Math.round(minutes / 60);
    if (hours < 24) return t("pm.personalHome.time.hoursAgo", { n: hours });
    const days = Math.round(hours / 24);
    return t("pm.personalHome.time.daysAgo", { n: days });
  }

  return { relative, locale };
}
