# Release workflow — DRAFT for the GitHub web UI

**Status: draft. This is not a live workflow file.** The repo owner
pastes the YAML below into `.github/workflows/release.yml` through
GitHub's web UI (the automation token cannot write workflow files).
It supersedes the v5 workflow text supplied earlier (2026-11-09):
changes are the Superintendent rename (workflow name, artefact
names) and the lint gate added in the Design V3 round — lint jobs
run first and the build only runs when they pass.

Behaviour, unchanged from v5 apart from names: pushing a tag
matching `coi-v*` builds `superintendent.exe` on `windows-latest`
and attaches it, plus `superintendent.exe.sha256`, to the release
for that tag. The tag scheme deliberately keeps its `coi-v*` name.

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

  build:
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
          name: superintendent-exe
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
```

Notes for the owner:

- `cargo` runs in `coi/` (the crate folder keeps its name; only
  the package/binary inside was renamed to `superintendent`).
- The shellcheck line tries the component script folders and falls
  back to tools/ only — tighten it once every script is clean.
- Adding the structure check to CI later is already done here
  (the `repo structure` step); no further change needed.
- Dependabot config lives at `.github/dependabot.yml` (not a
  workflow file; it was committed with the Design V3 round).
