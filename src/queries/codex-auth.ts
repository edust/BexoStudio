import {
  deleteCodexAuthProfile,
  importCurrentCodexAuthProfile,
  listCodexAuthProfiles,
  queryCodexAuthQuota,
  refreshAllCodexAuthQuotas,
  switchCodexAuthProfile,
  upsertCodexAuthProfile,
} from "@/lib/command-client";

export const codexAuthProfilesQueryKey = ["codexAuthProfiles"] as const;

export {
  deleteCodexAuthProfile,
  importCurrentCodexAuthProfile,
  listCodexAuthProfiles,
  queryCodexAuthQuota,
  refreshAllCodexAuthQuotas,
  switchCodexAuthProfile,
  upsertCodexAuthProfile,
};
