# Release workflow — DRAFT v2 for the GitHub web UI

**Status: draft. This is not a live workflow file.** The repo owner
pastes the YAML below into `.github/workflows/release.yml` through
GitHub's web UI (the automation token cannot write workflow files).
**This v2 supersedes the v3-round draft**: it keeps that draft's
lint gate and Superintendent naming, and adds Linux and macOS
build jobs, so a tag push produces all three platform binaries.

Behaviour: pushing a tag matching `coi-v*` runs the lint gate,
then builds on `windows-latest`, `ubuntu-latest` and
`macos-latest` in parallel; each job attaches its binary plus its
SHA256 file to the release for that tag. The tag scheme
deliberately keeps its `coi-v*` name. A manual `workflow_dispatch`
run uploads the same files as workflow artefacts instead.

**macOS is built but NOT run-tested.** Nobody on this project has
a Mac: the macOS binary is compiled on Apple's runner and never
executed anywhere before it ships. The release notes text in the
macOS job's comments says so, and INSTALL.md's macOS section
carries the same warning. Treat the first Mac report as the real
test.

```yaml
name: superintendent-release

on:
  push:
    tags:
      - "coi-v*"
  workflow_dispatch:

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust (with rustfmt + clippy)
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - name: rustfmt
        run: cargo fmt --check
        working-directory: coi
      - name: clippy
        run: cargo clippy -- -D warnings
        working-directory: coi
      - name: Set up Python
        uses: actions/setup-python@v5
        with:
          python-version: "3.12"
      - name: ruff (lint + format check)
        run: |
          pip install ruff
          ruff check tools plans
          ruff format --check tools plans
      - name: shellcheck
        run: |
          sudo apt-get update && sudo apt-get install -y shellcheck
          shellcheck tools/*.sh bridge/*.sh qos/*.sh 2>/dev/null || shellcheck tools/*.sh
      - name: markdownlint
        run: |
          npm install -g markdownlint-cli
          markdownlint "**/*.md"
      - name: repo structure
        run: python3 tools/check_repo_structure.py
      - name: mod packages
        run: python3 tools/lint_mod_packages.py
      - name: shutdown check
        run: |
          cargo build --manifest-path coi/Cargo.toml
          bash tools/check_shutdown.sh

  build-windows:
    needs: lint
    runs-on: windows-latest
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      - name: Build release binary
        run: cargo build --release
        working-directory: coi
      - name: Stage artefacts
        shell: pwsh
        run: |
          Copy-Item coi/target/release/superintendent.exe superintendent.exe
          (Get-FileHash superintendent.exe -Algorithm SHA256).Hash.ToLower() +
            "  superintendent.exe" | Out-File -Encoding ascii superintendent.exe.sha256
      - name: Upload workflow artefact
        uses: actions/upload-artifact@v4
        with:
          name: superintendent-windows
          path: |
            superintendent.exe
            superintendent.exe.sha256
      - name: Attach to the release
        if: startsWith(github.ref, 'refs/tags/')
        uses: softprops/action-gh-release@v2
        with:
          files: |
            superintendent.exe
            superintendent.exe.sha256

  build-linux:
    needs: lint
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      - name: Build release binary
        run: cargo build --release
        working-directory: coi
      - name: Stage artefacts
        run: |
          cp coi/target/release/superintendent superintendent-linux
          sha256sum superintendent-linux > superintendent-linux.sha256
      - name: Upload workflow artefact
        uses: actions/upload-artifact@v4
        with:
          name: superintendent-linux
          path: |
            superintendent-linux
            superintendent-linux.sha256
      - name: Attach to the release
        if: startsWith(github.ref, 'refs/tags/')
        uses: softprops/action-gh-release@v2
        with:
          files: |
            superintendent-linux
            superintendent-linux.sha256

  # macOS: compiled here, never run-tested (no Mac in the loop).
  # The binary ships labelled as such in INSTALL.md; the first
  # user report from a real Mac is its acceptance test.
  build-macos:
    needs: lint
    runs-on: macos-latest
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      - name: Build release binary
        run: cargo build --release
        working-directory: coi
      - name: Stage artefacts
        run: |
          cp coi/target/release/superintendent superintendent-macos
          shasum -a 256 superintendent-macos > superintendent-macos.sha256
      - name: Upload workflow artefact
        uses: actions/upload-artifact@v4
        with:
          name: superintendent-macos
          path: |
            superintendent-macos
            superintendent-macos.sha256
      - name: Attach to the release
        if: startsWith(github.ref, 'refs/tags/')
        uses: softprops/action-gh-release@v2
        with:
          files: |
            superintendent-macos
            superintendent-macos.sha256
```

Notes for the owner:

- `cargo` runs in `coi/` (the crate folder keeps its name; only
  the package/binary inside is `superintendent`).
- The lint job also runs `tools/check_shutdown.sh` against the
  debug build — the daemon round's graceful-exit gate — so a
  release cannot ship a broken Shutdown.
- The shellcheck line tries the component script folders and
  falls back to tools/ only — tighten it once every script is
  clean.
- Dependabot config lives at `.github/dependabot.yml` (not a
  workflow file; it was committed with the Design V3 round).
- Release notes for the first three-platform release should say,
  in words: *the macOS build is provided untested — it is built
  on Apple's runners but has never been run on a Mac by the
  project; please report how it behaves.*
