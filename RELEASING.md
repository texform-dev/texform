# Releasing TeXForm

TeXForm ships one lockstep version to crates.io, PyPI, and npm from a single source of truth: `workspace.package.version` in `Cargo.toml`. Every crate inherits it through `version.workspace = true`, maturin reads it for the Python package, and `packages/texform/package.json` is synchronized to it for npm.

Releases are driven by a **release PR** that `release-plz` maintains automatically. Merging that PR is the publish button.

## Pipeline at a glance

```
merge the release PR
  └─ release-plz  → publish crates to crates.io
                  → push the single tag vX.Y.Z (facade crate only)
                  → create the GitHub Release
       └─ tag triggers release.yml
                  → build + smoke-test wheels, sdist, and the npm package
                  → publish to PyPI and npm
                  → attach build artifacts to the GitHub Release
```

## Cut a release

`release-plz` keeps a release PR open and up to date on every push to `main`. To ship, review and merge it.

1. **Review the version bump.** It should move together across `Cargo.toml`, `CHANGELOG.md`, and `packages/texform/package.json` (the npm version is applied by a workflow step).
2. **Polish the changelog draft** into user-facing wording.
3. **Cover binding and wrapper changes.** `release-plz` adds no changelog entries for changes under `crates/texform-python/`, `crates/texform-wasm/`, `python/`, or `packages/texform/`; review those paths since the last release and add any by hand.
4. **Merge.** This publishes the crates, pushes the `vX.Y.Z` tag, and triggers `release.yml` to build, smoke-test, and publish to PyPI and npm.

## Release a binding- or wrapper-only change

`release-plz` detects changes from publishable Cargo crates only. Changes confined to these paths do **not** open a release PR on their own:

- `crates/texform-python/` and `crates/texform-wasm/` — the binding crates, marked `publish = false`.
- `python/` and `packages/texform/` — the Python and npm wrappers.

The release-PR job in `release-plz.yml` emits a warning when these paths change without a matching release PR. To ship such a change, use the manual entry:

1. Open a normal PR that bumps `workspace.package.version`, updates `CHANGELOG.md`, and runs `node .github/scripts/sync-versions.mjs` so `packages/texform/package.json` matches.
2. Merge that PR.
3. Run the `release-plz` workflow via **workflow_dispatch** on `main`. The dispatch path temporarily flips `release_always` to `true` so the release runs without a release-PR merge.

Do not use the manual entry to publish an arbitrary commit or to skip the review steps above.

## When a publish fails

Every publish step is safe to re-run:

- **crates.io** — `release-plz` skips crates already published at the current version.
- **PyPI** — `gh-action-pypi-publish` runs with `skip-existing: true`.
- **npm** — the publish step checks `npm view` first and exits cleanly if the version already exists.

If a version is already burned in one registry and cannot be reused, do not overwrite it — release the next patch instead. Versions are lockstep, so one patch bump covers all three registries.

## Registry configuration (reference)

Steady-state configuration. Touch this when rotating credentials, onboarding a maintainer, or renaming a workflow.

**GitHub repository**

- Environment `release` — gates the release-plz publish job and the PyPI/npm publish jobs. Use it for real publishing credentials and optional manual approval.
- Repository variable `RELEASE_ENABLED` — must be `true` for any publish to run. Keep this at repository scope because both release workflows read it before entering an environment.
- Repository secret `RELEASE_PR_TOKEN` — a fine-grained PAT scoped to `texform-dev/texform` with **Contents: read/write** and **Pull requests: read/write**. The release-pr job uses it to maintain the automated release PR without entering the `release` environment.
- Environment secret `RELEASE_PLZ_TOKEN` — a fine-grained PAT scoped to `texform-dev/texform` with **Contents: read/write** and **Pull requests: read/write**. The release job uses it to publish crates, push the release tag, and create the GitHub Release; the PAT is required because a tag pushed with the default `GITHUB_TOKEN` does not trigger the downstream `release.yml`.
- Do not configure `CARGO_REGISTRY_TOKEN` or `NODE_AUTH_TOKEN` in steady state. They were bootstrap-only registry tokens for the first `0.1.0` release before trusted publishing could be configured.

**Trusted publishing (OIDC).** All three registries publish via OIDC; no long-lived registry tokens are stored. Matching is case-sensitive.

| Registry | Configure |
| --- | --- |
| crates.io | repository `texform-dev/texform`, workflow `release-plz.yml` |
| PyPI | owner `texform-dev`, repository `texform`, workflow `release.yml`, environment `release` |
| npm | package owner/org, repository, workflow `release.yml`, environment `release` |

npm additionally requires Node ≥ 22.14.0, npm ≥ 11.5.1, and an exact `repository.url` in `packages/texform/package.json` that matches the GitHub repository — provenance fails otherwise.
