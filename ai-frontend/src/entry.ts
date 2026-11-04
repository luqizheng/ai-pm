import { createApp } from "vue";
import App from "./App.vue";

/**
 * ai-frontend 入口。
 *
 * 旧 PRISM 壳（src/shell）与 AI Workspace（src/workspace）已整体移除，页面按
 * AGENTS.md「AI R&D Workspace — Frontend Foundation」规范重建；本文件目前只
 * 装载设计系统 token 层（src/styles/workspace/，设计系统 V1.0 唯一视觉真源）
 * 并挂载占位页，让 `pnpm dev` 可启动。重建基础（认证 / 壳 / 导航 …）时替换。
 */
import "./styles/index.css";

createApp(App).mount("#app");
