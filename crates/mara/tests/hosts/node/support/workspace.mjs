// A browser workspace of a test's page beside a copy of the same fixture in CLI direct mode, so
// that each request compares the two (VER-HOST-COMPARISON): the copy receives the same requests
// and the same writes of the caller, and its files are the expected bytes.

import assert from "node:assert/strict";
import { mkdirSync, readdirSync, rmSync, rmdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

import {
  applied,
  changeSet,
  cli,
  copy,
  encode,
  filesOf,
  fixture,
  json,
  pairMids,
  replaceMids,
  withoutGeneratedRevisions,
} from "./hosts.mjs";

export class Compared {
  /** A workspace of the fixture `name`'s files but `.git` on `page`, and its CLI copy. */
  static async create(page, name) {
    const supplied = fixture(name);
    const created = await page.harness("create", encode(supplied.files));
    assert.deepEqual(created, {}, `the workspace of ${name} was not created`);
    return new Compared(page, supplied, copy(supplied.root));
  }

  constructor(page, supplied, root) {
    this.page = page;
    this.supplied = supplied;
    this.root = root;
  }

  /** The workspace's call `name` with `args`: `{ result }`, or `{ error }` with its reply. */
  browser(name, args = {}, options) {
    return this.page.harness("run", name, args, options);
  }

  /** The change set of the workspace. */
  async export() {
    const exported = await this.page.harness("export");
    assert.equal(exported.error, undefined, JSON.stringify(exported.error));
    return exported.changes;
  }

  /** The files of the workspace: the supplied files with its change set applied. */
  async files() {
    return applied(this.supplied.files, await this.export());
  }

  /** The copy's files but `.git`. */
  expectedFiles() {
    return filesOf(this.root);
  }

  /** The change set that the copy's writes made. */
  expectedChanges() {
    return changeSet(this.supplied.files, this.expectedFiles(), this.supplied.hashes);
  }

  /**
   * Runs `name` with `args` in the workspace and in CLI direct mode on the copy, and checks
   * that the workspace gives the copy's result, or failure, once the MIDs it generated are
   * paired with the copy's, and `normalize`, given each and whether it is the workspace's, made
   * them alike. Returns the copy's printed object.
   */
  async request(name, args = {}, normalize = (value) => value) {
    const ours = await this.browser(name, args);
    const theirs = cli(this.root, name, args);
    const pairs = pairMids(this.supplied.files, this.expectedFiles(), await this.files());
    const mids = new Set(pairs.values());
    const printed = ours.error ? ours.error.reply : ours.result;
    assert.ok(printed, `${name}: the workspace gave no reply: ${JSON.stringify(ours)}`);
    const paired = JSON.parse(replaceMids(JSON.stringify(printed), pairs));
    assert.deepEqual(
      normalize(withoutGeneratedRevisions(paired, mids), true),
      normalize(withoutGeneratedRevisions(theirs.json, mids), false),
      `${name} ${JSON.stringify(args)}`,
    );
    return theirs.json;
  }

  /** Gives the file at `path` the bytes `contents` in the workspace and in the copy. */
  async write(path, contents) {
    const bytes = Buffer.from(contents);
    assert.deepEqual(await this.page.harness("write", path, bytes.toString("base64")), {});
    mkdirSync(dirname(join(this.root, path)), { recursive: true });
    writeFileSync(join(this.root, path), bytes);
  }

  /**
   * Removes the file at `path` in the workspace and in the copy, with each directory above it in
   * the copy that it leaves empty, as the workspace's memory does (DES-BROWSER-WORKSPACE).
   */
  async remove(path) {
    assert.deepEqual(await this.page.harness("remove", path), {});
    rmSync(join(this.root, path));
    for (let directory = dirname(path); directory !== "."; directory = dirname(directory)) {
      if (readdirSync(join(this.root, directory)).length > 0) {
        break;
      }
      rmdirSync(join(this.root, directory));
    }
  }

  /** Checks that the workspace's files are the copy's, once their generated MIDs are paired. */
  async sameFiles() {
    const expected = this.expectedFiles();
    const files = await this.files();
    const pairs = pairMids(this.supplied.files, expected, files);
    const paired = new Map([...files].map(([path, bytes]) => [path, replaceMids(bytes.toString("utf8"), pairs)]));
    assert.deepEqual(
      json(Object.fromEntries(paired)),
      json(Object.fromEntries([...expected].map(([path, bytes]) => [path, bytes.toString("utf8")]))),
    );
  }
}
