# VFX approval flicker investigation — 2026-10-14

## Status

The frontend autosave feedback loop was reproduced in a component regression using Rust-equivalent JSON ordering, then patched. Automated regressions pass. After the user closed the locked executable, the final Windows debug rebuild **passed** and the rebuilt app launched successfully. **Actual desktop visual acceptance remains NOT TESTED**: screenshot capture timed out, including a refreshed-window retry, and the VFX navigation click returned `coordinate input geometry is unavailable`. Text accessibility confirmed all three workspace menus and the existing completed character workflow, but does not establish VFX visual sign-off. An earlier attempt had exhausted the Computer Use request budget.

## Confirmed cause and before/after evidence

`EffectEditor` compared drafts with `JSON.stringify`. Its locally constructed draft had insertion order `profile, settings, holds, baseRevisionId, importId`. A saved draft returned through Rust's `serde_json::Value` had sorted object keys: `baseRevisionId, holds, importId, profile, settings`. These objects have identical values but different JSON strings.

The loop was:

```text
approve regions / edit settings
→ draft appears changed
→ autosave calls run(save_draft)
→ editor-wide busy=true disables the fieldset
→ backend returns an unchanged, sorted draft
→ accept rehydrates fields and clears review checkboxes
→ library refresh completes; busy=false
→ order-sensitive comparison still says changed
→ another autosave
```

This is an autosave/request/busy-state loop, **not repeated approval commands**. It is independent of Vite HMR. Region approval can initiate it when the newly accepted import receives default frame settings/holds. Reopening an effect with a saved draft or editing a processed effect can initiate the same loop. Processed-revision approval shares the affected editor but is not itself the key-order mismatch.

The defective component failed the newly added idle regression (`expected 2 saves to be <= 1`). Deferred-response coverage also exercises intermediate disabled-state transitions, not just the final state. The corrected 30-second **virtual-time component** regression records at most one initial draft save after region approval, one approval command, and no editor disablement during autosave. Reopening an unchanged approved saved draft produces zero idle commands. This evidence is not a 30-second native visual observation.

Affected controls inherited `disabled={busy}` from `.vfx-editor-body`: import kind/import button, region controls, processing settings, timing/player controls, review checkboxes and export/history actions. Background library reads never needed to disable all of these controls. No VFX filesystem watcher, backend polling interval or repeated event subscription exists in the inspected code. Rust already suppresses identical draft metadata writes, but the old UI still repeatedly requested full load/preview replies and toggled busy state.

## Fix

- Canonical value keys sort **object members only**. Frame, coordinate, hold and palette-array order remain significant. Draft, dirty and region comparisons no longer depend on IPC key order.
- Foreground operations have explicit identities (`process`, `approve_import`, `approve`, etc.). Background draft saving has its own state and does not disable the editor.
- Per-editor command serialization, immediate click guards and in-flight draft deduplication prevent overlapping mutations and duplicate picker/approval/save actions.
- Autosave acknowledgements update the saved snapshot without rehydrating newer local edits or resetting review checks. Only actual import/process/restore operations rehydrate the form. Checklists reset when revision identity changes.
- Opening requests have epochs; stale opening responses and responses to unmounted effects are ignored. Confirmed generations cannot be replaced with older snapshots. Library refreshes have their own latest-response guard and do not extend foreground busy state.
- Failed saves retain local edits, stop automatic retries and offer **Retry draft save**. Switching effects retains the existing final draft flush, deduplicated against pending/confirmed saves.
- Already-approved exact revisions cannot be accidentally approved again through the same enabled button. Technical validation, dirty-state checks and export gating remain intact.
- Optional bounded, memory-only diagnostics record operation/state transitions; no artwork, reviewer names, payloads or file paths are logged. They are disabled by default and add no polling, storage writes or migration.

No production Rust processing, persistence or export-format code changed. The Rust additions are tests/read-only audit only. Existing scoped daisyUI controls were retained. React guidance informed primitive dependencies, snapshot identity and async cancellation/ordering; Computer Use was attempted but could not provide desktop evidence.

