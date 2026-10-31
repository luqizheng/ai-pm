import {
  FOCUS_OBJECT_TYPES,
  LAYOUT_TEMPLATES,
  type DataRef,
  type LayoutTemplate,
  type WorkspacePanel,
  type WorkspaceSpec
} from "./types";

export type ParseResult =
  | { ok: true; spec: WorkspaceSpec }
  | { ok: false; errors: string[] };

const isObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const isNonEmptyString = (value: unknown): value is string =>
  typeof value === "string" && value.trim().length > 0;

function parseDataRef(value: unknown, path: string, errors: string[]): DataRef | undefined {
  if (value === undefined) return undefined;
  if (!isObject(value)) {
    errors.push(`${path} must be an object`);
    return undefined;
  }
  if (!isNonEmptyString(value.type)) errors.push(`${path}.type must be a non-empty string`);
  if (!isNonEmptyString(value.id)) errors.push(`${path}.id must be a non-empty string`);
  return { type: String(value.type ?? ""), id: String(value.id ?? "") };
}

function parsePanel(value: unknown, index: number, errors: string[]): WorkspacePanel | undefined {
  const path = `panels[${index}]`;
  if (!isObject(value)) {
    errors.push(`${path} must be an object`);
    return undefined;
  }
  if (!isNonEmptyString(value.component)) {
    errors.push(`${path}.component must be a non-empty string`);
  }
  if (value.config !== undefined && !isObject(value.config)) {
    errors.push(`${path}.config must be an object when present`);
  }
  const dataRef = parseDataRef(value.dataRef, `${path}.dataRef`, errors);
  return {
    component: String(value.component ?? ""),
    dataRef,
    config: isObject(value.config) ? { ...value.config } : undefined
  };
}

/**
 * 手写校验（不引 zod：目标约束禁止新增 npm 依赖）。未知字段被忽略，
 * 与协议「刻意宽松」的约定一致；函数只负责结构合法性。
 */
export function parseWorkspaceSpec(input: unknown): ParseResult {
  const errors: string[] = [];
  if (!isObject(input)) {
    return { ok: false, errors: ["spec must be an object"] };
  }

  if (input.schemaVersion !== "1.0") {
    errors.push('schemaVersion must be "1.0"');
  }
  if (!isNonEmptyString(input.workspaceId)) {
    errors.push("workspaceId must be a non-empty string");
  }
  if (!isNonEmptyString(input.projectId)) {
    errors.push("projectId must be a non-empty string");
  }
  const subject = parseDataRef(input.subject, "subject", errors);

  const layout = input.layout;
  if (!LAYOUT_TEMPLATES.includes(layout as LayoutTemplate)) {
    errors.push(`layout must be one of ${LAYOUT_TEMPLATES.join("/")}`);
  }

  const panels: WorkspacePanel[] = [];
  if (!Array.isArray(input.panels) || input.panels.length === 0) {
    errors.push("panels must be a non-empty array");
  } else {
    input.panels.forEach((panel, index) => {
      const parsed = parsePanel(panel, index, errors);
      if (parsed) panels.push(parsed);
    });
  }

  if (errors.length > 0) return { ok: false, errors };

  return {
    ok: true,
    spec: {
      schemaVersion: "1.0",
      workspaceId: input.workspaceId as string,
      projectId: input.projectId as string,
      subject,
      layout: layout as LayoutTemplate,
      panels
    }
  };
}

/** /focus 的对象类型是否在 P0 子集内。 */
export function isFocusObjectType(value: string): boolean {
  return (FOCUS_OBJECT_TYPES as readonly string[]).includes(value);
}