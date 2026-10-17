# Static Environment Layer workflow

> This document describes the original Outback contract, retained as the built-in template. The dedicated **Environment Assets** menu now supports custom packages, palettes, props, tiles, overlays and package exports. See [Environment Assets workspace](environment-assets-workspace.md) for the current workflow, schema-2 storage, compatibility and verification notes.

## Scope and creative authority

Preferred environment workflow: **AI-generated original → Sprite Studio import → explicit pixel cleanup / palette review → approved native PNG + manifest → later Codex/Godot integration**. Aseprite is optional. Do not independently redraw approved AI artwork as geometric scenery. This tool does not generate or split a flattened scene into plausible layers.

Static layers do not enter character frame extraction, normalization, alignment, preview timing, or combat export. Use an independent environment project, or select an existing workspace project from the environment menu's own picker. Character selection and presets have no effect on environment dimensions. The original package remains `biome.sunscorched.v1` / `arena.outback`.

## Import, process, approve, export

1. Open **Environment Assets** in the sidebar. Choose its owning project and Outback package, then open **Asset editor**, select one of the six named layers and **Import layer PNG**. Character drag/drop is deliberately disabled on this route to prevent misassignment.
2. Inspect original dimensions, visible RGB color count, alpha counts, conservative pitch detection, source hash, and actual pixel-validation report. Imported PNG bytes are copied unchanged; PNG only, at most 32 MiB and 4096 pixels per axis.
3. Adjust the explicit processing controls and **Save settings for this layer only**. This makes only this layer's current derived revision stale. Unsaved controls do not change a preview or export.
4. **Process saved settings → review** creates a new immutable PNG revision from the preserved original. It never overwrites the source, a manual import, or an older revision.
5. Compare original/processed at **1×** and **4× nearest**. Review palette remapping carefully: nearest RGB can collapse subtle distinctions and is never automatic art approval. Use the visibility toggles and composite to inspect composition. Supply reviewer name and confirm the visual checklist, then **Approve processed revision**. Technical errors block approval.
6. **Export approved PNG + manifest** rechecks actual source/processed hashes and output pixels. Copy the resulting PNG and manifest for later engine integration. The UI displays the exact export directory.
7. Close/reopen the project, return to Static environment, and verify the selected processed revision, settings, approval, original, and export history. Layer selection/visibility/zoom are session preview controls, not production data.

## Processing contract

| Operation | Behavior |
| --- | --- |
| Canvas normalization | No automatic resize, trim, crop, recenter, pivot, offset, or bounding-box normalization. Explicit resize maps the entire input to 480×270 using nearest neighbor; aspect distortion is possible and must be reviewed. |
| Pixel pitch | Default 1 preserves pixels. Manual 2–16 samples origin-aligned blocks using their upper-left pixel, without blur. Detection reports only exact uniform blocks; unknown pitch is not proof of quality. |
| Background removal | Optional explicit `#RRGGBB` key, squared RGB-distance tolerance. Disabled for opaque sky. Does not run the character green-fringe or component-trimming routines. |
| Alpha cleanup | Explicit threshold at 128 only; otherwise alpha is preserved and partial alpha fails validation. |
| Palette remap | Explicit deterministic nearest squared RGB distance; equal-distance ties use documented palette order. No dithering, new colors, or inferred outline colors. |
| Pipeline order | Whole-canvas nearest resize if requested → origin-aligned manual pitch → optional color key → optional alpha threshold → optional palette remap. |

The ten approved colors are `#F3B56B #D9704C #A44950 #5A303B #814032 #A95438 #D37A45 #966644 #C68B52 #47766A`. Only visible pixels participate in RGB palette validation; transparent RGB is preserved unless explicitly keyed. No additional outline anchors have been authorized in this tool. Color reports list up to 128 unexpected values and the full unexpected-color count.

## Six-layer contract

All images have origin `(0,0)`, canvas `480×270`, and no timing or character pivot.

| Order | Asset ID | Alpha rule |
| --- | --- | --- |
| 0 | `sunscorched_sky_void` | Every pixel opaque. |
| 1 | `sunscorched_far_silhouette` | Binary alpha. |
| 2 | `sunscorched_midground_landmark` | Binary alpha. |
| 3 | `sunscorched_fight_plane_back` | Binary alpha; no surface-row rule. |
| 4 | `sunscorched_fight_plane_surface` | Every pixel Y=0–223 transparent, every pixel Y=224–269 opaque. |
| 5 | `sunscorched_restrained_foreground` | Binary alpha; may be entirely transparent. |

