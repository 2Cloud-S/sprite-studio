# Environment Assets workspace

Sprite Studio now has two top-level destinations: **Character Animations** and **Environment Assets**. Environment work uses its own project/package picker and independent sidecars. It never calls character recovery, cleanup, normalization, alignment or combat export. No game-engine repository is required or modified.

## User walkthrough

1. Open **Environment Assets** in the left menu. Choose an existing workspace project, or **New environment project** to create environment-only storage. This does not create/select a character. The last environment project/package selection is remembered per workspace.
2. **Create package**: choose Custom or the built-in Sunscorched Outback template. Custom supports a name, 1–4096 native pixels per axis, 0–128 initial layer slots and a palette profile. No artwork is generated.
3. **Package settings**: rename the display name, set optional biome/arena IDs, canvas, palette and preview guides. Add background layers, independent props, tiles or overlays. Remove assets from membership without deleting their imports/revisions; restore them here. Duplicate configuration creates an independent Custom package without copying artwork or approval.
4. **Asset library**: browse thumbnails, IDs, category, target dimensions, processing/review state, modification time and export state. Search by name/ID and filter by type/status. Missing/corrupt image previews report an error instead of silently retaining an old thumbnail.
5. **Asset editor**: choose the asset, **Import layer PNG** and inspect actual input dimensions, hash, colors and alpha. PNG originals are copied byte-for-byte to unique paths (up to 32 MiB, 4096 pixels per axis). Importing again preserves prior sources. Picker cancellation does nothing.
6. Save explicit processing settings for this asset, then **Process saved settings → review**. Inspect source/processed side by side at 1× native and enlarged nearest-neighbor zoom. Saving settings does not create a processed result. Every Process creates a new immutable revision from its selected original.
7. Review dimensions, alpha, colors, remapping and composition. Enter a reviewer and check the visual checklist before **Approve processed revision**. Technical success never grants artistic approval. Technical failures block approval and production export.
8. **Layer compositor**: reorder layers, toggle visibility, adjust review-only opacity, choose current/historical revisions and use native/enlarged previews. Compositor thumbnails show its chosen review revisions; library thumbnails remain tied to current assets. Props/overlays use explicit package placement metadata. Attach a master PNG for adjacent/opacity-overlay comparison. It is not split into layers. Guides, reference images and temporary opacity do not change production PNGs.
9. **Export & validation**: export a single approved revision or a selected approved package. Hashes and validation are checked again. Full packages include PNGs, individual manifests, layer order, source dependencies, exact dimensions, palette profile and validation. Every export has a unique directory and **Open folder** targets that recorded directory. Overview shows the latest dated asset/package/review exports in their actual cross-kind order. Legacy folders without dates remain available on the Export screen.
10. **Revision history**: inspect a historical image before Restore. It selects that original and processing settings without deleting newer work. If its canvas/palette/rules contract differs, first restore the appropriate configuration explicitly. **Inspect package snapshots** lists up to 100 validated metadata generations; reusing their canvas/palette/guide configuration preserves current sources and complete revision histories.

## Explicit transformations

| Operation | Behavior |
| --- | --- |
| Preserve | Default. Exact source canvas and origin; no crop, trim, recenter or resize. |
| Scale entire image | Explicit nearest-neighbor stretch to target dimensions. An aspect-ratio warning appears when proportions differ. |
| Fit aspect | Uniform nearest-neighbor scaling and transparent letterboxing. Offset is relative to centered placement; placement that would crop is rejected. |
| Fill aspect | Explicit uniform nearest-neighbor scale and crop. This is the only canvas option permitted to crop. Review before approval. |
| Resize canvas / Pad | No image scaling. Preserve top-left source coordinates plus whole-pixel offset, with transparent expansion. Placement that would crop is rejected. |
| Manual pixel pitch | 1 preserves pixels. 2–16 samples upper-left pixels of origin-aligned blocks; output dimensions remain unchanged. No smoothing or adaptive character-grid recovery is implied. |
| Background key | Explicit RGB-distance tolerance; disabled by an opaque validation rule. No character green-fringe/despeckle/component trimming. |
| Binary alpha | Explicit threshold at 128; otherwise alpha is preserved and checked against the asset rule. |
| Palette remap | Explicit deterministic nearest-RGB mapping. Ties use palette order; no dithering or invented colors. |

Full-canvas backgrounds always share the package canvas. Independent assets may set their own native dimensions and pivot metadata. Pivot/placement metadata never changes pixels. Tiles provide a 3×3 repeated preview and measured opposite-edge mismatches; even matching edges do not certify visual seamlessness.

The Outback template retains the six exact original IDs, 480×270 canvas, approved ten-color palette, fully opaque sky and Y=224 surface alpha rule. Validation follows **asset identity**, not current layer order. Other packages do not inherit those requirements. Duplicate the template configuration to Custom to change its contract.

Palettes support original/unrestricted colors, the ten-color Outback profile, and custom named profiles/slots. Import/export of profiles is via pasted/copied JSON, not native palette-file formats. Changing profiles only changes validation/settings; pixel conversion requires explicit remapping and a new reviewed processing revision. The editor reports visible color usage and unexpected colors.

