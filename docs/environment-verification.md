# Environment workspace verification — 2026-10-11

## Safeguards

- Working branch: `codex/environment-asset-workspace`.
- Source backup, including all pre-existing uncommitted work: `E:/dev/sprite-studio-environment-backup-2026-10-11`.
- Before-change inventory: `character-file-hashes.xml` in that backup records the 1,854 existing files below workspace projects, excluding environment sidecars/assets. Final SHA-256/inventory comparison: **1,854 before; 1,854 after; 0 changed/missing; 0 added**. The original character files are unchanged.
- No KangiFight Godot files were edited, no production artwork was generated, and no live environment metadata migration was applied.

## Automated checks

Final results on 2026-10-11:

| Check | Result |
| --- | --- |
| Frontend interactions (`npm test`) | 10 passed |
| TypeScript and Vite production build | Passed |
| Full Rust suite with installed Python/snapper | 63 passed, 0 failed, 2 intentionally ignored |
| Environment subset within Rust suite | 29 passed; 2 opt-in audit/demo tests ignored in the normal suite |
| Python cleanup regressions | 3 passed |
| Windows Tauri debug application build (`--no-bundle`) | Passed; `src-tauri/target/debug/sprite-studio.exe` |
| Rust formatting and tracked whitespace checks | Passed |
| npm dependency audit (including development dependencies) | 0 vulnerabilities |
| Character inventory and SHA-256 comparison | 1,854 files; 0 changed/missing/added |

The two opt-in tests are the live read-only audit (run separately) and the persistent synthetic-demo writer (not run). No installer/package signing was tested; the verified build is the local debug application. Existing unrelated uncommitted work was preserved rather than reset or committed with these changes.

Run from the Sprite Studio root:

```powershell
npm test
npm run build
$env:SPRITE_STUDIO_TEST_PYTHON = (Resolve-Path '.venv/Scripts/python.exe').Path
$env:SPRITE_STUDIO_TEST_SNAPPER = 'E:/dev/spritefusion-pixel-snapper-1.0.0/target/release/spritefusion-pixel-snapper.exe'
cargo test --manifest-path src-tauri/Cargo.toml
& '.venv/Scripts/python.exe' -m unittest discover -s src-tauri/python -p 'test_*.py' -v
npm run tauri build -- --debug --no-bundle
```

Coverage includes original bytes/dimensions, opaque sky, transparent full-canvas layers, exact Outback ground rule, custom packages, independent environment-only owners, props/pivots, tile-edge checks, preserve/fit/fill/pad transformations, deterministic PNG bytes, explicit palette mapping, cross-project/package isolation, per-layer invalidation, independent prop preservation on canvas edits, order persistence, source/processed/export hashes, approval gating, master-reference preservation, review-opacity isolation, single/package manifests, revision restoration, schema-1 backup/reopen, interrupted/corrupt metadata and newer-schema refusal. All existing character/recovery tests are also run with the installed local processing tools.

Frontend interaction tests use jsdom, not screenshots. They cover menu navigation with no selected character, independent project/package selection, searchable library, actual geometry controls, cancelled import, approval/export gating, compositor revision/opacity payloads, isolation of historical compositor previews from current library thumbnails, cross-kind export chronology, status derivation and multi-color text entry. Vitest uses its threads pool because fork-worker startup timed out on this machine.

The existing live Outback sidecar was audited read-only: six layers and the one registered source/revision image passed saved-hash verification, and metadata bytes were unchanged. The compatibility test separately exercises migration on synthetic copies, never the live workspace.

Reproduce the non-mutating audit against the existing project:

```powershell
$env:SPRITE_STUDIO_ENV_AUDIT_PROJECT = 'C:/Users/hp/OneDrive/Pictures/dev/sprites/projects/kangifight-character-dingoo-e374e855'
cargo test --manifest-path src-tauri/Cargo.toml --lib environment::tests::audit_existing_environment_read_only -- --ignored --nocapture
```

## Desktop visual review: NOT TESTED

The actual built `Sprite Studio` application launched successfully through the native Windows Computer Use API. Its accessibility tree showed the dedicated Character Animations / Environment Assets menu and the existing completed character workflow. Screenshot capture failed with `FrameArrived timed out` and then `window capture timed out` after refreshing window selection and activating/retrying. Element clicking reported `coordinate input geometry is unavailable`. Text accessibility was available, but it is not visual sign-off. No screenshots are claimed.

Manual desktop review still required:

1. Start `npm run tauri dev`. At default and minimum supported window sizes, verify navigation, scrolling, text clipping, focus states and status-bar layout.
2. Open Environment Assets without changing the selected character. Create a new environment-only project and Custom package in a temporary workspace. Confirm the independent picker and package overview.
3. Import representative opaque/transparent PNGs; capture Asset library, source/processed Asset editor and palette validation views. Compare 1× and 4× sharpness; test Cancel and a damaged PNG.
4. Use explicit Fit/Pad/Fill on an aspect-mismatched source and inspect padding/crop/stretch warnings. Confirm defaults do not trim or recenter full-canvas layers.
5. Capture Layer compositor after changing visibility, order, review opacity and reference overlay; check the ground/HUD/marker guides. Confirm exported review composites exclude guides/reference and production PNGs retain intended alpha.
6. Approve each reviewed revision, capture Export & validation, export single/package results and verify Open folder points to the exact displayed directory.
7. Restart, reopen the package and inspect current selections, histories and export folders. Restore an older image/configuration explicitly. Change one layer's settings and confirm other packages and character completion states remain unchanged.
8. Open the existing Outback package read-only. Confirm its six slots, 480×270 canvas, ten approved colors and Y=224 surface validation before making any edits.
