import { isTauri } from "@tauri-apps/api/core";

let loggingAttached = false;

export async function bootstrapDesktopRuntime() {
  if (!isTauri() || loggingAttached) {
    return;
  }

  try {
    const [{ attachConsole, info }] = await Promise.all([import("@tauri-apps/plugin-log")]);
    await attachConsole();
    await info("Bexo Studio frontend shell ready");
    loggingAttached = true;
  } catch (error) {
    console.error("Failed to attach Tauri log bridge", error);
  }
}
