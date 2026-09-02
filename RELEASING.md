# Releasing Mara

Mara releases from `main` through `.github/workflows/release.yml`. The workflow
builds before approval and performs every public mutation only in the protected
`release` environment. Do not create the tag or publish npm packages manually.

## Prepare a release

1. Create a release-preparation Linear issue and its short-lived branch from
   current `main`.
2. Select the version manually and update `[workspace.package].version`.
3. Generate `CHANGELOG.md` from the complete history:

   ```bash
   git-cliff --tag vX.Y.Z[-prerelease] --output CHANGELOG.md
   ```

4. Run the normal verification plus `scripts/smoke-npm.sh target/release/mara`.
5. Open one pull request with a Conventional Commit title and squash it to
   `main` after review. The changelog must describe the resulting squash commit
   history; regenerate it in a follow-up preparation commit if the title or
   history changed.

## Publish

From the GitHub Actions page, run **Release** on `main` with the exact version
without a leading `v`. Review the completed validation and build jobs, then
approve the `release` environment.

The protected job creates an annotated tag, a draft GitHub release, and checksums;
publishes and verifies the four native npm packages; publishes the dispatcher
last; runs a clean public `npx` smoke test; and finally publishes the GitHub
prerelease or release. Reruns accept only the same tag commit and byte-identical
already-published npm tarballs.

Prereleases publish under npm's `next` tag. Stable versions publish under
`latest`. User and MCP examples must pin exact versions.
