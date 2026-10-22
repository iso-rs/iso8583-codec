# Publishing

The crate is published by [`.github/workflows/publish.yml`](../.github/workflows/publish.yml)
on `v*` tags. The same workflow can be started manually as a dry run.

## One-time setup

Trusted publishing can only be configured after a crate exists on crates.io, so
the first release needs either a local publish or a repository secret.

### Option A: publish the first version locally

```text
cargo login
cargo publish -p iso8583-codec
```

Create the token at <https://crates.io/settings/tokens> with the `publish-new`
scope (and `publish-update` for later releases).

### Option B: publish the first version from GitHub Actions

1. Create a token at <https://crates.io/settings/tokens> with the `publish-new`
   scope.
2. Add it as the repository secret `CARGO_REGISTRY_TOKEN`
   (Settings → Secrets and variables → Actions).
3. Push a tag after the version check below. The workflow uses the secret when
   it is present.

### Recommended: switch to trusted publishing

After the first version is on crates.io:

1. Open <https://crates.io/crates/iso8583-codec/settings> → **Trusted
   Publishing**.
2. Add a GitHub publisher:
   - Repository owner: `iso-rs`
   - Repository name: `iso8583-codec`
   - Workflow filename: `publish.yml`
   - Environment: `crates-io`
3. Create the matching GitHub environment: repository **Settings** →
   **Environments** → **New environment** → `crates-io`.
   If you add deployment rules, allow the `v*` tag pattern; otherwise tag
   pushes cannot enter the environment.
4. Delete the `CARGO_REGISTRY_TOKEN` repository secret. As long as it exists,
   the workflow uses the long-lived token instead of OIDC.
5. Optionally add required reviewers to the `crates-io` environment so every
   publish waits for approval.

With the secret gone, the workflow authenticates through GitHub OIDC using
`rust-lang/crates-io-auth-action` and a short-lived crates.io token. No
long-lived credential is stored.

The trusted publisher configuration must match all four values exactly:
repository owner, repository name, workflow filename and environment.

### Verifying trusted publishing

A version can only be published once, so `0.1.0` cannot be reused to test the
OIDC path. The next release validates it end to end:

1. Bump the version, for example to `0.1.1`.
2. Update `CHANGELOG.md`, commit and push.
3. Tag and push:

   ```text
   git tag v0.1.1
   git push origin v0.1.1
   ```

4. Open the **Publish** workflow run. The **Authenticate with crates.io** step
   should succeed without any repository secret, followed by the publish step.

## Releasing a version

1. Bump `[workspace.package].version` in the root `Cargo.toml`.
2. Add the release to `CHANGELOG.md`.
3. Run the checks:

   ```text
   cargo fmt --all -- --check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test --all-targets --all-features
   cargo test --doc --all-features
   cargo publish -p iso8583-codec --dry-run
   ```

   If your local Cargo configuration replaces crates.io with a mirror, add
   `--registry crates-io` to the publish commands.

4. Commit the release changes.
5. Tag and push:

   ```text
   git tag vX.Y.Z
   git push origin main
   git push origin vX.Y.Z
   ```

The workflow refuses to publish when the tag does not match the crate version
(`vX.Y.Z` must match `version = "X.Y.Z"`). crates.io does not allow overwriting
a published version, so a failed release needs a version bump rather than a
re-tag.

## Manual dry run

Run the **Publish** workflow from the Actions tab with `dry_run` enabled (the
default). It executes `cargo publish --dry-run`, which packages and verifies the
crate without uploading it or requiring crates.io credentials.

## Troubleshooting

- **`crate version ... is already uploaded`** — bump the version; published
  versions are immutable.
- **`no token found`** — add `CARGO_REGISTRY_TOKEN` for the first release, or
  configure trusted publishing and make sure the environment is `crates-io`.
- **Trusted publishing mismatch** — the owner, repository, workflow filename and
  environment on crates.io must match this repository and `publish.yml`
  exactly.
- **Tag/version mismatch** — update the version in `Cargo.toml` or delete and
  recreate the tag.
