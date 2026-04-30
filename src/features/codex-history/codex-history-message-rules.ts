import type { CodexHistoryMessage } from "@/types/backend";

export function isCodexHistoryFinalMessage(message: CodexHistoryMessage) {
  return (
    message.itemType === "message" &&
    (message.role === "assistant" || message.role === "user") &&
    !isTechnicalUserContextMessage(message)
  );
}

export function shouldCollapseCodexHistoryMessage(message: CodexHistoryMessage) {
  return !isCodexHistoryFinalMessage(message);
}

export function filterCodexHistoryMessages(
  messages: CodexHistoryMessage[],
  showTechnicalMessages: boolean,
) {
  if (showTechnicalMessages) {
    return messages;
  }
  return messages.filter(isCodexHistoryFinalMessage);
}

function isTechnicalUserContextMessage(message: CodexHistoryMessage) {
  if (message.role !== "user" || message.itemType !== "message") {
    return false;
  }

  const content = message.content.trimStart().toLowerCase();
  return (
    content.startsWith("# agents.md instructions") ||
    content.startsWith("agents.md instructions") ||
    content.includes("<instructions>") ||
    content.includes("<permissions instructions>") ||
    content.includes("<environment_context>") ||
    content.includes("<collaboration_mode>") ||
    content.includes("<skills_instructions>")
  );
}
