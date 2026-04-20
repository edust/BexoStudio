import { z } from "zod";

import type { TerminalCommandTemplateRecord } from "@/types/backend";

export type ParsedTerminalCommand = {
  command: string;
  args: string[];
};

export const terminalCommandLineSchema = z
  .string()
  .trim()
  .min(1, "请输入终端命令")
  .max(512, "终端命令不能超过 512 个字符")
  .superRefine((value, context) => {
    try {
      parseTerminalCommandLine(value);
    } catch (error) {
      context.addIssue({
        code: z.ZodIssueCode.custom,
        message: error instanceof Error ? error.message : "终端命令格式无效",
      });
    }
  });

export const terminalCommandTemplateSchema = z.object({
  id: z
    .string()
    .trim()
    .max(64, "模板 ID 不能超过 64 个字符")
    .optional()
    .transform((value) => value?.trim() ?? ""),
  name: z
    .string()
    .trim()
    .min(1, "请输入模板名称")
    .max(80, "模板名称不能超过 80 个字符"),
  commandLine: terminalCommandLineSchema,
});

export type TerminalCommandTemplateFormInputValues = z.input<
  typeof terminalCommandTemplateSchema
>;
export type TerminalCommandTemplateFormValues = z.output<
  typeof terminalCommandTemplateSchema
>;

export function createEmptyTerminalCommandTemplate(
  sortOrder = 0,
): TerminalCommandTemplateRecord {
  return {
    id: createTerminalCommandTemplateId(),
    name: "",
    commandLine: "",
    sortOrder,
  };
}

export function createTerminalCommandTemplateId() {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }

  return `template-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

export function parseTerminalCommandLine(commandLine: string): ParsedTerminalCommand {
  const normalized = commandLine.trim();
  if (!normalized) {
    throw new Error("命令行不能为空");
  }
  if (normalized.includes("\n") || normalized.includes("\r")) {
    throw new Error("命令行必须是单行");
  }

  const tokens: string[] = [];
  let current = "";
  let quote: "'" | '"' | null = null;

  for (let index = 0; index < normalized.length; index += 1) {
    const character = normalized[index];

    if (quote) {
      if (character === "\\" && normalized[index + 1] === quote) {
        current += quote;
        index += 1;
        continue;
      }

      if (character === quote) {
        quote = null;
      } else {
        current += character;
      }
      continue;
    }

    if (character === '"' || character === "'") {
      quote = character;
      continue;
    }

    if (/\s/.test(character)) {
      if (current) {
        tokens.push(current);
        current = "";
      }
      continue;
    }

    current += character;
  }

  if (quote) {
    throw new Error("命令行包含未闭合的引号");
  }

  if (current) {
    tokens.push(current);
  }

  if (!tokens.length) {
    throw new Error("命令行不能为空");
  }

  return {
    command: tokens[0],
    args: tokens.slice(1),
  };
}

export function buildTerminalCommandLine(command: string, args: string[]) {
  return [command, ...args]
    .filter((segment) => segment.trim().length > 0)
    .map((segment) => quoteIfNeeded(segment.trim()))
    .join(" ");
}

export function sortTerminalCommandTemplates(templates: TerminalCommandTemplateRecord[]) {
  return [...templates]
    .map((template, index) => ({
      template,
      index,
      sortOrder: resolveTemplateSortOrder(template.sortOrder, index),
    }))
    .sort((left, right) => left.sortOrder - right.sortOrder || left.index - right.index)
    .map(({ template }, index) => ({
      ...template,
      sortOrder: index,
    }));
}

export function normalizeTerminalCommandTemplates(
  templates: TerminalCommandTemplateRecord[],
) {
  return assignTerminalCommandTemplateSortOrder(sortTerminalCommandTemplates(templates));
}

export function assignTerminalCommandTemplateSortOrder(
  templates: TerminalCommandTemplateRecord[],
) {
  return templates.map((template, index) => ({
    ...template,
    sortOrder: index,
  }));
}

export function reorderTerminalCommandTemplates(
  templates: TerminalCommandTemplateRecord[],
  draggingTemplateId: string,
  targetTemplateId: string,
) {
  const normalized = normalizeTerminalCommandTemplates(templates);
  const draggingIndex = normalized.findIndex(
    (template) => template.id === draggingTemplateId,
  );
  const targetIndex = normalized.findIndex((template) => template.id === targetTemplateId);

  if (draggingIndex < 0 || targetIndex < 0 || draggingIndex === targetIndex) {
    return null;
  }

  const nextTemplates = [...normalized];
  const [draggingTemplate] = nextTemplates.splice(draggingIndex, 1);
  nextTemplates.splice(targetIndex, 0, draggingTemplate);
  return assignTerminalCommandTemplateSortOrder(nextTemplates);
}

function quoteIfNeeded(value: string) {
  if (!/[\s"']/.test(value)) {
    return value;
  }

  if (!value.includes('"')) {
    return `"${value}"`;
  }

  if (!value.includes("'")) {
    return `'${value}'`;
  }

  return `"${value.replace(/"/g, '\\"')}"`;
}

function resolveTemplateSortOrder(sortOrder: number | undefined, fallbackIndex: number) {
  if (Number.isFinite(sortOrder) && sortOrder !== undefined && sortOrder >= 0) {
    return sortOrder;
  }

  return fallbackIndex;
}
