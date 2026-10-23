# Contributing and releases

## Pull requests and CI

All changes to `main` go through a pull request. No approving review is required
(appropriate for a solo-maintainer project). The GitHub branch ruleset requires
`CI required`, which succeeds only when all of these jobs pass:

- `cargo fmt --check` and `cargo clippy --locked --all-targets -- -D warnings`;
- Python tooling tests (`python -m unittest discover -s scripts -p 'test_*.py'`);
- `cargo test --locked` on Linux x64, Windows x64, macOS ARM64 and macOS x64;
- building and smoke-testing archive packaging on those same four platforms.

PR branches must be up to date with `main`. Force pushes and deletion of `main`
are blocked; the ruleset has no bypass actors, including administrators.
Protection is a GitHub repository setting, not something the workflow YAML can
apply. Repository administrators can edit the ruleset if recovery is necessary.

GPU rendering tests are marked `#[ignore]` and are **not** covered by hosted CI.
Run them locally on supported hardware when changing rendering code:

```sh
cargo test --release gpu_ -- --ignored --nocapture --test-threads=1
```

## Semantic versioning

`Cargo.toml` is the source of truth; keep its version and the root package entry
in `Cargo.lock` synchronized. Use `MAJOR.MINOR.PATCH` and tags `vMAJOR.MINOR.PATCH`:

- patch for compatible fixes;
- minor for new functionality (during `0.x`, also use minor for breaking changes);
- major for incompatible changes once the application reaches `1.0`.

Save-file format versions are independent of application versions. Preserve old
data or add an explicit migration when changing a save format.

## Making a release

1. Create a branch, update `Cargo.toml`, and run `cargo check` to update
   `Cargo.lock`. Include release notes in the PR description.
2. Open a PR and wait for `CI required`. Merge the PR into `main`.
3. Tag the merged commit locally and push the tag, for example:

   ```sh
   git switch main
   git pull --ff-only
   # Confirm that Cargo.toml says 0.1.1 before using this example tag.
   git tag -a v0.1.1 -m "Stargaze 0.1.1"
   git push origin v0.1.1
   ```

4. The Release workflow checks that the tag matches `Cargo.toml` and that the
   commit is reachable from `main`. It runs non-GPU release-mode tests, builds
   all four native targets, and creates archives with the executable, `configs/`,
   README, and embedded font license notices. It extracts each archive and checks
   `--version` and every bundled configuration without opening a window.
5. Only after every build succeeds does it publish a GitHub Release with generated
   release notes, `.zip` (Windows), `.tar.gz` (Linux/macOS), and `SHA256SUMS`.
   Review the generated notes afterward. No signing/notarization is configured.

Do not move published version tags. If a build fails before publication, fix the
cause through a PR and issue a new version tag, or rerun the failed workflow if
it was a transient runner/network issue. No binary release is created merely by
merging a PR, and no version bump is automatic.

Linux binaries are built on Ubuntu 24.04 (glibc 2.39+); older distributions are
not supported by these archives. macOS targets use the selected hosted runner's
SDK/deployment defaults. The app needs a supported GPU/driver and graphical
session on every platform.

## Local packaging check

With Python 3.12+ and Rust installed, substitute your native Rust target:

```sh
cargo build --locked --release --target aarch64-apple-darwin
python3 scripts/package_release.py --target aarch64-apple-darwin --tag v0.1.0
```

Archives are written to ignored `dist/`. Extract them in full: `configs/` must
remain beside the executable. Personal progress/settings live in native user
directories (see the README), not in the extracted application directory.
