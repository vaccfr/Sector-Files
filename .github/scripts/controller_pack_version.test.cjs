const assert = require("node:assert/strict");
const test = require("node:test");
const { compareVersions } = require("./controller_pack_version.cjs");

test("only newer upstream versions require an issue", () => {
  assert.equal(compareVersions("v2.0.0", "2.0.5"), -1);
  assert.equal(compareVersions("v2.0.5", "2.0.5"), 0);
  assert.equal(compareVersions("v2.0.6", "2.0.5"), 1);
});

test("stable versions supersede prereleases", () => {
  assert.equal(compareVersions("v2.0.0", "2.0.0-beta.6"), 1);
  assert.equal(compareVersions("2.0.0-beta.10", "2.0.0-beta.6"), 1);
  assert.equal(compareVersions("2.0.0-beta.6", "2.0.0"), -1);
});

test("four-part plugin versions and common tag prefixes", () => {
  assert.equal(compareVersions("v0.15.0.3", "0.15.0.2"), 1);
  assert.equal(compareVersions("release/v0.15.0.3", "0.15.0.3"), 0);
  assert.equal(compareVersions("unknown", "2.0.5"), null);
});