Localized painting/mask editing remains external: edit a copy in a pixel editor, save a separate PNG and reimport. Previous sources, manual imports and revisions remain recoverable. No new painting application or second animation engine was introduced.

## Storage, migration and isolation

Existing projects keep their original `project.json` untouched. Independent environment projects live at:

```text
<workspace>/environment-projects/<owner-uuid>/owner.json
<workspace>/environment-projects/<owner-uuid>/environments/<package-id>/...
```

Environment packages associated with existing projects stay at `projects/<project>/environments/<package-id>/`. The legacy `biome.sunscorched.v1` location remains valid. New packages have unique `package-<uuid>` IDs. Layer IDs are stable even after display-name edits or reordering.

```text
environments/<package-id>/
  index.json                          # schema 2
  index.v1-backup-<uuid>.json          # exact metadata before first schema-1 edit
  index.previous.json
  index.pending.json
  history/<uuid>.json                 # retained metadata generations / interrupted copies
  layers/<asset-id>/<source-id>/original.png
  layers/<asset-id>/revisions/<revision-id>/processed.png
  references/<uuid>/original.png
  exports/<asset-id>/<revision>-<uuid>/...
  package-exports/<uuid>/assets/*.png
  package-exports/<uuid>/assets/*.manifest.json
  package-exports/<uuid>/package.manifest.json
  review-composites/<uuid>/composite.review-only.png
  review-composites/<uuid>/manifest.review-only.json
```

Schema 1 loads with an in-memory compatibility adapter and no write. First edit retains the original metadata backup, then uses the existing durable history → pending validation → previous rotation → primary commit sequence. Reopening tries primary, previous, pending and validated history. An unknown newer schema blocks; it is never silently downgraded. Damaged/interrupted files remain for diagnosis. No destructive migration or workspace reset occurs.

Per-asset settings stale only that asset's current derived selection. Package contract edits stale only assets whose effective canvas/palette/validation contract changes; independently sized props are unaffected by a package-canvas-only change. Rename, order and review controls do not erase approvals or files. Restoring older settings does not automatically restore current approval. Other packages, projects and character animation states are outside this module's mutation boundary. A shared operation mutex protects metadata writes; canonical-path checks and hashes protect imports/previews/exports.

Review composite exports record selected revision IDs and review opacity and are explicitly `review_only`. Production export retains the processed PNG's intended alpha, excludes guides/reference images and requires approval of the exact selected revision. Package export preflights every selected asset before writing its manifest.

## Implementation and reused functionality

The existing environment module's byte-preserving PNG importer, deterministic processor, validation, approval, immutable outputs, hash checks and recovery-safe sidecar commits were generalized in place. Existing character processing and recovery algorithms were not rewritten. Frontend controls keep Sprite Studio's fonts, tokens, button conventions and nearest-neighbor previews.

The pixel-sprite workflow skill informed the explicit transformation/review boundaries and immutable provenance. A targeted React best-practices review improved concurrent preview reads, cancellation and current-versus-historical preview isolation. The Computer Use skill was used for the real desktop launch/capture attempt; its capture failure is reported separately, not treated as visual approval.

Files changed/added for this work (excluding pre-existing uncommitted character changes):

- `src-tauri/src/environment.rs`: data-driven schema/contracts, package/asset commands, independent owners, palettes, transforms, history, compositor and exports; regression tests.
- `src-tauri/src/studio.rs`: read-only workspace lookup and reusable folder opener visibility; formatting of an existing test helper.
- `src-tauri/src/lib.rs`: registration of the environment-library command.
- `src/App.tsx`, `src/styles.css`: dedicated menu and environment route, isolated character navigation and workbench styling.
- `src/EnvironmentAssets.tsx`: environment-specific projects and package creation/opening.
- `src/EnvironmentWorkspace.tsx`: overview, library, editor, compositor, export and history views.
- `src/EnvironmentPackageSettings.tsx`: configuration, asset membership, profiles and template/custom controls.
- `src/environmentTypes.ts`, `src/services/environment.ts`, `src/domain/environment.ts`: typed commands, contracts, state/status derivation.
- `tests/environment.test.tsx`, `vitest.config.ts`, `package.json`, `package-lock.json`: frontend interaction tests and test tooling. The vulnerable transitive `source-map-js` was updated; npm audit reports zero vulnerabilities.
- `README.md`, `docs/static-environment-workflow.md`, this document and `docs/environment-verification.md`: current workflow, compatibility, exact verification and manual-review steps.

## Limitations

PNG import only; conservative exact-block pitch detection may return unknown for AI mixels. There is no image generation, landscape separation, automatic artistic approval, animated-environment engine, combat VFX, mask painter, native palette-file importer, or game-engine integration. Review guides currently provide ground/HUD horizontal lines and arbitrary vertical markers rather than editable region polygons. Full-canvas rules and palettes remain explicitly configured. Desktop screenshots/visual sign-off were blocked by the native capture service; see the verification report rather than assuming a build is visual approval.