Composite order follows the saved package layer order, not checkbox-selection order. Reordering retains stable layer IDs and validation rules. Missing/stale layers are explicitly excluded in the UI and counted. Current layers with wrong dimensions cannot composite. Draft layers may composite for review but cannot export as approved assets. Optional Y=224 and X=156/324 guides are DOM overlays, never baked into PNGs. Composite exports are separately marked `review_only`, record visible layer revisions, and contain no guides or parallax.

## Data, revisions, and recovery

```text
projects/<project>/
  project.json                         # existing character metadata; never rewritten here
  environments/biome.sunscorched.v1/
    index.json                         # schema 2; existing schema 1 loads read-only until saved
    index.v1-backup-<uuid>.json          # exact old metadata retained on first migrated save
    index.previous.json                # previous committed metadata
    index.pending.json                 # durable candidate during commit
    history/<uuid>.json                # retained metadata generations / interrupted copies
    layers/<asset-id>/<source-id>/original.png
    layers/<asset-id>/revisions/<revision-id>/processed.png
    exports/<asset-id>/<export-revision>-<uuid>/<asset-id>.png
    exports/<asset-id>/<export-revision>-<uuid>/manifest.json
    review-composites/<uuid>/composite.review-only.png
    review-composites/<uuid>/manifest.review-only.json
```

The static sidecar adds no fields to `project.json`, so no character-schema migration is necessary. Existing per-animation cleanup and evidence-checked recovery are unchanged. All writes share the existing project operation mutex. Original imports, revisions, exports, and metadata history are append-only; selected IDs may change, pixels are not overwritten. Source replacement stales only that layer. **Recover an earlier revision** explicitly restores that revision's source, parameters, and approval; newer revisions remain available.

Commit sequence: write/flush a complete history snapshot → retain old pending/backup generations → write/flush and deserialize/validate pending metadata → rotate primary to previous → rename pending to primary. Reopen checks primary → previous → pending → newest structurally valid history snapshot, reports recovery, and preserves damaged files for diagnosis. Source/output hashes are rechecked before approval/export. An unknown newer primary schema blocks rather than silently downgrading. No workspace is wiped. Schema-1 metadata loads without writes; its first edit retains an exact v1 backup before saving schema 2.

### Export manifest

Current manifests use schema 2 and the configured asset type, package ID, canvas and palette profile. They retain source/processed/export hashes, layer order, origin, explicit processing parameters, actual validation, revision lineage and reviewer data. Legacy schema-1 exports remain unchanged. For the Outback template the full-canvas assets remain 480×270 at origin [0,0]. See the current workspace document for independent-asset and full-package exports.

Environment manifests intentionally contain no fighter pivot, animation FPS, GIF duration, or tick holds. Repeated exports preserve identical PNG bytes/hashes but have new lineage/revision metadata and directories.

## Validation and synthetic demonstration

From the repository root:

```powershell
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
& '.venv/Scripts/python.exe' -m unittest discover -s src-tauri/python -p 'test_*.py' -v
npm run tauri build -- --debug --no-bundle
```

Optional persistent synthetic workspace (the target **must not already exist**):

```powershell
$env:SPRITE_STUDIO_STATIC_DEMO_DIR = 'E:/dev/sprite-studio-static-demo-new'
cargo test --manifest-path src-tauri/Cargo.toml --lib environment::tests::write_demo -- --ignored --nocapture
```

The fixture creates its own ordinary project, six deliberately simple test PNGs, approved synthetic revisions, exports, and a review composite; it checks export/reopen equality. These are **test patterns, not Outback artwork**. Open its workspace using Change workspace to inspect the panel. Return to the original workspace afterward. Never point this command at a character workspace.

## Manual limitations

Pixel validation cannot certify artistic detail, detect every baked checkerboard, distinguish intentional palette patterns from antialiasing, infer a useful pitch for inconsistent AI pixels, or reconstruct separate layers from flattened art. Human native/composite review remains mandatory. Palette remapping may require external/manual corrections and reimport as a new source. The Outback template retains the six identities and ten-color authority above; custom packages support independent canvases and palettes. No image generation or Godot modification is performed.
