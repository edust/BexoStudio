import {
  deleteCodexAuthProfile,
  getCodexAuthProfileDetail,
  importCurrentCodexAuthProfile,
  listCodexAuthProfiles,
  queryCodexAuthQuota,
  refreshAllCodexAuthQuotas,
  switchCodexAuthProfile,
  upsertCodexAuthProfile,
} from "@/lib/command-client";

export const codexAuthProfilesQueryKey = ["codexAuthProfiles"] as const;
export const codexAuthProfileDetailQueryKey = (id: string) =>
  ["codexAuthProfileDetail", id] as const;

export {
  deleteCodexAuthProfile,
  getCodexAuthProfileDetail,
  importCurrentCodexAuthProfile,
  listCodexAuthProfiles,
  queryCodexAuthQuota,
  refreshAllCodexAuthQuotas,
  switchCodexAuthProfile,
  upsertCodexAuthProfile,
};