## Light Contact data and preservation

The live project was inspected, **not reset, reimported or reapproved**:

| Saved item | Actual value |
| --- | --- |
| Project / effect storage IDs | `5e11b831-4815-4b2a-acfe-29cdf804e462` / `4b2d5c7f-b504-4e11-a4a8-4facc1d5446d` |
| Effect | `effect.contact.light` |
| Original | `Pixel Art Impact Spark Sprite Sheet.png`, 2172×724 |
| Original SHA-256 | `bd5a4ec4cca783f7b4cc78cf4f24c5c02ebb4e1e54c5c247ab3effe08c3d3360` |
| Approved import | `aa2bb28a-a16f-48d5-b8c8-8f5688e11f76`, reviewer Afnan |
| Regions | `[0,0,724,724]`, `[724,0,724,724]`, `[1448,0,724,724]` |
| Active processed revision | `5a89e4f4-9aa3-4eab-baaf-38a86e006957` |
| History / exports | Two processed revisions, neither approved; zero exports; generation 8; no saved draft |
| Saved runtime | 24×24 cell; anchor (12,12); east; non-looping; holds 2/3/3; 8 advancing ticks; hit stop pauses |
| Palette | `#FFF1D2`, `#C9C2B8`, `#38344B`; enforced; authority ID null |

These values were read from actual data, not substituted from the proposed study. The three source regions cover the entire original board, including disconnected particles and padding; nothing was component-pruned. The read-only Rust audit verified the original plus all six processed-frame hashes (7 total), exact active IDs and unchanged metadata.

**Separate existing artwork issue:** the active processed revision already reports 15,791 / 45,502 / 17,727 clipped nontransparent pixels and empty-frame errors. Its saved processing is pitch 1, offset (0,0), with no chroma/threshold/remap. Placing those 724×724 source regions directly into 24×24 is not valid. Those immutable failed previews were preserved, not silently repaired or artistically approved. The owner must explicitly create/review a corrected candidate before production approval/export. The UI fix does not weaken that gate.

Safeguards:

- Branch: `codex/fix-vfx-approval-loop`; existing dirty work preserved, no commit.
- Full workspace, app settings and relevant dirty source backup: `E:/dev/sprite-studio-recovery-backups/vfx-flicker-20261014-b4e8d18f-1318-4398-ab1e-56ceebaea109`.
- `workspace-hashes.xml` records 2,449 current files. After comparison: **0 changed/missing, 0 added; app settings byte-identical**.
- The older `.vfx-work/baseline/workspace-hashes.json` inventory was independently rechecked: **all 2,425 files unchanged**.
- Character/environment source code and all live sources, metadata, approvals and exports remain unchanged. No KangiFight repository files were edited; no destructive migration or storage reset occurred.

## Exact files changed in this fix

- `src/VfxWorkspace.tsx` — canonical comparisons, separate operation/save lifecycle, serialized commands, stale-response guards, preserved local edits, retry and state tracing.
- `src/domain/vfx.ts` — object-order-independent value identity.
- `src/services/vfx.ts` — opt-in request tracing; same Tauri command contract.
- `src/domain/vfxDiagnostics.ts` — new bounded memory-only diagnostic helper.
- `tests/vfx.test.tsx` — idle-transition, wire-order, region/processed approvals, reopen, races, deduplication, StrictMode, import and failure regressions.
- `tests/vfx-diagnostics.test.tsx` — new diagnostic opt-in/bounds/privacy tests.
- `src-tauri/src/vfx_tests.rs` — unchanged-draft/load/library metadata and approval invariants; opt-in read-only live audit.
- This report.

Unrelated formatter-only changes to `environment.rs` and `lib.rs` were restored byte-for-byte to the starting backup. Other pre-existing dirty/untracked changes are not part of this fix.

## Verification

Results from this investigation:

- Frontend: **36 tests passed** (24 VFX, 10 environment, 2 diagnostics); includes 30-second virtual idle windows and intermediate enabled/disabled assertions.
- Full configured Rust suite: **80 passed, 0 failed, 4 intentionally ignored**. Includes character/pose/cleanup/recovery, environment and deterministic VFX round-trip/export regressions. The new live read-only audit was run separately and passed.
- Python cleanup: **3 passed**.
- TypeScript/Vite production build: passed.
- Native Tauri debug build: **passed** after the user closed the old app. `npm run tauri build -- --debug --no-bundle` produced `E:/dev/sprite-studio/src-tauri/target/debug/sprite-studio.exe` (23,807,488 bytes); the rebuilt app was launched through native Computer Use and its returned main window was inspected through text accessibility. No installer/signing verification claimed.
- Rebuilt executable SHA-256: `0acd10f8633a4d1aa82020537565f1b575c406a99884f3e933c420ea60ff330f`. A fresh post-launch comparison again found all 2,449 workspace files and app settings unchanged.
- Real desktop VFX approval/idle/resize/screenshots and development-vs-compiled comparison: **NOT TESTED**. Capture returned `FrameArrived timed out`, then `window capture timed out` after refreshing/activating the returned window. Text-only accessibility worked; the subsequent VFX navigation click failed with `coordinate input geometry is unavailable`. No screenshots/recording or completed native VFX interaction are claimed.
- No native render counts, filesystem-event traces or backend-call counts are claimed; component counters and backend metadata assertions are automated evidence only.

Reproduce automated checks:

```powershell
npm test
npm run build
$env:SPRITE_STUDIO_TEST_SNAPPER = 'E:/dev/spritefusion-pixel-snapper-1.0.0/target/release/spritefusion-pixel-snapper.exe'
$env:SPRITE_STUDIO_TEST_PYTHON = 'E:/dev/sprite-studio/.venv/Scripts/python.exe'
cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
& '.venv/Scripts/python.exe' -m unittest discover -s src-tauri/python -p 'test_*.py' -v
$env:SPRITE_STUDIO_VFX_AUDIT_WORKSPACE = 'C:/Users/hp/OneDrive/Pictures/dev/sprites'
cargo test --manifest-path src-tauri/Cargo.toml --lib vfx::tests::audit_existing_light_contact_read_only -- --ignored --exact --nocapture
```

## Manual desktop acceptance still required

1. The rebuilt debug app is already available at `E:/dev/sprite-studio/src-tauri/target/debug/sprite-studio.exe` and was reopened. Perform the checks below in that app. Then close it normally and repeat under `npm run tauri dev` to distinguish HMR from application behavior. Do not rebuild over a running executable.
2. Use a **copy** of the workspace to replay import and region approval without modifying the production project. Open Visual Effects → Light Contact, import its three-pose PNG, inspect all rectangles/particles, and approve once. Observe for at least **30 real seconds**: no repeated grey-out cycles, changing selected frame/revision or approval toggling.
3. Inspect live Light Contact read-only. Its approved regions must remain approved, while its existing failed processed previews correctly remain blocked. Do not reapprove failed art just to test a button.
4. In a temporary study with technically valid frames, process → review → approve once → export twice. Confirm no extra approvals, correct export availability, identical output hashes and preserved preview tick/selection across unchanged saves/refreshes.
5. Make a real settings change: only that draft should require review. Edit again while a save is pending; the acknowledgement must not revert the new value. Test failure/retry on disposable data.
6. Switch effects rapidly, then Character Animations and Environment Assets, then back. Close/reopen and compare IDs, regions, anchors, offsets, palette, holds and export histories.
7. Optional DevTools instrumentation (development build): set `window.__SPRITE_STUDIO_VFX_TRACE__ = []` **before** the approval. Inspect the same array after 30 seconds. Expect one `approve_import` or `approve` start/end pair; at most one default draft save after region approval; no further idle command starts; no alternating `busy` state. It retains the last 300 events only. Delete that property to disable diagnostics. Capture real screenshots/recording if available.

Acceptance remains pending until the real desktop checks pass. The confirmed component-loop fix is not presented as a full native visual sign-off.
