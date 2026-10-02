# Sprite Studio

Created by [Afnan Khan (@2Cloud-S)](https://github.com/2Cloud-S).

A local-first Windows desktop workbench for 2D character sprites. Milestones 1–3 create projects, preserve original anchors, recover the pixel grid, and review chroma cleanup/fixed-cell placement. Milestone 4 imports normalized animation frames, edits whole-pixel alignment with onion skin, and reviews timed playback. Milestone 5 creates deterministic draft spritesheets, GIF previews, and JSON manifests. The current milestone also imports pose boards, detects/curates raw poses, and reviews batch snap and cleanup/normalization. Image generation and a persistent queued job runner are not yet implemented.

## Run

Requires Node.js, npm, Rust with the Windows MSVC toolchain, and the Tauri 2 Windows prerequisites (including WebView2).
For milestone 2, build or install the local [Sprite Fusion Pixel Snapper CLI](https://github.com/Hugo-Dz/spritefusion-pixel-snapper), then select its `spritefusion-pixel-snapper.exe` in the app. The executable location and reported version are stored in local app settings; no hosted image service is used.
For milestone 3, install Python 3.13 and a local environment containing the pinned Pillow, NumPy, and OpenCV dependencies, then select that environment's `python.exe` in the app. For example:

```powershell
py -3.13 -m venv .venv
.\.venv\Scripts\python.exe -m pip install -r processing-requirements.txt
```

The app validates both image libraries before saving the Python path. The cleanup script is bundled in the Rust application and runs only against the selected project's applied native snap.

```powershell
npm install
npm run tauri dev
```

Verification:

```powershell
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
```

To include the end-to-end snap, cleanup, and export tests, set `SPRITE_STUDIO_TEST_SNAPPER` to the absolute Sprite Fusion executable path and `SPRITE_STUDIO_TEST_PYTHON` to the absolute Python executable path before running `cargo test`.

## Workspace and project contract

The first launch asks for a workspace folder. App settings live in the OS app-config directory as `app-settings.json`, with `workspaceRoot` and `lastProjectId`. The chosen workspace contains:

```text
<workspace>/
  projects/
    <name-slug>-<short-id>/
      project.json
      anchors/<source-facing>/raw/<import-uuid>.<png|jpg>
      anchors/<source-facing>/snap/<review-uuid>/native.png
      anchors/<source-facing>/snap/<review-uuid>/reference.png
      anchors/<source-facing>/cleanup/<review-uuid>/cleaned.png
      anchors/<source-facing>/cleanup/<review-uuid>/normalized.png
      animations/<animation-uuid>/frames_normalized/<frame-uuid>.png
      animations/<animation-uuid>/board/raw/<board-uuid>.<png|jpg>
      animations/<animation-uuid>/board/reviews/<review-uuid>/candidate-*.png
      animations/<animation-uuid>/frames_raw/<frame-uuid>.png
      animations/<animation-uuid>/batch_snap/<review-uuid>/<index>.png
      animations/<animation-uuid>/batch_cleanup/<review-uuid>/<index>-{cleaned,normalized}.png
      exports/references/<source-facing>/<cleanup-review-uuid>/normalized.png
      exports/references/<source-facing>/<cleanup-review-uuid>/cutout.png
      exports/references/<source-facing>/<cleanup-review-uuid>/snapped-native.png
      exports/references/<source-facing>/<cleanup-review-uuid>/nearest-neighbour-reference.png
      exports/references/<source-facing>/<cleanup-review-uuid>/manifest.json
      exports/animations/<animation-uuid>/<export-uuid>/spritesheet.png
      exports/animations/<animation-uuid>/<export-uuid>/preview.gif
      exports/animations/<animation-uuid>/<export-uuid>/manifest.json
  deleted-projects/
    <name-slug>-<short-id>/  # recoverable, complete character projects
```

The library's **Delete** action moves a complete character project into `deleted-projects`; it does not erase its assets. **Undo** is available immediately, and **Recently deleted → Restore** remains available later. Restoration moves the project back into `projects` and reopens it. A conflicting project folder is never overwritten.

`project.json` uses schema version `1` with additive, backward-compatible fields. It records identity, preset, runtime settings, immutable import/snap/cleanup records, animation frames and reviews, active IDs, SHA-256 hashes, relative paths, and stage states. Reimporting clears dependent active IDs, while prior files and reviews remain intact. Image bytes are validated and copied by Rust. Project-relative paths are validated, and metadata is written through a temporary file before replacement.

New projects start with north, south, east, and west sources, 256 × 256 cells, and a (128, 255) default pivot. Runtime cell and pivot settings can be adjusted for the character. Existing projects retain their saved settings. Exporting does not approve art for production or write into another project.

## Workflow boundary

Stage IDs on each anchor import are `raw`, `snap`, `cleanup`, `normalize`, `align`, `preview`, and `export`, with `waiting`, `processing`, `review`, `complete`, or `failed` states. `raw` is complete after import. Snap, cleanup, and normalization require review and **Apply** before becoming the next active input. Cleanup and normalization run together because the cutout and fixed-cell placement are one review decision; both stage states change together. A failed or interrupted run is marked failed while prior applied output remains preserved.

Cleanup is deterministic: auto chroma samples the source corners, or a six-digit RGB color can be entered. Tolerance is a hard RGB-distance threshold; OpenCV removes components smaller than the chosen pixel count. Pillow saves exact-color, binary-alpha PNGs. Transparent surroundings are trimmed to the foreground bounding box; no foreground pixels are cropped. The silhouette is centered horizontally on the project pivot and bottom-aligned at that pivot using whole pixels. No scaling or smoothing occurs; a silhouette exceeding the cell fails with a clear error. Any configured neutral-height difference is shown for review, not silently corrected. All output remains a `DRAFT` study in Sprite Studio's workspace.

The optional green-fringe cleanup also keys darker green pixels within two native pixels of a green chroma background; a narrow RGB tolerance alone may miss those snapped edge colors. It does not globally remove green from the character. Turn the option off in Project workflow settings when intentional green details touch the silhouette. Changing cleanup code or settings does not rewrite immutable reviews or exports: run cleanup again, inspect the new candidate, Apply it, then export a new version.

Runtime geometry can be **Flexible** or **Locked**, with an optional neutral-height target and tolerance. Locked mode freezes the cell and pivot. If cleanup finds an intact silhouette that cannot be placed at that pivot, **Auto-fit to locked cell** retries the local snapper at progressively coarser grids, measures the cleaned foreground before normalization, and ranks fitting candidates by neutral-height match, grid change, retained foreground detail, and speckle loss. It shows up to three review choices and never Applies one automatically. Each retry is an immutable snap review; the original import and previously applied snap remain untouched. If none fit, use another source or manually choose a coarser pixel size. Flexible projects can instead accept a suggested larger cell. Legacy KangiFight projects retain their fixed geometry contract.

After applying cleanup for a single-image reference, **Export reference** copies the applied PNGs and a draft manifest to the selected project's `exports/references/` folder. The source images remain untouched. The sidebar's **Workspace** action opens the canonical selected workspace; **Change workspace** is a separate action. **Project exports** opens the selected project's `exports/` folder. Each completed reference or animation export also has an **Open export folder** action targeting that exact result. Older animation exports in `animations/<animation-uuid>/exports/` remain readable and openable.

## Animation and export

Create an animation for a project source facing, then import ordered PNG frames that are already normalized to the project's fixed cell and binary alpha. Imported bytes are preserved. The alignment editor previews frame offsets, a prior-frame onion skin, pixel grid, pivot/baseline guides, and a real-time player; arrows move one pixel and Shift+arrows move five. Review and **Apply** write immutable offset and timing records, not changes to the source frames. Reimporting frames invalidates active alignment, preview, and export IDs.

After applying alignment and timed preview, export locally with Pillow. New projects can choose columns, padding, and spacing while every cell keeps its configured runtime size. Every run writes a new folder; unchanged inputs/options produce identical PNG, GIF, and manifest hashes. GIFs may merge identical adjacent poses while preserving total playback duration. The manifest records every source frame in order, its offset and rectangle, hashes, and `DRAFT` status. Generated files stay in Sprite Studio's workspace until explicitly reviewed for use elsewhere.

Sprite Studio works entirely inside the selected workspace and never writes into an external game project automatically.

## Pose boards and batch frames

On a character's main screen, choose **Pose board → animation** if one image contains several poses. Create or select an animation, choose its facing, then import the PNG/JPEG pose board. Single-image references are a separate, optional path for neutral character images; they are not a prerequisite for pose-board work. Drag-and-drop follows the selected path.

The app shows contextual size and setting guidance at each stage. Suggested generation canvases are **not** runtime requirements: a neutral single-image anchor can start at 1024 × 1024; a high-detail pose board can start at 2048 × 1536 (implied 4 × 3 areas of 512 × 512), or 1536 × 1152 (384 × 384 areas). Use flat, non-conflicting chroma. Extraction finds foreground components rather than cutting those implied cells. Local extraction supports boards up to 4096 pixels per axis.

Walk-cycle video curation is still external: select one complete cycle and import fixed-cell PNG frames. Sprite Studio does not currently import video.

| Step | Editable starting values | Required output/review |
| --- | --- | --- |
| Pose recovery | Auto chroma; tolerance 36; minimum area 64; merge gap 4 | Inspect complete poses and order before Apply; supported tolerance is 0–80 and merge gap 0–32. |
| Per-frame snap | 16 colors; Auto pixel size; approved palette entries when available | Snap each crop independently; inspect native clusters, then Apply. |
| Chroma cleanup | Board chroma; tolerance 24; speckle area 2 | Binary alpha, no missing contour pixels; fixed project cell and pivot, no scaling. |
| Alignment and playback | Whole-pixel offsets; 8 FPS study preview | Review loop seam and drift at native and nearest-neighbour sizes. |
| Draft export | Up to 5 columns, zero padding/spacing as editable starting values. | Confirm sheet, GIF, and manifest together; exports remain `DRAFT` until reviewed for the target game. |

For single-image references, snap starts at 16 colors and Auto pixel size, then cleanup starts with Auto chroma, tolerance 18, and speckle area 2. These are processing starting values, not guarantees or palette approval. The small recovered native image and its nearest-neighbour upscale serve different purposes; only the upscale is a downstream generation reference.

Within an animation, detect chroma-separated components, reject/reorder candidates or add an exact-coordinate manual crop, then **Apply** to preserve raw crops. **Snap all** creates a separate review for every raw frame. **Apply snapped frames** makes that review the cleanup input. **Clean all** previews binary-alpha cutouts in fixed cells; **Apply normalized frames** appends immutable frames to the animation for alignment and export. A failed batch keeps the previously applied reviews and raw sources.

Each snapped-frame thumbnail opens an individual review dialog with exact native dimensions, 1×/2×/4×/8× nearest-neighbour zoom, and previous/next navigation. The single-image anchor snap review has the same large-view option. Zoom affects only display, not saved pixels or Apply state.

If a batch pose cannot fit its fixed cell and pivot, **Auto-fit to locked cell** can create and rank new batch snap reviews without replacing the applied batch. Inspect individual candidate frames and **Apply selected snap batch** before running cleanup again. This applies to automatic snap/cleanup mode; manual upscale handoff inputs are not silently resnapped. Snapping never rescales a normalized frame. When tight manual crops have no chroma at their corners, use the detected pose-board background color instead of `auto` for cleanup.
