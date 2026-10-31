/**
 * Vditor 运行时资源基址。
 *
 * latex/lute 引擎、内容主题、emoji 图片都由 Vditor 在运行时用 `<script>` / `<link>`
 * 从 `options.cdn` 指定的目录动态加载，默认指向 unpkg。内网与离线环境下该地址不可达，
 * 编辑器会卡在解析引擎未加载的状态。
 *
 * 资源由 `scripts/sync-vditor-assets.js` 落到 `public/vditor/`，这里拼出同源路径，
 * 跟随 `BASE_URL` 变化（部署在子路径下也能命中）。
 */
const base = import.meta.env.BASE_URL.replace(/\/+$/, "");

export const VDITOR_CDN = `${base}/vditor`;
