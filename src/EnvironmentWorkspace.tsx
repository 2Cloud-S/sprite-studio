import { useEffect, useRef, useState } from "react";
import {
  ArrowDown,
  ArrowUp,
  FolderOpen,
  ImagePlus,
  Layers3,
  Search,
} from "lucide-react";
import { environmentDesktop } from "./services/environment";
import type { AssetPreview } from "./types";
import type {
  EnvironmentLayer,
  EnvironmentParameters,
  EnvironmentReport,
  EnvironmentRequest,
  EnvironmentRevision,
  EnvironmentState,
  EnvironmentCatalog,
  EnvironmentRules,
  EnvironmentReply,
} from "./environmentTypes";
import {
  assetStatus,
  activeRevision,
  environmentCounts,
  matchesEnvironmentAsset,
} from "./domain/environment";
import { PackageSettings } from "./EnvironmentPackageSettings";

function Validation({ report }: { report: EnvironmentReport }) {
  return (
    <div className="environment-validation">
      <strong>
        {report.valid
          ? "Technical gates pass · visual review still required"
          : "DRAFT · technical gates fail"}
      </strong>
      <p>
        {report.width} × {report.height} · origin (0,0) · {report.colorCount}{" "}
        visible RGB colors · pitch{" "}
        {report.detectedPixelPitch ?? "not reliably detected"}
      </p>
      <p>
        Opaque {report.opaquePixels} · transparent {report.transparentPixels} ·
        partial alpha {report.partialAlphaPixels}
      </p>
      <p>
        Canvas {report.canvasPass ? "PASS" : "FAIL"} · palette{" "}
        {report.palettePass ? "PASS" : "FAIL"} · alpha{" "}
        {report.alphaPass ? "PASS" : "FAIL"} · ground{" "}
        {report.groundPass ? "PASS / N/A" : "FAIL"}
      </p>
      <p>
        Opposite edge mismatches: horizontal {report.edgeMismatches[0]} ·
        vertical {report.edgeMismatches[1]}. Matching edges alone do not certify
        artistic seamlessness.
      </p>
      <div className="environment-swatches" aria-label="Approved palette">
        {report.approvedColors.map((c) => (
          <span
            key={c}
            style={{ backgroundColor: c }}
            title={c}
            aria-label={c}
          />
        ))}
      </div>
      {report.errors.length ? (
        <ul>
          {report.errors.map((e) => (
            <li key={e}>{e}</li>
          ))}
        </ul>
      ) : null}
      {report.unexpectedColorCount ? (
        <details>
          <summary>
            {report.unexpectedColorCount} unexpected colors (first 128)
          </summary>
          <p className="environment-hash">
            {report.unexpectedColors.join(" ")}
          </p>
        </details>
      ) : null}
      <details>
        <summary>Visible color usage (up to 128 colors)</summary>
        <div className="environment-color-usage">
          {report.colorUsage.map(([c, n]) => (
            <span key={c}>
              <i style={{ background: c }} />
              {c} · {n} px
            </span>
          ))}
        </div>
      </details>
      <details>
        <summary>Review limitations</summary>
        {report.warnings.map((w) => (
          <p key={w}>{w}</p>
        ))}
      </details>
    </div>
  );
}

function ParametersEditor({
  layer,
  canvas,
  sourceSize,
  disabled,
  save,
}: {
  layer: EnvironmentLayer;
  canvas: [number, number];
  sourceSize?: [number, number];
  disabled: boolean;
  save: (parameters: EnvironmentParameters) => void;
}) {
  const [parameters, setParameters] = useState(layer.parameters);
  const dirty = JSON.stringify(parameters) !== JSON.stringify(layer.parameters);
  const patch = (value: Partial<EnvironmentParameters>) =>
    setParameters((p) => ({ ...p, ...value }));
  return (
    <fieldset disabled={disabled} className="environment-parameters">
      <legend>Explicit, non-destructive processing</legend>
      <label>
        Canvas operation → {canvas.join(" × ")}
        <select
          value={parameters.resizeToCanvas ? "stretch" : parameters.canvasMode}
          onChange={(e) =>
            patch({
              resizeToCanvas: false,
              canvasMode: e.target.value as EnvironmentParameters["canvasMode"],
            })
          }
        >
          <option value="preserve">Preserve source canvas and origin</option>
          <option value="stretch">
            Scale entire image · nearest (may stretch)
          </option>
          <option value="fit">
            Fit aspect · nearest + transparent padding
          </option>
          <option value="fill">Fill aspect · nearest + explicit crop</option>
          <option value="pad">
            Resize canvas only / transparent expansion
          </option>
        </select>
      </label>
      {sourceSize &&
      sourceSize[0] * canvas[1] !== sourceSize[1] * canvas[0] &&
      (parameters.canvasMode === "stretch" || parameters.resizeToCanvas) ? (
        <p className="environment-alert">
          Aspect ratios differ. Stretch will distort artwork; choose Fit to
          preserve proportions.
        </p>
      ) : null}
      {parameters.canvasMode !== "preserve" &&
      parameters.canvasMode !== "stretch" ? (
        <div className="environment-actions">
          <label>
            Position X{" "}
            <input
              type="number"
              value={parameters.offset[0]}
              onChange={(e) =>
                patch({
                  offset: [Number(e.target.value), parameters.offset[1]],
                })
              }
            />
          </label>
          <label>
            Position Y{" "}
            <input
              type="number"
              value={parameters.offset[1]}
              onChange={(e) =>
                patch({
                  offset: [parameters.offset[0], Number(e.target.value)],
                })
              }
            />
          </label>
          <small>
            Pad uses source origin + offset. Fit / Fill use centered placement +
            offset. Only Fill may crop.
          </small>
        </div>
      ) : null}
      <label>
        Origin-aligned manual pixel pitch{" "}
        <input
          type="number"
          min={1}
          max={16}
          value={parameters.pixelPitch}
          onChange={(e) => patch({ pixelPitch: Number(e.target.value) })}
        />
      </label>
      <small>
        Pitch 1 preserves pixels. Higher pitches sample the top-left of each
        block; review detail loss before approval.
      </small>
      <label>
        <input
          type="checkbox"
          disabled={layer.rules.alpha === "opaque"}
          checked={parameters.removeBackground !== null}
          onChange={(e) =>
            patch({ removeBackground: e.target.checked ? "#00FF00" : null })
          }
        />{" "}
        Remove a selected background color{" "}
        {layer.rules.alpha === "opaque" ? "(disabled by opaque rule)" : ""}
      </label>
      {parameters.removeBackground !== null ? (
        <div>
          <label>
            Key color{" "}
            <input
              type="color"
              value={parameters.removeBackground}
              onChange={(e) =>
                patch({ removeBackground: e.target.value.toUpperCase() })
              }
            />
          </label>
          <label>
            RGB distance tolerance{" "}
            <input
              type="number"
              min={0}
              max={255}
              value={parameters.backgroundTolerance}
              onChange={(e) =>
                patch({ backgroundTolerance: Number(e.target.value) })
              }
            />
          </label>
        </div>
      ) : null}
      <label>
        <input
          type="checkbox"
          checked={parameters.binaryAlpha}
          onChange={(e) => patch({ binaryAlpha: e.target.checked })}
        />{" "}
        Propose binary alpha threshold at 128 (review antialiased edges)
      </label>
      <label>
        <input
          type="checkbox"
          checked={parameters.remapPalette}
          onChange={(e) => patch({ remapPalette: e.target.checked })}
        />{" "}
        Propose nearest-RGB package palette remap (can collapse subtle colors)
      </label>
      <button
        className="button ghost"
        disabled={!dirty}
        onClick={() => save(parameters)}
      >
        Save settings for this layer only
      </button>
      <small>
        {dirty
          ? "Unsaved settings do not apply to previews or exports. Save, then process and review."
          : "Saved. Processing always reads the preserved original. Previous revisions and exports remain recoverable."}
      </small>
    </fieldset>
  );
}

