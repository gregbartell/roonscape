import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const rootManifest = JSON.parse(
  await readFile(new URL("../package.json", import.meta.url), "utf8"),
);
const bridgeManifest = JSON.parse(
  await readFile(
    new URL("../src/bridge/package.json", import.meta.url),
    "utf8",
  ),
);

test("the regular suite builds shared prerequisites once", () => {
  assert.equal(
    rootManifest.scripts.test,
    "npm run test:build && npm run test:built",
  );
  assert.equal(
    rootManifest.scripts["test:build"],
    "npm run build && npm run test:renderer:build",
  );
  assert.doesNotMatch(
    rootManifest.scripts["test:built"],
    /(?:npm run build|cargo build)/,
  );
});

test("standalone test commands delegate to built stages", () => {
  assert.equal(
    bridgeManifest.scripts.test,
    "npm run build && npm run test:built",
  );
  for (const name of [
    "smoke:ipc",
    "test:bare-x-fullscreen",
    "test:bridge",
    "test:fixture-launcher",
    "test:source-launcher",
  ]) {
    assert.match(rootManifest.scripts[name], new RegExp(`${name}:built$`));
  }
});

test("the repository check reuses the Bridge built by typechecking", () => {
  assert.match(rootManifest.scripts.check, /npm run typecheck/);
  assert.match(
    rootManifest.scripts.check,
    /npm run test:renderer:build && npm run test:built$/,
  );
});
