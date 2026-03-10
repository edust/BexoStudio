import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";

import { hasDesktopRuntime } from "@/lib/command-client";

type DesktopNotificationInput = {
  title: string;
  body: string;
};

let permissionResolved = false;
let permissionGranted = false;

export async function sendDesktopNotification(input: DesktopNotificationInput) {
  if (!hasDesktopRuntime()) {
    return false;
  }

  const title = input.title.trim();
  const body = input.body.trim();
  if (!title || !body) {
    return false;
  }

  try {
    const granted = await ensureNotificationPermission();
    if (!granted) {
      console.warn("[desktop-notification] permission denied", { title, body });
      return false;
    }

    await sendNotification({ title, body });
    return true;
  } catch (error) {
    console.warn("[desktop-notification] failed to send", {
      title,
      body,
      error,
    });
    return false;
  }
}

async function ensureNotificationPermission() {
  if (permissionResolved) {
    return permissionGranted;
  }

  permissionGranted = await isPermissionGranted();
  if (!permissionGranted) {
    permissionGranted = (await requestPermission()) === "granted";
  }

  permissionResolved = true;
  return permissionGranted;
}
