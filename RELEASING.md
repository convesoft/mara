# Releasing Mara

Mara releases from `main` through `.github/workflows/release.yml`. The workflow
builds before approval and performs every public mutation only in the protected
`release` environment. Do not create the tag or publish npm packages manually.

## One-time repository setup

1. Create a GitHub environment named `release` and require the intended
   maintainer as a reviewer.
2. Merge `release.yml` to the default branch.
3. With npm CLI 11.15 or newer and an account that can administer all five
   packages, configure the same trusted publisher for each package:

```bash
for package_name in \
  @convesoft/mara \
  @convesoft/mara-linux-x64-gnu \
  @convesoft/mara-linux-arm64-gnu \
  @convesoft/mara-darwin-x64 \
  @convesoft/mara-darwin-arm64
do
  npm trust github "$package_name" \
    --file release.yml \
    --repository convesoft/mara \
    --environment release \
    --allow-publish \
    --yes
done
```

The workflow uses npm OIDC trusted publishing and stores no npm write token.

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
