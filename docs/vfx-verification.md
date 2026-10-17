# VFX implementation verification — 2026-10-13

## Baseline

Before application changes: `npm test` passed **10/10** existing environment UI tests; `npm run build` passed; `cargo test --manifest-path src-tauri/Cargo.toml` passed **63**, with **2 explicitly ignored** audit/demo cases. A second baseline with the installed snapper and Python configured also passed 63. Character, generic animation, pose extraction, cleanup/alignment, exports, revision/recovery and environment source/docs were inspected. The KangiFight VFX spec was read only as an export-contract example.

Existing source was dirty before this phase and was preserved. Source plus persistent JSON were backed up to `E:/dev/sprite-studio-recovery-backups/vfx-20261013-86f78701-79a9-4b30-ad29-f384792e897c`; every existing workspace file was hashed (2,425). See `.vfx-work/baseline/` for logs/backup/hash records.

## Implementation evidence

New Rust tests cover actual PNG preservation/order, board and explicit sheet rectangles, disconnected one-pixel particles, asymmetric expansion, stable origin/anchor, chroma/threshold/hidden RGB, no default rescale, clipping, exact palette vs explicit remap, permitted empty terminal export, approval/exact targets, positive integer holds, deterministic exports, hashes, reopen, draft recovery/staleness, independent effects and unchanged character/environment metadata. Frontend tests cover navigation, new effects, import routing/cancellation, exact region review, palette input, tick editor/FPS isolation, approval/export gates, saved drafts, non-loop/hit-stop clock, mirroring and native-size preview.

The explicit synthetic round-trip uses the real VFX implementation, not a UI mock: three authored PNGs → original-byte import → region approval → fixed-cell processing → test-owner approval → two identical PNG/manifest exports → saved-byte verification → reopen. Output: 72×24, anchor (12,12), holds 2/3/3, eight advancing ticks, binary alpha and exact proposed RGB. Test-owner approval exercises the gate; it is not approval of production AI artwork. Blank-terminal/padding export is a separate real-PNG test.

## Reproduce

```powershell
cd E:/dev/sprite-studio
npm test
npm run build
$env:SPRITE_STUDIO_TEST_SNAPPER = 'E:/dev/spritefusion-pixel-snapper-1.0.0/target/release/spritefusion-pixel-snapper.exe'
$env:SPRITE_STUDIO_TEST_PYTHON = 'E:/dev/sprite-studio/.venv/Scripts/python.exe'
cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
```

The installed-tool character tests must have those environment variables to prove external snap/cleanup behavior. A parallel post-change run had one existing Python-launch failure in `installed_batch_auto_fit_preserves_applied_frames_until_review_apply`; its exact isolated rerun and the complete serial run passed. No character processing code/test gate was weakened. Retained failed-run log: `.vfx-work/cargo-regressions.log`.

Default Rust tests explicitly ignore the existing live-environment audit, existing static-demo writer and new opt-in VFX evidence writer. The VFX round-trip itself is also a normal nonignored temporary-directory test. To preserve a separate demo, choose a **new** directory:

```powershell
$env:SPRITE_STUDIO_VFX_DEMO_DIR = 'E:/dev/sprite-studio/.vfx-work/roundtrip-20261013'
cargo test --manifest-path src-tauri/Cargo.toml --lib vfx::tests::write_synthetic_round_trip_evidence -- --ignored --exact --nocapture
npm run tauri build -- --debug --no-bundle
```

The demo writer refuses existing folders. It writes `round-trip.json`, original inputs, VFX state, revisions and two deterministic export folders. Final command logs and a machine-readable verification report live in `.vfx-work/final/`. The walkthrough and contract are in [vfx-workspace.md](vfx-workspace.md).

## Desktop and AI review status

Final results: **24 frontend tests passed; 79 Rust tests passed, 0 failed, 3 explicitly ignored in the default suite; 3 Python cleanup tests passed; frontend and Tauri desktop builds passed**. The ignored live-environment audit and new VFX evidence writer were executed separately and each passed; the unrelated static-environment demo writer remains intentionally unexecuted. The configured full Rust suite was run serially. All **2,425 existing workspace files and app settings remained byte-identical**. Exact changed-file inventory compared with the pre-change source backup contains **19 files**.

Saved evidence: [machine-readable verification](../.vfx-work/final/report.json), [preservation/file inventory](../.vfx-work/final/preservation.json), [synthetic round-trip and exact PNG/manifest paths](../.vfx-work/roundtrip-20261013/round-trip.json). The sheet hash is `8387a7a3f1757d1e71805ea7e87714e0abcabfd5378f7e407609aaf21fc29e33`; both exports from the same approved revision match. Its palette authority ID remains null. These are synthetic test artifacts, not approved production AI assets.

Built desktop launch: VERIFIED. Native accessibility: Character Animations, Environment Assets and Visual Effects present. Screenshot/layout review: **NOT TESTED** after fresh-window retry failed. No browser rendering is presented as a desktop screenshot. Follow the guide's manual walkthrough; review actual AI inputs separately. Synthetic tests do not verify AI-image cleanup or production aesthetic quality. Engine import adapters and game-specific consumption remain unimplemented.

## Exact files in this phase

Added:

- `src-tauri/src/asset_io.rs` — shared existing PNG/hash/confined-write/sidecar primitives.
- `src-tauri/src/vfx.rs` — VFX domain, import, processing, approvals, persistence and export.
- `src-tauri/src/vfx_tests.rs` — real PNG/round-trip/recovery/regression tests.
- `src/VfxWorkspace.tsx` — library, four workflow views, fixed-cell/tick viewer and review/history.
- `src/vfxTypes.ts`, `src/services/vfx.ts`, `src/domain/vfx.ts`, `src/vfx.css` — typed desktop boundary, clock/anchor logic and scoped controls.
- `tests/vfx.test.tsx` — frontend/domain interaction tests.
- `docs/vfx-workspace.md`, `docs/vfx-verification.md` — contract, walkthrough and evidence.

Modified:

- `src/App.tsx` — third destination and scoped navigation/drop behavior; existing routes retained.
- `src/styles.css` — imports scoped VFX styles.
- `src-tauri/src/lib.rs` — registers the VFX command and shared module.
- `src-tauri/src/environment.rs` — delegates unchanged PNG/hash/write primitives and metadata rotation to shared helpers.
- `package.json`, `package-lock.json` — scoped daisyUI button/badge dependency.
- `.gitignore` — excludes local verification artifacts.
- `README.md` — links the VFX workflow.

Character processing Python, `studio.rs`, character store/types, Environment Assets React components and KangiFight files received no changes in this phase. Existing earlier changes in those files were retained.
