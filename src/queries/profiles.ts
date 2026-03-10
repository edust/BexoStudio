import { listCodexProfiles, upsertCodexProfile } from "@/lib/command-client";

export const profilesQueryKey = ["codexProfiles"] as const;

export { listCodexProfiles, upsertCodexProfile };
