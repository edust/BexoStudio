import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

test("Tauri production runtime styles inherit the generated CSP nonce", () => {
  const indexHtml = readFileSync("index.html", "utf8");
  const providers = readFileSync("src/app/providers.tsx", "utf8");
  const tauriConfig = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));

  const nonceAnchorIndex = indexHtml.indexOf('id="bexo-runtime-style-nonce"');
  const runtimeBootstrapIndex = indexHtml.indexOf('id="bexo-runtime-style-bootstrap"');

  assert.notEqual(nonceAnchorIndex, -1, "index.html must expose Tauri's style nonce");
  assert.notEqual(runtimeBootstrapIndex, -1, "runtime style bootstrap must exist");
  assert.ok(
    nonceAnchorIndex < runtimeBootstrapIndex,
    "nonce anchor must be parsed before the runtime style bootstrap",
  );
  assert.match(indexHtml, /document\.createElement\s*=\s*function/);
  assert.match(indexHtml, /normalizedTagName\s*===\s*"style"/);
  assert.match(indexHtml, /element\.nonce\s*=\s*runtimeStyleNonce/);

  assert.match(providers, /RUNTIME_STYLE_NONCE_ELEMENT_ID/);
  assert.match(providers, /csp=\{runtimeStyleNonce \? \{ nonce: runtimeStyleNonce \} : undefined\}/);
  assert.equal(
    tauriConfig.app.security.dangerousDisableAssetCspModification,
    undefined,
    "the fix must not disable Tauri's production CSP nonce injection",
  );
});
