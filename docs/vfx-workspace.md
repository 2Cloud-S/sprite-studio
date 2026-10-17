# Visual Effects workspace

Visual Effects is a separate main-navigation destination alongside Character Animations and Environment Assets. It uses Sprite Studio's React workbench, Rust image library, workspace lookup/operation lock and shared PNG/hash/confined-write/durable-sidecar helpers. Character cleanup and its opaque-bounds/bottom-center normalization are never called by VFX operations. No game project, runtime adapter or effect simulation is changed.

## User walkthrough

1. From `E:/dev/sprite-studio`, run `npm run tauri dev`, or open the built `src-tauri/target/debug/sprite-studio.exe`. Keep your current Sprite Studio workspace selected. Choose **Visual Effects**, enter a collection name under **New VFX project**, and click **Create VFX project**. This does not change the selected character or any environment package.
2. Click **New effect**. Set its ID, display name and optional package association. **Custom** is the default: editable 32×32 cell, (16,16) anchor, unrestricted RGB and unset orientation/mirroring. These are editor starting values, never production authority. Alternatively select **PROPOSED Light Contact** for `effect.contact.light`, a 24×24 cell, (12,12) anchor, east source, horizontal anchor mirror, non-looping playback, proposed #FFF1D2/#C9C2B8/#38344B subset and pause-on-hit-stop. No palette authority ID is invented.
3. In **Import & regions**, choose individual PNGs, a horizontal board with proposed equal-width regions, or a transparent sheet with explicit `[x,y,width,height]` rectangles. Original bytes are copied unchanged. Inspect the overlaid source rectangles and individual crop previews, including all detached sparks and padding. Edit coordinates, add/remove regions and reorder them. Click **Preview edited regions**, enter the region reviewer and **Approve these frame regions**. Editing a region is not approval. Horizontal segmentation does not use connected-body detection; uneven boards need manual rectangles.
4. In **Process & align**, set the common cell and contact anchor. Leave each frame at offset (0,0), pitch 1, no chroma and no alpha threshold to preserve native RGBA positions. All translations are explicit integer offsets from the rectangle's top-left origin. Larger pitch is an explicit nearest-neighbor downsample; the complete source rectangle must divide evenly. Inspect one-pixel particles carefully before approving any pitch change.
5. If needed, explicitly select chroma and tolerance or an alpha threshold (1–255). Transparent RGB is normalized to zero. Connected-component/speckle removal is disabled; no foreground-bounds trimming, centering, bottom alignment or automatic fit occurs. Palette validation reports exact unexpected RGB values. Select **Explicit RGB remap** only when you intend conversion; it never occurs merely because a palette is selected. Named palette profiles can be exchanged as JSON; package references and authority IDs are user-supplied and optional. A blank authority ID remains `UNASSIGNED`, including after local RGB-subset review.
6. Click **Create non-destructive processing preview**. Compare the source and result. Clipped opaque pixels, non-binary alpha, unexpected colors and invalid empty frames block approval/export. Edge contact and possible green fringes require review; green is not automatically considered noise. Alpha/chroma/pitch operations can intentionally remove pixels, so inspect their previews. A technically valid image is not proof of artistic or semantic adequacy.
7. Open **Timing & preview**. Author a positive integer hold for every ordered frame and set simulation frequency, looping, orientation, mirror, hit-stop and completion policies. The three-frame Light Contact study proposes **2,3,3 = 8 advancing ticks**, at editable 60 Hz. All remain proposed until owner approval. Preview FPS changes presentation sampling only; it does not overwrite holds or exported frequency. Test Play/Pause, tick/frame stepping, 1×, integer zoom, checker/dark/light, anchor crosshair and mirror. Simulated hit stop with `pause` freezes age and consumes no holds. Optional grayscale and temporary background are review-only.
8. Choose **Hide renderer at approved expiry**, or explicitly permit a fully transparent last frame and select **Finish on approved empty terminal frame**. Other empty frames fail validation. Empty-terminal completion requires non-looping playback. No fake pixel is added. Click **Save timing/layout as new review revision** to process the exact draft configuration.
9. In **Review & exports**, inspect each frame's actual reports and hashes. Enter the owner and explicitly check artwork/particles, regions/anchor/pitch, palette, holds/loop and clock/completion. Click **Approve exact VFX revision**, choose gutter (zero supported), then **Export approved VFX**. This is local owner approval of the asset, not game-engine production approval. **Verify saved export** rechecks actual saved bytes, originals, hashes and pixels against that approved revision; **Open export folder** opens that exact result.
10. Reopen the app and select the same VFX project/effect. Valid draft layout/palette/hold changes autosave after 250 ms, with a final save attempt when switching effects; **Save draft settings now** gives an explicit save. Wait for saved status before closing, and correct invalid/partial fields before expecting them to persist. Unsubmitted rectangle edits need **Preview edited regions**. Saved draft edits block stale approval/export until processed. History restores a whole prior revision, including its profile, frames and holds; **Use frame N settings** restores individual settings/holds from the same approved import into a new draft. Prior source files, reviews and exports remain intact.

