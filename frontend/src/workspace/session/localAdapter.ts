import type { WorkspaceSpec } from "../spec/types";

/**
 * 工作区会话适配器（附录 D.2 的 P0 形态）：sessionStorage 存会话 id 实现
 * 多标签独立；localStorage 存快照与最近 10 份历史（D.3）。
 */
export interface SessionAdapter {
  load(projectId: string): WorkspaceSpec | undefined;
  save(projectId: string, spec: WorkspaceSpec): void;
  history(projectId: string): WorkspaceSpec[];
}

const SESSION_KEY = "ai-pm:workspace:session";
const HISTORY_LIMIT = 10;

export function currentSessionId(): string {
  const existing = sessionStorage.getItem(SESSION_KEY);
  if (existing) return existing;
  const created = `sess-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  sessionStorage.setItem(SESSION_KEY, created);
  return created;
}

const snapshotKey = (projectId: string, sessionId: string) =>
  `ai-pm:workspace:snapshot:${projectId}:${sessionId}`;
const historyKey = (projectId: string, sessionId: string) =>
  `ai-pm:workspace:history:${projectId}:${sessionId}`;

export const localSessionAdapter: SessionAdapter = {
  load(projectId) {
    const raw = localStorage.getItem(snapshotKey(projectId, currentSessionId()));
    if (!raw) return undefined;
    try {
      return JSON.parse(raw) as WorkspaceSpec;
    } catch {
      return undefined;
    }
  },

  save(projectId, spec) {
    const sessionId = currentSessionId();
    localStorage.setItem(snapshotKey(projectId, sessionId), JSON.stringify(spec));

    const key = historyKey(projectId, sessionId);
    const previous = localSessionAdapter.history(projectId);
    const next = [spec, ...previous].slice(0, HISTORY_LIMIT);
    localStorage.setItem(key, JSON.stringify(next));
  },

  history(projectId) {
    const raw = localStorage.getItem(historyKey(projectId, currentSessionId()));
    if (!raw) return [];
    try {
      const parsed = JSON.parse(raw) as WorkspaceSpec[];
      return Array.isArray(parsed) ? parsed : [];
    } catch {
      return [];
    }
  }
};