function Approval({
  revision,
  disabled,
  approve,
}: {
  revision: EnvironmentRevision;
  disabled: boolean;
  approve: (reviewer: string) => void;
}) {
  const [reviewer, setReviewer] = useState("");
  const [checked, setChecked] = useState(false);
  return (
    <fieldset disabled={disabled} className="environment-parameters">
      <legend>Approve this exact revision</legend>
      <label>
        Reviewer{" "}
        <input
          value={reviewer}
          placeholder="Name / initials"
          onChange={(e) => setReviewer(e.target.value)}
        />
      </label>
      <label>
        <input
          type="checkbox"
          checked={checked}
          onChange={(e) => setChecked(e.target.checked)}
        />{" "}
        I reviewed 1× / 4× detail, remapping, transparent edges, composition,
        and absence of baked checkerboards or guides.
      </label>
      <button
        className="button primary"
        disabled={!checked || !reviewer.trim() || !revision.report.valid}
        onClick={() => approve(reviewer)}
      >
        Approve processed revision
      </button>
      {revision.approvedAt ? (
        <small>
          Approved by {revision.reviewer} · {revision.approvedAt}
        </small>
      ) : null}
    </fieldset>
  );
}

export function EnvironmentWorkspace({
  projectId,
  packageId,
  onChange,
}: {
  projectId: string;
  packageId: string;
  onChange: (catalog: EnvironmentCatalog, packageId: string) => void;
}) {
  const [environment, setEnvironment] = useState<EnvironmentState | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [tab, setTab] = useState("Overview");
  const [visible, setVisible] = useState<Record<string, boolean>>({});
  const [opacity, setOpacity] = useState<Record<string, number>>({});
  const [reviewRevisions, setReviewRevisions] = useState<
    Record<string, string>
  >({});
  const [scale, setScale] = useState(1);
  const [guides, setGuides] = useState(true);
  const [master, setMaster] = useState<AssetPreview | null>(null);
  const [masterOpacity, setMasterOpacity] = useState(0);
  const [thumbnails, setThumbnails] = useState<Record<string, AssetPreview>>(
    {},
  );
  const [reviewThumbnails, setReviewThumbnails] = useState<
    Record<string, AssetPreview>
  >({});
  const [search, setSearch] = useState("");
  const [filterType, setFilterType] = useState("");
  const [filterStatus, setFilterStatus] = useState("");
  const [historyId, setHistoryId] = useState("");
  const [metadataHistory, setMetadataHistory] =
    useState<EnvironmentReply["metadataHistory"]>(null);
  const [historyPreview, setHistoryPreview] = useState<AssetPreview | null>(
    null,
  );
  const [exportSelected, setExportSelected] = useState<Record<string, boolean>>(
    {},
  );
  const [original, setOriginal] = useState<AssetPreview | null>(null);
  const [originalReport, setOriginalReport] =
    useState<EnvironmentReport | null>(null);
  const [processed, setProcessed] = useState<AssetPreview | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const busyRef = useRef(false);
  const mounted = useRef(true);
  const layer =
    environment?.layers.find((l) => l.assetId === selectedId && !l.removed) ??
    environment?.layers.find((l) => !l.removed);
  const selected = layer?.order ?? 0;
  const source = layer?.sources.find((s) => s.id === layer.currentSourceId);
  const revision = layer?.revisions.find(
    (r) => r.id === layer.currentRevisionId,
  );
  const activeLayers = environment?.layers.filter((l) => !l.removed) ?? [];
  const compositeLayers = activeLayers.filter(
    (l) =>
      visible[l.assetId] !== false &&
      (reviewRevisions[l.assetId] || l.currentRevisionId),
  );
  const thumbnailKey = activeLayers
    .map(
      (l) => `${l.assetId}:${l.currentRevisionId || l.currentSourceId || ""}`,
    )
    .join("|");
  const reviewKey = activeLayers
    .map((l) => `${l.assetId}:${reviewRevisions[l.assetId] || ""}`)
    .join("|");
  const compositePreview = (l: EnvironmentLayer) =>
    reviewRevisions[l.assetId]
      ? reviewThumbnails[l.assetId]
      : thumbnails[l.assetId];
  const command = (request: EnvironmentRequest) =>
    environmentDesktop.command(projectId, request, packageId);

  useEffect(() => {
    mounted.current = true;
    environmentDesktop
      .command(projectId, { operation: "load" }, packageId)
      .then((reply) => {
        if (mounted.current) {
          setEnvironment(reply.environment);
          setNotice(reply.recoveryNotice);
        }
      })
      .catch((e) => {
        if (mounted.current) setError(String(e));
      });
    return () => {
      mounted.current = false;
    };
  }, [projectId, packageId]);

  useEffect(() => {
    let cancelled = false;
    setOriginal(null);
    setOriginalReport(null);
    setProcessed(null);
    const read = (isOriginal: boolean) =>
      environmentDesktop
        .command(
          projectId,
          { operation: "preview", order: selected, original: isOriginal },
          packageId,
        )
        .then((reply) => ({
          image:
            reply.result && typeof reply.result !== "string"
              ? reply.result
              : null,
          report: reply.validation || null,
        }));
    Promise.all([
      source ? read(true) : Promise.resolve(null),
      revision ? read(false) : Promise.resolve(null),
    ])
      .then(([a, b]) => {
        if (!cancelled) {
          setOriginal(a?.image || null);
          setOriginalReport(a?.report || null);
          setProcessed(b?.image || null);
        }
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [
    projectId,
    packageId,
    selected,
    source?.id,
    revision?.id,
    JSON.stringify(environment?.palette),
    JSON.stringify(layer?.rules),
    JSON.stringify(layer?.nativeSize),
    JSON.stringify(environment?.canvas),
  ]);

  useEffect(() => {
    let cancelled = false;
    setThumbnails({});
    Promise.allSettled(
      activeLayers
        .filter((l) => l.currentRevisionId || l.currentSourceId)
        .map(async (l) => {
          const revision = l.currentRevisionId;
          const reply = await environmentDesktop.command(
            projectId,
            revision
              ? { operation: "revision_preview", order: l.order, revision }
              : { operation: "preview", order: l.order, original: true },
            packageId,
          );
          return [
            l.assetId,
            typeof reply.result === "object" ? reply.result : null,
          ] as const;
        }),
    )
      .then((results) => {
        const entries = results.flatMap((r) =>
          r.status === "fulfilled" ? [r.value] : [],
        );
        if (!cancelled) {
          setThumbnails(
            Object.fromEntries(
              entries.filter(
                (entry): entry is readonly [string, AssetPreview] => !!entry[1],
              ),
            ),
          );
          const failures = results.filter((r) => r.status === "rejected");
          if (failures.length)
            setError(
              `${failures.length} asset preview(s) unavailable. ${failures.map((r) => (r.status === "rejected" ? String(r.reason) : "")).join("; ")}`,
            );
        }
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [projectId, packageId, thumbnailKey]);

  useEffect(() => {
    let cancelled = false;
    setReviewThumbnails({});
    Promise.allSettled(
      activeLayers
        .filter((l) => reviewRevisions[l.assetId])
        .map(async (l) => {
          const reply = await environmentDesktop.command(
            projectId,
            {
              operation: "revision_preview",
              order: l.order,
              revision: reviewRevisions[l.assetId],
            },
            packageId,
          );
          return [
            l.assetId,
            typeof reply.result === "object" ? reply.result : null,
          ] as const;
        }),
    ).then((results) => {
      if (cancelled) return;
      const entries = results.flatMap((r) =>
        r.status === "fulfilled" && r.value[1]
          ? [r.value as readonly [string, AssetPreview]]
          : [],
      );
      setReviewThumbnails(Object.fromEntries(entries));
      const failures = results.filter((r) => r.status === "rejected");
      if (failures.length)
        setError(
          `Historical compositor preview unavailable. ${failures.map((r) => (r.status === "rejected" ? String(r.reason) : "")).join("; ")}`,
        );
    });
    return () => {
      cancelled = true;
    };
  }, [projectId, packageId, reviewKey]);

  useEffect(() => {
    let cancelled = false;
    setMaster(null);
    if (environment?.masterReference)
      environmentDesktop
        .command(projectId, { operation: "reference_preview" }, packageId)
        .then((reply) => {
          if (!cancelled && reply.result && typeof reply.result !== "string")
            setMaster(reply.result);
        })
        .catch((e) => {
          if (!cancelled) setError(String(e));
        });
    return () => {
      cancelled = true;
    };
  }, [projectId, packageId, environment?.masterReference?.sha256]);

  useEffect(() => {
    let cancelled = false;
    setHistoryPreview(null);
    if (historyId)
      environmentDesktop
        .command(
          projectId,
          {
            operation: "revision_preview",
            order: selected,
            revision: historyId,
          },
          packageId,
        )
        .then((reply) => {
          if (!cancelled && reply.result && typeof reply.result !== "string")
            setHistoryPreview(reply.result);
        })
        .catch((e) => {
          if (!cancelled) setError(String(e));
        });
    return () => {
      cancelled = true;
    };
  }, [historyId, selected, projectId, packageId]);

  async function run(request: EnvironmentRequest) {
    if (busyRef.current) return;
    busyRef.current = true;
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const reply = await command(request);
      if (mounted.current) {
        if (reply.metadataHistory) setMetadataHistory(reply.metadataHistory);
        setEnvironment(reply.environment);
        setNotice(
          typeof reply.result === "string"
            ? `Saved: ${reply.result}`
            : reply.recoveryNotice ||
                "Saved locally. Previous sources and revisions are preserved.",
        );
        onChange(reply.catalog, reply.environment.packageId);
      }
    } catch (e) {
      if (mounted.current) setError(String(e));
    } finally {
      busyRef.current = false;
      if (mounted.current) setBusy(false);
    }
  }
  async function importPNG() {
    try {
      const path = await environmentDesktop.choosePNG();
      if (typeof path === "string" && mounted.current)
        await run({ operation: "import", order: selected, path });
    } catch (e) {
      if (mounted.current) setError(String(e));
    }
  }

  async function importReference() {
    try {
      const path = await environmentDesktop.choosePNG();
      if (typeof path === "string")
        await run({ operation: "attach_reference", path });
    } catch (e) {
      setError(String(e));
    }
  }
  const chooseLayer = (l: EnvironmentLayer, destination = "Asset editor") => {
    setSelectedId(l.assetId);
    setTab(destination);
    setHistoryId("");
  };
  const reorder = (l: EnvironmentLayer, delta: number) => {
    if (!environment) return;
    const ids = environment.layers.map((l) => l.assetId);
    const target = l.order + delta;
    if (target < 0 || target >= ids.length) return;
    [ids[l.order], ids[target]] = [ids[target], ids[l.order]];
    void run({ operation: "reorder", asset_ids: ids });
  };
  const counts = environment ? environmentCounts(environment) : null;
  const exportPaths = environment
    ? [
        ...environment.packageExports,
        ...environment.compositeExports,
        ...environment.layers.flatMap((l) => l.exports),
      ]
    : [];
  return (
    <section
      id="environment-workspace"
      className="environment-workspace"
      aria-busy={busy}
    >
      <header className="environment-package-heading">
        <div>
          <h2>{environment?.displayName ?? "Opening package…"}</h2>
          <p>
            {packageId} · {environment?.canvas.join(" × ")} native canvas ·
            shared origin (0,0)
          </p>
        </div>
        <span className="preset-badge">
          {environment?.templateId ? "Outback template" : "Custom package"}
        </span>
      </header>
      <nav className="environment-tabs" aria-label="Environment package views">
        {[
          "Overview",
          "Asset library",
          "Package settings",
          "Asset editor",
          "Layer compositor",
          "Export & validation",
          "Revision history",
        ].map((t) => (
          <button
            key={t}
            aria-current={tab === t ? "page" : undefined}
            onClick={() => setTab(t)}
          >
            {t}
          </button>
        ))}
      </nav>
      {error ? (
        <div className="environment-alert" role="alert">
          {error}
        </div>
      ) : null}
      {notice ? (
        <p className="environment-notice" role="status">
          {notice}
        </p>
      ) : null}
      {!environment ? (
        <p>Loading saved package records…</p>
      ) : (
        <>
          {tab === "Overview" && (
            <>
              <p className="environment-workflow-line">
                Create / open package → Import → Configure → Process → Compare →
                Approve → Export
              </p>
              <div className="environment-metrics">
                {counts &&
                  Object.entries(counts).map(([label, n]) => (
                    <div key={label}>
                      <strong>{n}</strong>
                      <span>{label}</span>
                    </div>
                  ))}
              </div>
              <div className="environment-actions">
                <button
                  className="button primary"
                  onClick={() => setTab("Asset library")}
                >
                  <Layers3 size={16} />
                  Open asset library
                </button>
                <button
                  className="button ghost"
                  onClick={() => setTab("Package settings")}
                >
                  Configure package / add asset
                </button>
              </div>
              <h3>Latest exports</h3>
              {environment.exportHistory
                .slice(-5)
                .reverse()
                .map(({ path, kind, exportedAt }) => (
                  <div key={path}>
                    <small>
                      {kind === "review_only"
                        ? "Review-only composite"
                        : kind === "package"
                          ? "Approved package"
                          : "Approved asset"}{" "}
                      · {new Date(exportedAt).toLocaleString()}
                    </small>
                    <ExportFolder
                      path={path}
                      disabled={busy}
                      open={() => void run({ operation: "open_export", path })}
                    />
                  </div>
                ))}
              {exportPaths.length > 0 && !environment.exportHistory.length && (
                <p>
                  Existing exports have no recorded timestamps. Open Export &
                  validation to inspect those preserved folders.
                </p>
              )}
              {!exportPaths.length && (
                <p>
                  No exports yet. Technical validation and explicit visual
                  approval are required.
                </p>
              )}
              <p>
                Original images, revisions and exports are append-only.
                Character projects and other packages are not changed by this
                editor.
              </p>
            </>
          )}
          {tab === "Package settings" && (
            <PackageSettings
              key={environment.packageId + ":" + environment.revision}
              environment={environment}
              disabled={busy}
              run={(request) => void run(request)}
            />
          )}
          {tab === "Asset library" && (
            <>
              <div className="environment-actions">
                <label>
                  <Search size={16} />
                  Search{" "}
                  <input
                    value={search}
                    onChange={(e) => setSearch(e.target.value)}
                    placeholder="Name or asset ID"
                  />
                </label>
                <label>
                  Type{" "}
                  <select
                    value={filterType}
                    onChange={(e) => setFilterType(e.target.value)}
                  >
                    <option value="">All types</option>
                    {["background_layer", "prop", "tile", "overlay"].map(
                      (t) => (
                        <option key={t}>{t}</option>
                      ),
                    )}
                  </select>
                </label>
                <label>
                  Status{" "}
                  <select
                    value={filterStatus}
                    onChange={(e) => setFilterStatus(e.target.value)}
                  >
                    <option value="">All states</option>
                    {["empty", "stale", "failed", "review", "approved"].map(
                      (t) => (
                        <option key={t}>{t}</option>
                      ),
                    )}
                  </select>
                </label>
                <button
                  className="button ghost"
                  onClick={() => setTab("Package settings")}
                >
                  Add asset
                </button>
              </div>
              <div className="environment-asset-grid">
                {activeLayers
                  .filter((l) =>
                    matchesEnvironmentAsset(
                      l,
                      search,
                      filterType,
                      filterStatus,
                    ),
                  )
                  .map((l) => (
                    <button
                      className="environment-asset-card"
                      key={l.assetId}
                      onClick={() => chooseLayer(l)}
                    >
                      <div className="environment-thumbnail">
                        {thumbnails[l.assetId] ? (
                          <img
                            src={thumbnails[l.assetId].dataUrl}
                            alt={l.name}
                          />
                        ) : (
                          <ImagePlus size={28} />
                        )}
                      </div>
                      <strong>{l.name}</strong>
                      <span>
                        {l.assetType} ·{" "}
                        {(l.nativeSize || environment.canvas).join(" × ")}
                      </span>
                      <span className={`environment-state ${assetStatus(l)}`}>
                        {assetStatus(l)} ·{" "}
                        {l.exportedRevisionIds.includes(
                          l.currentRevisionId || "",
                        )
                          ? "Current revision exported"
                          : l.exports.length
                            ? `${l.exports.length} historical asset exports`
                            : "Not exported"}
                      </span>
                      <small>{l.assetId}</small>
                      <small>
                        {environment.displayName} ·{" "}
                        {l.modifiedAt
                          ? new Date(l.modifiedAt).toLocaleString()
                          : "Not imported"}
                      </small>
                      <small>
                        {activeRevision(l)?.report.errors.join("; ") ||
                          (l.currentRevisionId
                            ? "Technical checks pass · visual approval separate"
                            : "Import or process to validate")}
                      </small>
                    </button>
                  ))}
              </div>
              {!activeLayers.length && (
                <p>No active assets. Add a slot in Package settings.</p>
              )}
            </>
          )}
          {tab === "Asset editor" &&
            (layer ? (
              <>
                <AssetPicker
                  layers={activeLayers}
                  layer={layer}
                  select={(l) => chooseLayer(l)}
                />
                <h3>
                  {layer.name}{" "}
                  <span className={`environment-state ${assetStatus(layer)}`}>
                    {assetStatus(layer)}
                  </span>
                </h3>
                <div className="environment-actions">
                  <button
                    className="button primary"
                    disabled={busy}
                    onClick={() => void importPNG()}
                  >
                    Import layer PNG
                  </button>
                  <button
                    className="button ghost"
                    disabled={busy || !source}
                    onClick={() =>
                      void run({ operation: "process", order: selected })
                    }
                  >
                    Process saved settings → review
                  </button>
                  <button
                    className="button ghost"
                    disabled={busy || !revision?.approvedAt}
                    onClick={() =>
                      void run({ operation: "export", order: selected })
                    }
                  >
                    Export approved PNG + manifest
                  </button>
                </div>
                <p>
                  {layer.assetType === "background_layer"
                    ? "Full-canvas layer: no automatic trim or recenter. Preserve origin unless you explicitly transform."
                    : "Independent asset. Its image dimensions and pivot metadata are separate from package coordinates."}{" "}
                  Alpha: {layer.rules.alpha}.
                  {layer.rules.groundY !== null
                    ? ` Ground/contact rule Y=${layer.rules.groundY}.`
                    : ""}
                </p>
                {source ? (
                  <p className="environment-hash">
                    Original: {source.originalName} · SHA-256 {source.sha256}
                  </p>
                ) : null}
                <ParametersEditor
                  key={`${layer.assetId}:${JSON.stringify(layer.parameters)}`}
                  layer={layer}
                  canvas={layer.nativeSize || environment.canvas}
                  sourceSize={
                    source
                      ? [source.report.width, source.report.height]
                      : undefined
                  }
                  disabled={busy}
                  save={(parameters) =>
                    void run({
                      operation: "settings",
                      order: selected,
                      parameters,
                    })
                  }
                />
                <Zoom value={scale} change={setScale} />
                <div className="environment-comparison">
                  {[
                    {
                      label: "Original · unchanged",
                      image: original,
                      report: originalReport || source?.report,
                    },
                    {
                      label: "Processed · selected revision",
                      image: processed,
                      report: revision?.report,
                    },
                  ].map((item) => (
                    <div key={item.label}>
                      <h3>{item.label}</h3>
                      <div className="environment-scroll">
                        {item.image ? (
                          <img
                            alt={item.label}
                            src={item.image.dataUrl}
                            style={{
                              width: item.image.width * scale,
                              height: item.image.height * scale,
                            }}
                          />
                        ) : (
                          <p>
                            No current image. Previous outputs are retained in
                            revision history.
                          </p>
                        )}
                      </div>
                      {item.report ? <Validation report={item.report} /> : null}
                    </div>
                  ))}
                </div>
                {revision ? (
                  <Approval
                    key={revision.id}
                    revision={revision}
                    disabled={busy}
                    approve={(reviewer) =>
                      void run({
                        operation: "approve",
                        order: selected,
                        reviewer,
                      })
                    }
                  />
                ) : null}
                {layer.assetType === "tile" && processed && (
                  <>
                    <h3>3 × 3 edge-continuity preview</h3>
                    <p>
                      Inspect joins visually. Opposite-edge mismatch counts are
                      measured, not a seamlessness certificate.
                    </p>
                    <div className="environment-scroll">
                      <div
                        className="environment-tile-preview"
                        style={{
                          width: processed.width * scale * 3,
                          gridTemplateColumns: `repeat(3,${processed.width * scale}px)`,
                        }}
                      >
                        {Array.from({ length: 9 }, (_, i) => (
                          <img
                            key={i}
                            src={processed.dataUrl}
                            alt={`Tile repeat ${i + 1}`}
                            style={{
                              width: processed.width * scale,
                              height: processed.height * scale,
                            }}
                          />
                        ))}
                      </div>
                    </div>
                  </>
                )}
                <AssetMetadata
                  key={layer.assetId + ":" + layer.modifiedAt}
                  layer={layer}
                  template={!!environment.templateId}
                  disabled={busy}
                  run={(request) => void run(request)}
                />
                <details>
                  <summary>
                    Localized pixel correction · external edit and reimport
                  </summary>
                  <p>
                    Use the preserved original or a copied processed PNG in your
                    pixel editor. Save a separate PNG, then Import layer PNG as
                    a new source. Process and approve again. Older sources,
                    manual edits, revisions and exports remain in Revision
                    history. Sprite Studio does not paint pixels or overwrite
                    your originals.
                  </p>
                </details>
              </>
            ) : (
              <p>Add an asset in Package settings first.</p>
            ))}
          {tab === "Layer compositor" && (
            <>
              <div className="environment-actions">
                <Zoom value={scale} change={setScale} />
                <label>
                  <input
                    type="checkbox"
                    checked={guides}
                    onChange={(e) => setGuides(e.target.checked)}
                  />
                  Package guides · never exported
                </label>
                <button
                  className="button ghost"
                  disabled={busy}
                  onClick={() => void importReference()}
                >
                  Attach master reference PNG
                </button>
                <label>
                  Reference overlay{" "}
                  <input
                    type="range"
                    min={0}
                    max={1}
                    step={0.05}
                    value={masterOpacity}
                    onChange={(e) => setMasterOpacity(Number(e.target.value))}
                  />
                  {Math.round(masterOpacity * 100)}%
                </label>
              </div>
              <p>
                {compositeLayers.length}/{activeLayers.length} visible selected
                revisions. Missing/stale assets are excluded unless you
                explicitly select a historical revision. Opacity is review-only;
                production PNGs retain their original alpha.
              </p>
              <div className="environment-compositor">
                <div className="environment-layer-stack">
                  {activeLayers.map((l) => (
                    <div
                      key={l.assetId}
                      className={l.assetId === layer?.assetId ? "selected" : ""}
                    >
                      <div className="environment-actions">
                        <input
                          type="checkbox"
                          aria-label={`Show ${l.name}`}
                          checked={visible[l.assetId] !== false}
                          onChange={(e) =>
                            setVisible((v) => ({
                              ...v,
                              [l.assetId]: e.target.checked,
                            }))
                          }
                        />
                        <button
                          className="environment-text-button"
                          onClick={() => setSelectedId(l.assetId)}
                        >
                          {l.name}
                        </button>
                        <button
                          className="icon-button"
                          aria-label={`Move ${l.name} up`}
                          disabled={busy || l.order === 0}
                          onClick={() => reorder(l, -1)}
                        >
                          <ArrowUp size={15} />
                        </button>
                        <button
                          className="icon-button"
                          aria-label={`Move ${l.name} down`}
                          disabled={
                            busy || l.order === environment.layers.length - 1
                          }
                          onClick={() => reorder(l, 1)}
                        >
                          <ArrowDown size={15} />
                        </button>
                      </div>
                      <small>
                        {assetStatus(l)} · {l.assetType}
                      </small>
                      {compositePreview(l) && (
                        <div className="environment-thumbnail environment-stack-thumbnail">
                          <img
                            alt={`${l.name} · selected review revision`}
                            src={compositePreview(l).dataUrl}
                          />
                        </div>
                      )}
                      <label>
                        Review opacity{" "}
                        <input
                          type="range"
                          min={0}
                          max={1}
                          step={0.05}
                          value={opacity[l.assetId] ?? 1}
                          onChange={(e) =>
                            setOpacity((v) => ({
                              ...v,
                              [l.assetId]: Number(e.target.value),
                            }))
                          }
                        />
                      </label>
                      <select
                        aria-label={`Review revision for ${l.name}`}
                        value={reviewRevisions[l.assetId] || ""}
                        onChange={(e) =>
                          setReviewRevisions((v) => ({
                            ...v,
                            [l.assetId]: e.target.value,
                          }))
                        }
                      >
                        <option value="">
                          Current revision{" "}
                          {l.currentRevisionId?.slice(0, 8) || "· none / stale"}
                        </option>
                        {l.revisions.map((r, i) => (
                          <option key={r.id} value={r.id}>
                            History {i + 1} · {r.id.slice(0, 8)} ·{" "}
                            {r.approvedAt ? "approved" : "draft"}
                          </option>
                        ))}
                      </select>
                    </div>
                  ))}
                </div>
                <div>
                  <div className="environment-scroll">
                    <div
                      className="environment-canvas environment-live-composite"
                      style={{
                        width: environment.canvas[0] * scale,
                        height: environment.canvas[1] * scale,
                      }}
                    >
                      {compositeLayers.map((l) => {
                        const image = compositePreview(l);
                        return image ? (
                          <img
                            key={l.assetId}
                            alt={l.name}
                            src={image.dataUrl}
                            style={{
                              left: l.position[0] * scale,
                              top: l.position[1] * scale,
                              width: image.width * scale,
                              height: image.height * scale,
                              opacity: opacity[l.assetId] ?? 1,
                            }}
                          />
                        ) : null;
                      })}
                      {master && masterOpacity > 0 && (
                        <img
                          alt="Master reference overlay · review only"
                          src={master.dataUrl}
                          style={{
                            width: environment.canvas[0] * scale,
                            height: environment.canvas[1] * scale,
                            opacity: masterOpacity,
                          }}
                        />
                      )}
                      {guides && (
                        <div className="environment-guides" aria-hidden="true">
                          {environment.guides.groundY !== null && (
                            <i
                              style={{
                                top: environment.guides.groundY * scale,
                              }}
                            />
                          )}
                          {environment.guides.hudY !== null && (
                            <i
                              style={{ top: environment.guides.hudY * scale }}
                            />
                          )}
                          {environment.guides.markersX.map((x) => (
                            <b key={x} style={{ left: x * scale }} />
                          ))}
                        </div>
                      )}
                    </div>
                  </div>
                  {master && (
                    <details>
                      <summary>
                        Master composition ·{" "}
                        {environment.masterReference?.originalName} · unchanged{" "}
                        {master.width}×{master.height}
                      </summary>
                      <div className="environment-scroll">
                        <img
                          src={master.dataUrl}
                          alt="Original master composition reference"
                          style={{
                            width: master.width * scale,
                            height: master.height * scale,
                          }}
                        />
                      </div>
                      <p>
                        Overlay is fitted for comparison only. The reference is
                        not separated into layers or included in production
                        exports.
                      </p>
                    </details>
                  )}
                </div>
              </div>
              <button
                className="button ghost"
                disabled={busy || !compositeLayers.length}
                onClick={() =>
                  void run({
                    operation: "review_composite",
                    layers: compositeLayers.map((l) => ({
                      order: l.order,
                      revision:
                        reviewRevisions[l.assetId] || l.currentRevisionId!,
                      opacity: opacity[l.assetId] ?? 1,
                    })),
                  })
                }
              >
                Export review-only composite · no guides / reference
              </button>
            </>
          )}
          {tab === "Export & validation" && (
            <>
              <h3>Export exact approved revisions</h3>
              <p>
                Every export rechecks source and processed hashes, dimensions,
                palette and alpha. Review composites are not production assets.
                Missing/stale/unapproved assets cannot be selected.
              </p>
              {activeLayers.map((l) => (
                <div key={l.assetId} className="environment-export-row">
                  <label>
                    <input
                      type="checkbox"
                      disabled={assetStatus(l) !== "approved"}
                      checked={
                        exportSelected[l.assetId] !== false &&
                        assetStatus(l) === "approved"
                      }
                      onChange={(e) =>
                        setExportSelected((v) => ({
                          ...v,
                          [l.assetId]: e.target.checked,
                        }))
                      }
                    />
                    <strong>{l.name}</strong>
                  </label>
                  <span>
                    {assetStatus(l)} · revision{" "}
                    {l.currentRevisionId?.slice(0, 8) || "—"}
                  </span>
                  <button
                    className="button ghost"
                    disabled={busy || assetStatus(l) !== "approved"}
                    onClick={() =>
                      void run({ operation: "export", order: l.order })
                    }
                  >
                    Export asset
                  </button>
                  <button
                    className="environment-text-button"
                    onClick={() => chooseLayer(l)}
                  >
                    Review / resolve
                  </button>
                </div>
              ))}
              <button
                className="button primary"
                disabled={
                  busy ||
                  !activeLayers.some(
                    (l) =>
                      assetStatus(l) === "approved" &&
                      exportSelected[l.assetId] !== false,
                  )
                }
                onClick={() =>
                  void run({
                    operation: "export_package",
                    orders: activeLayers
                      .filter(
                        (l) =>
                          assetStatus(l) === "approved" &&
                          exportSelected[l.assetId] !== false,
                      )
                      .map((l) => l.order),
                  })
                }
              >
                Export selected approved package
              </button>
              <h3>Export folders · immutable history</h3>
              {exportPaths
                .slice()
                .reverse()
                .map((path) => (
                  <ExportFolder
                    key={path}
                    path={path}
                    disabled={busy}
                    open={() => void run({ operation: "open_export", path })}
                  />
                ))}
            </>
          )}
          {tab === "Revision history" && (
            <details>
              <summary>
                Package configuration history · retained metadata generations
              </summary>
              <p>
                Load up to 100 validated metadata snapshots. Reusing a
                configuration changes canvas/palette/guides only; sources and
                revision histories are never rolled back or removed.
              </p>
              <button
                className="button ghost"
                disabled={busy}
                onClick={() => void run({ operation: "package_history" })}
              >
                Inspect package snapshots
              </button>
              {metadataHistory?.map((h) => (
                <div key={h.snapshot} className="environment-export-row">
                  <span>
                    {h.name} · {h.canvas.join("×")} · {h.palette.name} ·{" "}
                    {h.assets} assets · {h.snapshot.slice(0, 8)}
                  </span>
                  <button
                    className="button ghost"
                    disabled={busy}
                    onClick={() =>
                      void run({
                        operation: "configure",
                        name: h.name,
                        canvas: h.canvas,
                        palette: h.palette,
                        guides: h.guides,
                        biome: h.biome,
                        arena: h.arena,
                      })
                    }
                  >
                    Use this package configuration
                  </button>
                </div>
              ))}
            </details>
          )}
          {tab === "Revision history" &&
            (layer ? (
              <>
                <AssetPicker
                  layers={activeLayers}
                  layer={layer}
                  select={(l) => chooseLayer(l, "Revision history")}
                />
                <h3>
                  {layer.name} · {layer.sources.length} original imports /{" "}
                  {layer.revisions.length} processing revisions
                </h3>
                <p>
                  Preview a revision first. Restore selects its original and
                  processing settings; it does not delete newer work. A
                  different package canvas/palette contract must be restored
                  explicitly.
                </p>
                <label>
                  Revision{" "}
                  <select
                    value={historyId}
                    onChange={(e) => setHistoryId(e.target.value)}
                  >
                    <option value="">Choose a revision to inspect</option>
                    {layer.revisions.map((r, i) => (
                      <option key={r.id} value={r.id}>
                        Revision {i + 1} ·{" "}
                        {r.approvedAt ? `approved by ${r.reviewer}` : "draft"} ·{" "}
                        {r.createdAt || r.id.slice(0, 8)}
                      </option>
                    ))}
                  </select>
                </label>
                {historyPreview && (
                  <div className="environment-scroll">
                    <img
                      alt="Historical revision · not active"
                      src={historyPreview.dataUrl}
                      style={{
                        width: historyPreview.width * scale,
                        height: historyPreview.height * scale,
                      }}
                    />
                  </div>
                )}
                <button
                  className="button ghost"
                  disabled={busy || !historyId}
                  onClick={() =>
                    void run({
                      operation: "restore",
                      order: selected,
                      revision: historyId,
                    })
                  }
                >
                  Restore inspected revision
                </button>
                <details>
                  <summary>Preserved source history</summary>
                  {layer.sources.map((src) => (
                    <p key={src.id} className="environment-hash">
                      {src.originalName} · {src.report.width}×
                      {src.report.height} · {src.path} · SHA-256 {src.sha256}
                    </p>
                  ))}
                </details>
              </>
            ) : (
              <p>No active asset selected.</p>
            ))}
        </>
      )}
    </section>
  );
}

function Zoom({
  value,
  change,
}: {
  value: number;
  change: (n: number) => void;
}) {
  return (
    <label>
      Zoom{" "}
      <select value={value} onChange={(e) => change(Number(e.target.value))}>
        {[0.25, 0.5, 1, 2, 4, 8].map((n) => (
          <option key={n} value={n}>
            {n}× {n === 1 ? "native" : "nearest"}
          </option>
        ))}
      </select>
    </label>
  );
}
function AssetPicker({
  layers,
  layer,
  select,
}: {
  layers: EnvironmentLayer[];
  layer: EnvironmentLayer;
  select: (l: EnvironmentLayer) => void;
}) {
  return (
    <label>
      Asset{" "}
      <select
        value={layer.assetId}
        onChange={(e) => {
          const l = layers.find((l) => l.assetId === e.target.value);
          if (l) select(l);
        }}
      >
        {layers.map((l) => (
          <option key={l.assetId} value={l.assetId}>
            {l.name} · {assetStatus(l)}
          </option>
        ))}
      </select>
    </label>
  );
}
function ExportFolder({
  path,
  disabled,
  open,
}: {
  path: string;
  disabled: boolean;
  open: () => void;
}) {
  return (
    <div className="environment-export-folder">
      <code>{path}</code>
      <button className="button ghost" disabled={disabled} onClick={open}>
        <FolderOpen size={15} />
        Open folder
      </button>
    </div>
  );
}
function AssetMetadata({
  layer,
  template,
  disabled,
  run,
}: {
  layer: EnvironmentLayer;
  template: boolean;
  disabled: boolean;
  run: (r: EnvironmentRequest) => void;
}) {
  const [name, setName] = useState(layer.name);
  const [rules, setRules] = useState<EnvironmentRules>(layer.rules);
  const [size, setSize] = useState(layer.nativeSize);
  const [pivot, setPivot] = useState(layer.pivot);
  const [position, setPosition] = useState(layer.position);
  const locked = template && layer.assetId.startsWith("sunscorched_");
  return (
    <details>
      <summary>Asset dimensions, validation and placement metadata</summary>
      <fieldset disabled={disabled} className="environment-parameters">
        <label>
          Display name{" "}
          <input value={name} onChange={(e) => setName(e.target.value)} />
        </label>
        <label>
          Alpha rule{" "}
          <select
            disabled={locked}
            value={rules.alpha}
            onChange={(e) =>
              setRules({
                ...rules,
                alpha: e.target.value as EnvironmentRules["alpha"],
              })
            }
          >
            <option value="binary">Binary 0/255</option>
            <option value="opaque">Opaque</option>
            <option value="any">Allow partial transparency</option>
          </select>
        </label>
        <label>
          Ground/contact row (blank = no rule)
          <input
            type="number"
            disabled={locked}
            min={0}
            value={rules.groundY ?? ""}
            onChange={(e) =>
              setRules({
                ...rules,
                groundY: e.target.value === "" ? null : Number(e.target.value),
              })
            }
          />
        </label>
        {layer.assetType !== "background_layer" && (
          <>
            <label>
              <input
                type="checkbox"
                checked={size !== null}
                onChange={(e) => setSize(e.target.checked ? [64, 64] : null)}
              />
              Independent native dimensions
            </label>
            {size && (
              <div className="environment-actions">
                {[0, 1].map((i) => (
                  <label key={i}>
                    {i === 0 ? "Width" : "Height"}
                    <input
                      type="number"
                      min={1}
                      max={4096}
                      value={size[i]}
                      onChange={(e) =>
                        setSize(
                          i === 0
                            ? [Number(e.target.value), size[1]]
                            : [size[0], Number(e.target.value)],
                        )
                      }
                    />
                  </label>
                ))}
              </div>
            )}
            <label>
              <input
                type="checkbox"
                checked={pivot !== null}
                onChange={(e) => setPivot(e.target.checked ? [0, 0] : null)}
              />
              Explicit pivot metadata · does not move image pixels
            </label>
            {pivot && (
              <div className="environment-actions">
                {[0, 1].map((i) => (
                  <label key={i}>
                    Pivot {i === 0 ? "X" : "Y"}
                    <input
                      type="number"
                      value={pivot[i]}
                      onChange={(e) =>
                        setPivot(
                          i === 0
                            ? [Number(e.target.value), pivot[1]]
                            : [pivot[0], Number(e.target.value)],
                        )
                      }
                    />
                  </label>
                ))}
              </div>
            )}
          </>
        )}
        {layer.assetType === "tile" && (
          <label>
            <input
              type="checkbox"
              checked={rules.seamlessRequired}
              onChange={(e) =>
                setRules({ ...rules, seamlessRequired: e.target.checked })
              }
            />
            Require identical opposite edges (still review joins visually)
          </label>
        )}
        <div className="environment-actions">
          {[0, 1].map((i) => (
            <label key={i}>
              Package placement {i === 0 ? "X" : "Y"}
              <input
                disabled={locked}
                type="number"
                value={position[i]}
                onChange={(e) =>
                  setPosition(
                    i === 0
                      ? [Number(e.target.value), position[1]]
                      : [position[0], Number(e.target.value)],
                  )
                }
              />
            </label>
          ))}
        </div>
        <button
          className="button ghost"
          disabled={!name.trim()}
          onClick={() =>
            run({
              operation: "asset_settings",
              order: layer.order,
              name,
              rules,
              native_size: size,
              pivot,
              position,
            })
          }
        >
          Save this asset only
        </button>
        {locked && (
          <small>
            The Outback built-in canvas, origin and alpha rules are fixed.
            Duplicate package configuration for a custom contract.
          </small>
        )}
      </fieldset>
    </details>
  );
}