## Export contract

Each export has a new immutable folder containing:

- `spritesheet.png`: horizontal full cells, selected gutter, no outer padding. Three 24×24 cells with gutter 0 yield 72×24. No guides, checkerboard, contrast filter, temporary background or preview mirror is baked in.
- `manifest.json`: versioned **`sprite-studio.vfx.v1`** metadata. It records effect/project/package identity, revision/hash/processor, original hashes, PNG/source-manifest hashes, cell/sheet sizes, anchor, explicit frame order/rectangles, processing, per-frame `hold_ticks`, total advancing ticks, simulation Hz, loop/clock/completion/orientation/mirror/palette and exact owner approval. `processingOffset` is already baked into the cell; runtime `offset` is [0,0], explicitly marked `placementBakedIntoCell`, to avoid applying a translation twice.
- `source.json` and `raw/<source-id>.png`: original-byte provenance and unchanged imported PNGs, including whole boards. They are not normalized replacements.
- `review-frames/NNN.png`: optional review sequence supplied by this MVP. No GIF is required; a GIF/preview FPS would not be timing or alpha authority.

The referenced KangiFight specification is example package data. A future game adapter would copy Sprite Studio's **manifest.json** byte-identically to `contact_light.source.json` and derive its own approved runtime metadata. That filename does **not** mean renaming this export's separate original-provenance `source.json`. The export states `engineAdapter: NOT_IMPLEMENTED`; it does not claim Godot or KangiFight consumes this schema. Production palette-bank identity and game-specific policies remain the responsibility of their owner.

Anchor coordinates are whole-pixel canvas boundary coordinates measured from (0,0). Horizontal mirroring maps pixel index `x` to `2*anchorX-1-x`, preserving that boundary point. Off-center anchors can move mirrored pixels outside the cell; the preview warns. Export remains the original source-facing cells.

## Persistence and recovery

```text
<workspace>/vfx-projects/<project-id>/owner.json
<workspace>/vfx-projects/<project-id>/effects/<effect-id>/
  index.json / index.previous.json / index.pending.json
  history/<snapshot-id>.json
  raw/<source-id>.png
  revisions/<revision-id>/NNN.png
  exports/<export-id>/...
```

IDs used as folders are generated UUIDs, separate from the user's semantic effect ID. Each effect owns its metadata generations and artifacts; editing one does not invalidate another. Shared sidecar saving writes a durable history snapshot before pending/previous rotation. VFX recovery chooses the latest complete valid generation, retains damaged/interrupted files and requires owner reapproval before a recovered state exports. Newer schemas are refused. There is no migration or overwrite of character/environment metadata. Sources and generated files use confined, exclusive writes and SHA-256 verification. All operations use the existing workspace lock.

The pre-change source and persistent JSON backup is recorded in `.vfx-work/baseline/backup.json`; baseline hashes cover all 2,425 existing workspace files. The release does not perform a destructive auto-upgrade.

## Review boundaries

This release is PNG-only, CPU processing, up to 128 ordered regions, 32 MiB per PNG / 256 MiB per import, 1–2048 native cells per axis and at most 4096 pixels sheet width. Explicit image/pitch/alpha/palette changes require human comparison. Geometry checks cannot recognize an AI effect's intended contact core, distinguish a mote from contamination, or certify semantic/aesthetic quality. The end-to-end evidence is synthetic; real AI-image cleanup is **NOT TESTED**.

Desktop visual review is **NOT TESTED**: the built app launched and native accessibility confirmed the three destinations, but screenshot capture returned `FrameArrived timed out` and then `window capture timed out`. Perform the walkthrough above at default/minimum window sizes, inspect the preview's 1×/zoom/mirror/padding and source/result comparisons, and verify saved exports/reopen before production use. No production VFX was generated, no fighter art was changed, and no engine/FeedbackDirector integration, particle simulator, DCC dependency or second persistence service was added.
