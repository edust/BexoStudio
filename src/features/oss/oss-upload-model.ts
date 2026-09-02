export const OSS_UPLOAD_ENQUEUE_CONCURRENCY = 4;

export type OssUploadStartInput = {
  targetId: string;
  localPath: string;
  objectKey: string;
  overwrite: boolean;
};

export type OssUploadCandidate = {
  localPath: string;
  objectKey: string;
  name: string;
};

export type OssUploadRejection = {
  localPath: string;
  name: string;
  reason: string;
};

export type OssUploadBatchResult = {
  accepted: OssUploadCandidate[];
  rejected: OssUploadRejection[];
};

export function buildOssUploadObjectKey(parentPrefix: string, name: string) {
  const normalizedPrefix = parentPrefix.trim();
  const prefix = normalizedPrefix && !normalizedPrefix.endsWith("/")
    ? `${normalizedPrefix}/`
    : normalizedPrefix;
  return `${prefix}${name}`;
}

export function prepareOssUploadCandidates(
  paths: string[],
  parentPrefix: string,
): OssUploadBatchResult {
  const accepted: OssUploadCandidate[] = [];
  const rejected: OssUploadRejection[] = [];
  const objectKeys = new Set<string>();
  for (const localPath of paths) {
    const name = localPath.split(/[\\/]/).filter(Boolean).at(-1) ?? "";
    if (!localPath || !name || [...localPath].some(isControlCharacter)) {
      rejected.push({ localPath, name: name || "未命名文件", reason: "本地路径无效" });
      continue;
    }
    const objectKey = buildOssUploadObjectKey(parentPrefix, name);
    if (objectKeys.has(objectKey)) {
      rejected.push({ localPath, name, reason: "批次内存在相同目标文件名" });
      continue;
    }
    objectKeys.add(objectKey);
    accepted.push({ localPath, objectKey, name });
  }
  return { accepted, rejected };
}

export async function enqueueOssUploadPaths(
  paths: string[],
  targetId: string,
  parentPrefix: string,
  start: (input: OssUploadStartInput) => Promise<unknown>,
  concurrency = OSS_UPLOAD_ENQUEUE_CONCURRENCY,
): Promise<OssUploadBatchResult> {
  const prepared = prepareOssUploadCandidates(paths, parentPrefix);
  const accepted: OssUploadCandidate[] = [];
  const rejected = [...prepared.rejected];
  let cursor = 0;
  const workerCount = Math.min(Math.max(1, concurrency), prepared.accepted.length);

  const worker = async () => {
    while (cursor < prepared.accepted.length) {
      const index = cursor;
      cursor += 1;
      const candidate = prepared.accepted[index];
      try {
        await start({
          targetId,
          localPath: candidate.localPath,
          objectKey: candidate.objectKey,
          overwrite: false,
        });
        accepted.push(candidate);
      } catch (error) {
        rejected.push({
          localPath: candidate.localPath,
          name: candidate.name,
          reason: error instanceof Error ? error.message : "文件未能加入上传队列",
        });
      }
    }
  };

  if (workerCount > 0) {
    await Promise.all(Array.from({ length: workerCount }, () => worker()));
  }
  return { accepted, rejected };
}

function isControlCharacter(value: string) {
  const codePoint = value.codePointAt(0) ?? 0;
  return (codePoint >= 0 && codePoint <= 0x1f) || (codePoint >= 0x7f && codePoint <= 0x9f);
}
