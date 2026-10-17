import { useCallback, useEffect, useRef, useState } from "react";
import { Pause, Play, Plus, Sparkles, StepForward } from "lucide-react";
import { vfxDesktop } from "./services/vfx";
import { traceVfx } from "./domain/vfxDiagnostics";
import {
  advanceVfxClock,
  defaultProcessing,
  frameAtTick,
  lightStudy,
  vfxValueKey,
} from "./domain/vfx";
import type {
  VfxChecks,
  VfxDraft,
  VfxLibrary,
  VfxProcessing,
  VfxProfile,
  VfxRegion,
  VfxReply,
  VfxReport,
  VfxRequest,
} from "./vfxTypes";
const button = "vfx-ui-btn vfx-ui-btn-sm";
const emptyChecks = (): VfxChecks => ({
  artwork: false,
  regionsAndAnchor: false,
  palette: false,
  timing: false,
  hitStopAndCompletion: false,
});
function NumberField({
  label,
  value,
  change,
  min = -4096,
  max = 4096,
}: {
  label: string;
  value: number;
  change: (v: number) => void;
  min?: number;
  max?: number;
}) {
  return (
    <label>
      {label}
      <input
        type="number"
        step="1"
        min={min}
        max={max}
        value={value}
        onChange={(e) => change(Number(e.target.value))}
      />
    </label>
  );
}

export function VfxWorkspace({ workspace }: { workspace: string }) {
  const key = `sprite-studio:vfx:${workspace}`;
  const [library, setLibrary] = useState<VfxLibrary>({
    projects: [],
    failures: [],
  });
  const [project, setProject] = useState(
    () => localStorage.getItem(key + ":project") || "",
  );
  const [effect, setEffect] = useState(
    () => localStorage.getItem(key + ":effect") || "",
  );
  const [projectName, setProjectName] = useState("");
  const [newEffect, setNewEffect] = useState(false);
  const [name, setName] = useState("New effect");
  const [effectId, setEffectId] = useState("effect.custom");
  const [packageName, setPackageName] = useState("");
  const [study, setStudy] = useState(false);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const [search, setSearch] = useState("");
  const libraryEpoch = useRef(0);
  const libraryAlive = useRef(false);
  const refresh = useCallback(async () => {
    const epoch = ++libraryEpoch.current;
    const l = await vfxDesktop.command<VfxLibrary>({ operation: "library" });
    if (libraryAlive.current && epoch === libraryEpoch.current) setLibrary(l);
  }, [workspace]);
  useEffect(() => {
    libraryAlive.current = true;
    refresh().catch((e) => {
      if (libraryAlive.current) setError(String(e));
    });
    return () => {
      libraryAlive.current = false;
      libraryEpoch.current++;
    };
  }, [refresh]);
  useEffect(() => {
    localStorage.setItem(key + ":project", project);
    localStorage.setItem(key + ":effect", effect);
  }, [key, project, effect]);
  async function create(kind: "project" | "effect") {
    if (busyRef.current) return;
    busyRef.current = true;
    setBusy(true);
    setError("");
    try {
      if (kind === "project") {
        const r = await vfxDesktop.command<{
          library: VfxLibrary;
          createdProjectId: string;
        }>({ operation: "create_project", name: projectName });
        libraryEpoch.current++;
        if (!libraryAlive.current) return;
        setLibrary(r.library);
        setProject(r.createdProjectId);
        setEffect("");
        setProjectName("");
      } else {
        const profile = lightStudy();
        if (!study) {
          profile.cell = [32, 32];
          profile.anchor = [16, 16];
          profile.sourceOrientation = "none";
          profile.mirrorPolicy = "none";
          profile.palette = {
            name: "Unrestricted · pending review",
            authorityId: null,
            packageReference: null,
            colors: [],
            enforce: false,
          };
        }
        const r = await vfxDesktop.command<VfxReply>(
          {
            operation: "create_effect",
            effect_id: effectId,
            name,
            package: packageName,
            profile,
          },
          project,
        );
        await refresh();
        setEffect(r.effect.id);
        setNewEffect(false);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  }
  const owner = library.projects.find((p) => p.id === project);
  return (
    <div className="vfx-workspace" id="vfx-workspace">
      <header className="page-heading">
        <div>
          <span className="eyebrow">VISUAL EFFECTS</span>
          <h1>Keep every spark.</h1>
          <p>
            Fixed cells, shared contact anchors and explicit tick holds.
            Originals stay untouched.
          </p>
        </div>
        <span className="vfx-ui-badge">LOCAL · REVIEW REQUIRED</span>
      </header>
      {(error || library.failures.length > 0) && (
        <div role="alert" className="vfx-notice">
          {error}
          {library.failures.map((f) => (
            <p key={f}>{f}</p>
          ))}
        </div>
      )}
      <div className="vfx-toolbar">
        <label>
          VFX project
          <select
            value={project}
            disabled={busy}
            onChange={(e) => {
              setProject(e.target.value);
              setEffect("");
            }}
          >
            <option value="">Choose project…</option>
            {library.projects.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        </label>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void create("project");
          }}
        >
          <label>
            New VFX project
            <input
              value={projectName}
              maxLength={160}
              onChange={(e) => setProjectName(e.target.value)}
              placeholder="Project or collection name"
            />
          </label>
          <button className={button} disabled={busy || !projectName.trim()}>
            Create VFX project
          </button>
        </form>
        <button
          className={button}
          disabled={!owner || busy}
          onClick={() => setNewEffect((v) => !v)}
        >
          <Plus size={16} />
          New effect
        </button>
      </div>
      {newEffect && (
        <form
          className="vfx-section vfx-toolbar"
          onSubmit={(e) => {
            e.preventDefault();
            void create("effect");
          }}
        >
          <label>
            Effect ID
            <input
              value={effectId}
              maxLength={160}
              onChange={(e) => setEffectId(e.target.value)}
            />
          </label>
          <label>
            Display name
            <input
              value={name}
              maxLength={160}
              onChange={(e) => setName(e.target.value)}
            />
          </label>
          <label>
            Package association
            <input
              value={packageName}
              maxLength={160}
              onChange={(e) => setPackageName(e.target.value)}
              placeholder="Optional package name/reference"
            />
          </label>
          <label>
            Starting profile
            <select
              value={study ? "light" : "custom"}
              onChange={(e) => {
                const light = e.target.value === "light";
                setStudy(light);
                if (light && effectId === "effect.custom") {
                  setEffectId("effect.contact.light");
                  if (name === "New effect") setName("Light Contact study");
                }
              }}
            >
              <option value="light">
                PROPOSED Light Contact · 24×24 / (12,12)
              </option>
              <option value="custom">Custom · editable 32×32</option>
            </select>
          </label>
          <p>
            All dimensions, RGB colors, holds and clock policies start
            unapproved.
          </p>
          <button
            className={button}
            disabled={busy || !name.trim() || !effectId.trim()}
          >
            Create draft effect
          </button>
        </form>
      )}
      <div className="vfx-hub">
        <aside className="vfx-library" aria-label="VFX asset library">
          <h2>Effect library</h2>
          <label>
            Find effects
            <input
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Name, ID or package"
            />
          </label>
          {owner?.effects
            .filter((e) =>
              `${e.name} ${e.effectId} ${e.package}`
                .toLowerCase()
                .includes(search.toLowerCase()),
            )
            .map((e) => (
              <button
                className={`vfx-effect-row ${effect === e.id ? "active" : ""}`}
                key={e.id}
                onClick={() => setEffect(e.id)}
              >
                <Sparkles size={16} />
                <span>
                  <strong>{e.name}</strong>
                  <small>
                    {e.effectId} · {e.status}
                  </small>
                </span>
              </button>
            ))}
          {!owner?.effects.length && (
            <p>Create an effect to import its frames.</p>
          )}
        </aside>
        {owner && effect ? (
          <EffectEditor
            key={`${project}:${effect}`}
            projectId={project}
            effectId={effect}
            changed={refresh}
          />
        ) : (
          <section className="vfx-section">
            <h2>A separate home for VFX</h2>
            <p>
              Select a project and effect. Character and environment workspaces
              retain their own state.
            </p>
          </section>
        )}
      </div>
    </div>
  );
}

function EffectEditor({
  projectId,
  effectId,
  changed,
}: {
  projectId: string;
  effectId: string;
  changed: () => Promise<void>;
}) {
  const [reply, setReply] = useState<VfxReply | null>(null);
  const [profile, setProfile] = useState<VfxProfile>(lightStudy);
  const [settings, setSettings] = useState<VfxProcessing[]>([]);
  const [holds, setHolds] = useState<number[]>([]);
  const [tab, setTab] = useState("Import & regions");
  const [operation, setOperation] = useState<string | null>(null);
  const busy = operation !== null;
  const busyRef = useRef(false);
  const [saving, setSaving] = useState(false);
  const pickerOpen = useRef(false);
  const savingRef = useRef(false);
  const [saveFailed, setSaveFailed] = useState(false);
  const alive = useRef(false);
  const epoch = useRef(0);
  const accepted = useRef<VfxReply | null>(null);
  const queue = useRef<Promise<unknown>>(Promise.resolve());
  const draftRequests = useRef(new Map<string, Promise<VfxReply>>());
  const lastPersistedDraft = useRef<string | null>(null);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [reviewer, setReviewer] = useState("");
  const [mode, setMode] = useState("frames");
  const [count, setCount] = useState(3);
  const [rectText, setRectText] = useState(
    "[[0,0,24,24],[24,0,24,24],[48,0,24,24]]",
  );
  const [regions, setRegions] = useState<VfxRegion[]>([]);
  const [checks, setChecks] = useState<VfxChecks>(emptyChecks);
  const [gutter, setGutter] = useState(0);
  const [paletteJSON, setPaletteJSON] = useState("");
  const [colorsText, setColorsText] = useState(
    lightStudy().palette.colors.join(" "),
  );
  const latestDraft = useRef<{
    draft: VfxDraft;
    needed: boolean;
    valid: boolean;
  } | null>(null);
  const effect = reply?.effect;
  const revision = effect?.revisions.find(
    (r) => r.id === effect.activeRevisionId,
  );
  const pending = effect?.imports.find((i) => i.id === effect.pendingImportId);
  const activeImport = effect?.imports.find(
    (i) => i.id === effect.activeImportId,
  );
  const dirty =
    !!revision &&
    vfxValueKey({ profile, settings, holds }) !==
      vfxValueKey({
        profile: revision.profile,
        settings: revision.frames.map((f) => f.processing),
        holds: revision.frames.map((f) => f.holdTicks),
      });
  function accept(r: VfxReply, hydrate = true) {
    if (
      !alive.current ||
      r.effect.generation < (accepted.current?.effect.generation ?? -1)
    )
      return;
    const priorRevision = accepted.current?.effect.activeRevisionId;
    accepted.current = r;
    setReply(r);
    if (!hydrate) return; // An autosave ACK must never replace newer local edits.
    const rev = r.effect.revisions.find(
      (v) => v.id === r.effect.activeRevisionId,
    );
    const imp = r.effect.imports.find(
      (i) => i.id === (r.effect.pendingImportId || r.effect.activeImportId),
    );
    setProfile(
      r.effect.draft?.profile || rev?.profile || r.effect.draftProfile,
    );
    setSettings(
      r.effect.draft?.settings ||
        rev?.frames.map((f) => f.processing) ||
        imp?.regions.map(() => defaultProcessing()) ||
        [],
    );
    setHolds(
      r.effect.draft?.holds ||
        rev?.frames.map((f) => f.holdTicks) ||
        imp?.regions.map((_, i) =>
          r.effect.effectId === "effect.contact.light" &&
          imp.regions.length === 3
            ? [2, 3, 3][i]
            : 1,
        ) ||
        [],
    );
    setRegions(imp?.regions || []);
    if (priorRevision !== r.effect.activeRevisionId) setChecks(emptyChecks());
  }
  useEffect(() => {
    alive.current = true;
    const requestEpoch = ++epoch.current;
    vfxDesktop
      .command<VfxReply>({ operation: "load" }, projectId, effectId)
      .then((r) => {
        if (alive.current && requestEpoch === epoch.current) accept(r);
      })
      .catch((e) => {
        if (alive.current && requestEpoch === epoch.current)
          setError(String(e));
      });
    return () => {
      alive.current = false;
      epoch.current++;
    };
  }, [projectId, effectId]);
  function enqueue(request: VfxRequest): Promise<VfxReply> {
    const task = queue.current
      .catch(() => {})
      .then(() => vfxDesktop.command<VfxReply>(request, projectId, effectId));
    queue.current = task;
    return task;
  }
  function persistDraft(draft: VfxDraft) {
    const serial = vfxValueKey(draft);
    const existing = draftRequests.current.get(serial);
    if (existing) return existing;
    const task = enqueue({ operation: "save_draft", draft });
    void task
      .then(() => {
        lastPersistedDraft.current = serial;
      })
      .catch(() => {});
    draftRequests.current.set(serial, task);
    void task
      .finally(() => {
        draftRequests.current.delete(serial);
      })
      .catch(() => {});
    return task;
  }
  function refreshLibrary() {
    void changed().catch((e) => {
      if (alive.current)
        setError(`Effect saved, but library refresh failed: ${String(e)}`);
    });
  }
  async function saveDraft(draft: VfxDraft) {
    if (savingRef.current) return;
    savingRef.current = true;
    setSaving(true);
    setSaveFailed(false);
    ++epoch.current;
    try {
      const r = await persistDraft(draft);
      accept(r, false);
      // Update the library badge once, independently of editor busy state.
      if (alive.current) refreshLibrary();
    } catch (e) {
      if (alive.current) {
        setSaveFailed(true);
        setError(`Draft save failed; local edits are retained. ${String(e)}`);
      }
    } finally {
      savingRef.current = false;
      if (alive.current) setSaving(false);
    }
  }
  async function run(request: VfxRequest) {
    if (busyRef.current || !alive.current) return;
    busyRef.current = true;
    ++epoch.current; // An earlier opening/load response cannot overwrite this action.
    setOperation(request.operation);
    setError("");
    setMessage("");
    try {
      const r = await enqueue(request);
      const hydrate = [
        "import_frames",
        "set_regions",
        "approve_import",
        "process",
        "restore",
      ].includes(request.operation);
      accept(r, hydrate);
      if (alive.current) refreshLibrary();
      if (alive.current && request.operation === "verify_export")
        setMessage(
          "Saved PNG, manifest, original hashes and approved pixels verified.",
        );
      return r;
    } catch (e) {
      if (alive.current) setError(String(e));
    } finally {
      busyRef.current = false;
      if (alive.current) setOperation(null);
    }
  }
  async function importPNG() {
    if (busyRef.current || pickerOpen.current) return;
    pickerOpen.current = true;
    try {
      const selected = await vfxDesktop.choosePNG(mode === "frames");
      if (!selected || !alive.current) return;
      const paths = Array.isArray(selected) ? selected : [selected];
      const rectangles = mode === "rectangles" ? JSON.parse(rectText) : null;
      await run({
        operation: "import_frames",
        paths,
        board_count: mode === "board" ? count : null,
        rectangles,
      });
    } catch (e) {
      if (alive.current) setError(String(e));
    } finally {
      pickerOpen.current = false;
    }
  }
  useEffect(() => {
    if (
      colorsText
        .split(/[\s,]+/)
        .filter(Boolean)
        .join(" ") !== profile.palette.colors.join(" ")
    )
      setColorsText(profile.palette.colors.join(" "));
  }, [profile.palette.colors]);
  const draft: VfxDraft = {
    profile,
    settings,
    holds,
    baseRevisionId: effect?.activeRevisionId ?? null,
    importId: effect?.activeImportId ?? null,
  };
  const storedDraft = effect?.draft || {
    profile: revision?.profile || effect?.draftProfile,
    settings: revision?.frames.map((f) => f.processing) || [],
    holds: revision?.frames.map((f) => f.holdTicks) || [],
    baseRevisionId: effect?.activeRevisionId ?? null,
    importId: effect?.activeImportId ?? null,
  };
  const draftSerial = vfxValueKey(draft);
  const storedSerial = vfxValueKey(storedDraft);
  const validDraft =
    profile.cell.every((n) => Number.isInteger(n) && n > 0 && n <= 2048) &&
    profile.anchor.every(
      (n, i) => Number.isInteger(n) && n >= 0 && n < profile.cell[i],
    ) &&
    Number.isInteger(profile.tickHz) &&
    profile.tickHz > 0 &&
    profile.tickHz <= 1000 &&
    (!profile.palette.enforce || profile.palette.colors.length > 0) &&
    profile.palette.colors.every((c) => /^#[\da-f]{6}$/i.test(c)) &&
    settings.length === (activeImport?.regions.length || 0) &&
    holds.length === settings.length &&
    holds.every((n) => Number.isInteger(n) && n > 0 && n <= 36000) &&
    settings.every(
      (p) =>
        Number.isInteger(p.pixelPitch) &&
        p.pixelPitch >= 1 &&
        p.pixelPitch <= 64 &&
        p.offset.every((n) => Number.isInteger(n) && Math.abs(n) <= 4096) &&
        (!p.chroma || /^#[\da-f]{6}$/i.test(p.chroma)) &&
        (p.alphaThreshold === null ||
          (Number.isInteger(p.alphaThreshold) &&
            p.alphaThreshold >= 1 &&
            p.alphaThreshold <= 255)),
    ) &&
    (profile.completion !== "empty_terminal" ||
      (profile.allowEmptyTerminal && !profile.looping));
  const draftNeeded = !!effect && draftSerial !== storedSerial;
  latestDraft.current = { draft, needed: draftNeeded, valid: validDraft };
  useEffect(() => {
    traceVfx({
      kind: "editor-state",
      projectId,
      effectId,
      operation:
        operation ?? (saving ? "saving-draft" : saveFailed ? "failed" : "idle"),
      busy,
      saving,
      importId: effect?.activeImportId,
      revisionId: effect?.activeRevisionId,
      approved: !!revision?.approval && !dirty,
      dirty,
    });
  }, [
    projectId,
    effectId,
    operation,
    saving,
    saveFailed,
    effect?.activeImportId,
    effect?.activeRevisionId,
    !!revision?.approval,
    dirty,
  ]);
  useEffect(() => {
    setError("");
    setSaveFailed(false);
  }, [draftSerial]);
  useEffect(() => {
    if (!draftNeeded || !validDraft || busy || saving || error) return;
    const timer = window.setTimeout(() => {
      void saveDraft(draft);
    }, 250);
    return () => window.clearTimeout(timer);
  }, [draftSerial, storedSerial, busy, saving, error]);
  useEffect(
    () => () => {
      const d = latestDraft.current;
      if (
        d?.needed &&
        d.valid &&
        !busyRef.current &&
        vfxValueKey(d.draft) !== lastPersistedDraft.current
      )
        void persistDraft(d.draft).catch((e) =>
          console.error("VFX draft flush failed", e),
        );
    },
    [projectId, effectId],
  );
  const patch = (p: Partial<VfxProfile>) => setProfile((v) => ({ ...v, ...p }));
  const processing = (i: number, p: Partial<VfxProcessing>) =>
    setSettings((v) => v.map((s, j) => (i === j ? { ...s, ...p } : s)));
  const propose = () => run({ operation: "process", profile, settings, holds });
  if (!effect)
    return (
      <section className="vfx-section">
        {error ? <p role="alert">{error}</p> : "Opening effect…"}
      </section>
    );
  const errors =
    revision?.frames.flatMap((f, i) =>
      f.report.errors.map((e) => `Frame ${i + 1}: ${e}`),
    ) || [];
  return (
    <section
      className="vfx-editor"
      aria-label="VFX effect editor"
      aria-busy={busy}
      data-operation={
        operation ?? (saving ? "saving-draft" : saveFailed ? "failed" : "idle")
      }
    >
      <header className="vfx-toolbar">
        <div>
          <h2>{effect.name}</h2>
          <small>
            {effect.effectId} · {effect.package || "No package assigned"}
          </small>
        </div>
        <span className="vfx-ui-badge">
          {revision?.approval && !dirty ? "OWNER APPROVED" : "DRAFT / PROPOSED"}
        </span>
      </header>
      {error && (
        <p role="alert" className="vfx-notice">
          {error}
        </p>
      )}
      {saving && <p role="status">Saving draft… Editing remains available.</p>}
      {saveFailed && (
        <button
          className={button}
          disabled={busy || saving || !validDraft}
          onClick={() => {
            setError("");
            void saveDraft(draft);
          }}
        >
          Retry draft save
        </button>
      )}
      {reply?.recoveryNotice && (
        <p role="alert" className="vfx-notice">
          {reply.recoveryNotice}
        </p>
      )}
      {message && <p role="status">{message}</p>}
      {dirty && (
        <p className="vfx-notice">
          {draftNeeded
            ? "Unsaved configuration · saving valid draft settings…"
            : "Draft settings saved locally."}{" "}
          Create and review a new candidate before approval/export.
        </p>
      )}
      <nav className="vfx-tabs" aria-label="Effect workflow">
        {[
          "Import & regions",
          "Process & align",
          "Timing & preview",
          "Review & exports",
        ].map((t) => (
          <button
            className={button}
            key={t}
            aria-pressed={tab === t}
            onClick={() => setTab(t)}
          >
            {t}
          </button>
        ))}
      </nav>
      <fieldset className="vfx-editor-body" disabled={busy}>
        <div hidden={tab !== "Import & regions"}>
          <section className="vfx-section">
            <h3>Import original PNGs</h3>
            <p>
              No connected-body detection. Horizontal regions include all
              padding and detached particles. Review every rectangle.
            </p>
            <div className="vfx-toolbar">
              <label>
                Import kind
                <select value={mode} onChange={(e) => setMode(e.target.value)}>
                  <option value="frames">
                    Individual PNG frames · picker order
                  </option>
                  <option value="board">
                    Horizontal AI board · proposed equal regions
                  </option>
                  <option value="rectangles">
                    Transparent sheet · explicit rectangles
                  </option>
                </select>
              </label>
              {mode === "board" && (
                <NumberField
                  label="Proposed frame count"
                  value={count}
                  min={1}
                  max={128}
                  change={setCount}
                />
              )}
              <button className={button} onClick={importPNG}>
                Import VFX PNG{mode === "frames" ? " frames" : " board"}
              </button>
            </div>
            {mode === "rectangles" && (
              <label>
                Rectangles JSON · [x,y,width,height]
                <textarea
                  value={rectText}
                  onChange={(e) => setRectText(e.target.value)}
                />
              </label>
            )}
          </section>
          {(pending || activeImport) && (
            <section className="vfx-section">
              <h3>
                {pending
                  ? "Approve detected frame regions"
                  : "Approved frame regions"}
              </h3>
              <p>
                Keep complete spark groups in a rectangle; transparent regions
                are meaningful. Use arrows to change frame order.
              </p>
              <div className="vfx-originals">
                {reply?.originals.map((o) => (
                  <figure key={o.sourceId}>
                    <div
                      className="vfx-board"
                      style={{
                        aspectRatio: `${o.image.width}/${o.image.height}`,
                      }}
                    >
                      <img src={o.image.dataUrl} alt="Original VFX board" />
                      {regions
                        .filter((r) => r.sourceId === o.sourceId)
                        .map((r, i) => (
                          <span
                            key={i}
                            className="vfx-region-overlay"
                            style={{
                              left: `${(100 * r.rect[0]) / o.image.width}%`,
                              top: `${(100 * r.rect[1]) / o.image.height}%`,
                              width: `${(100 * r.rect[2]) / o.image.width}%`,
                              height: `${(100 * r.rect[3]) / o.image.height}%`,
                            }}
                          >
                            {i + 1}
                          </span>
                        ))}
                    </div>
                    <figcaption>
                      Original bytes retained · {o.image.width}×{o.image.height}
                    </figcaption>
                  </figure>
                ))}
              </div>
              {regions.map((r, i) => (
                <div className="vfx-region-row" key={i}>
                  <strong>Frame {i + 1}</strong>
                  {["X", "Y", "Width", "Height"].map((label, j) => (
                    <NumberField
                      key={label}
                      label={`Frame ${i + 1} region ${label}`}
                      value={r.rect[j]}
                      min={j < 2 ? 0 : 1}
                      change={(v) =>
                        setRegions((rows) =>
                          rows.map((row, n) =>
                            n === i
                              ? {
                                  ...row,
                                  rect: row.rect.map((x, k) =>
                                    k === j ? v : x,
                                  ) as VfxRegion["rect"],
                                }
                              : row,
                          ),
                        )
                      }
                    />
                  ))}
                  <button
                    className={button}
                    disabled={i === 0}
                    onClick={() =>
                      setRegions((rows) => {
                        const copy = [...rows];
                        [copy[i - 1], copy[i]] = [copy[i], copy[i - 1]];
                        return copy;
                      })
                    }
                  >
                    Move frame {i + 1} earlier
                  </button>
                  <button
                    className={button}
                    onClick={() =>
                      setRegions((rows) => rows.filter((_, n) => n !== i))
                    }
                    disabled={regions.length === 1}
                  >
                    Remove region {i + 1}
                  </button>
                </div>
              ))}
              <div className="vfx-toolbar">
                <button
                  className={button}
                  onClick={() => {
                    const first = regions[0];
                    if (first)
                      setRegions((rows) => [
                        ...rows,
                        { ...first, rect: [...first.rect] },
                      ]);
                  }}
                >
                  Add manual region
                </button>
                <button
                  className={button}
                  onClick={() => run({ operation: "set_regions", regions })}
                >
                  Preview edited regions
                </button>
              </div>
              <div className="vfx-crops">
                {reply?.crops.map((p, i) => (
                  <figure key={i}>
                    <img
                      src={p.dataUrl}
                      width={Math.min(p.width * 4, 240)}
                      alt={`Region preview ${i + 1}`}
                    />
                    <figcaption>
                      Frame {i + 1} · {p.width}×{p.height}
                    </figcaption>
                  </figure>
                ))}
              </div>
              <label>
                Region reviewer
                <input
                  value={reviewer}
                  maxLength={160}
                  onChange={(e) => setReviewer(e.target.value)}
                />
              </label>
              <button
                className={button}
                disabled={
                  !pending ||
                  !reviewer.trim() ||
                  vfxValueKey(regions) !== vfxValueKey(pending.regions)
                }
                onClick={() =>
                  run({
                    operation: "approve_import",
                    import_id: pending!.id,
                    reviewer,
                  })
                }
              >
                Approve these frame regions
              </button>
            </section>
          )}
        </div>
        <div hidden={tab !== "Process & align"}>
          <section className="vfx-section">
            <h3>Shared fixed cell · no automatic body alignment</h3>
            <div className="vfx-toolbar">
              <NumberField
                label="Cell width"
                value={profile.cell[0]}
                min={1}
                max={2048}
                change={(n) => patch({ cell: [n, profile.cell[1]] })}
              />
              <NumberField
                label="Cell height"
                value={profile.cell[1]}
                min={1}
                max={2048}
                change={(n) => patch({ cell: [profile.cell[0], n] })}
              />
              <NumberField
                label="Contact anchor X"
                value={profile.anchor[0]}
                min={0}
                change={(n) => patch({ anchor: [n, profile.anchor[1]] })}
              />
              <NumberField
                label="Contact anchor Y"
                value={profile.anchor[1]}
                min={0}
                change={(n) => patch({ anchor: [profile.anchor[0], n] })}
              />
            </div>
            <p>
              Origin stays (0,0). Offset translates the full rectangle, never
              its opaque bounding box. Pitch 1 preserves native scale and
              single-pixel particles. Larger explicit pitches downsample and
              need pixel review.
            </p>
            {settings.map((p, i) => (
              <details key={i} className="vfx-frame-settings" open={i === 0}>
                <summary>
                  Frame {i + 1} · offset ({p.offset.join(",")}) · pitch{" "}
                  {p.pixelPitch}
                </summary>
                <div className="vfx-toolbar">
                  <NumberField
                    label={`Frame ${i + 1} offset X`}
                    value={p.offset[0]}
                    change={(n) => processing(i, { offset: [n, p.offset[1]] })}
                  />
                  <NumberField
                    label={`Frame ${i + 1} offset Y`}
                    value={p.offset[1]}
                    change={(n) => processing(i, { offset: [p.offset[0], n] })}
                  />
                  <NumberField
                    label={`Frame ${i + 1} pixel pitch`}
                    value={p.pixelPitch}
                    min={1}
                    max={64}
                    change={(n) => processing(i, { pixelPitch: n })}
                  />
                  <label>
                    Explicit alpha threshold
                    <input
                      type="number"
                      min={1}
                      max={255}
                      step={1}
                      placeholder="Preserve input alpha"
                      value={p.alphaThreshold ?? ""}
                      onChange={(e) =>
                        processing(i, {
                          alphaThreshold:
                            e.target.value === ""
                              ? null
                              : Number(e.target.value),
                        })
                      }
                    />
                  </label>
                  <label>
                    Chroma color · optional
                    <input
                      placeholder="#00FF00 or blank"
                      value={p.chroma ?? ""}
                      onChange={(e) =>
                        processing(i, { chroma: e.target.value || null })
                      }
                    />
                  </label>
                  <NumberField
                    label={`Frame ${i + 1} chroma tolerance`}
                    value={p.tolerance}
                    min={0}
                    max={255}
                    change={(n) => processing(i, { tolerance: n })}
                  />
                  <label>
                    <input
                      type="checkbox"
                      checked={p.remapPalette}
                      onChange={(e) =>
                        processing(i, { remapPalette: e.target.checked })
                      }
                    />
                    Explicit RGB remap to selected palette
                  </label>
                </div>
              </details>
            ))}
            <button
              className={button}
              disabled={!validDraft || busy}
              onClick={() => run({ operation: "save_draft", draft })}
            >
              Save draft settings now
            </button>
            <p>
              Speckle/component removal: DISABLED. Transparent RGB
              normalization: enabled. No smoothing, trimming or bottom
              alignment.
            </p>
          </section>
          <section className="vfx-section">
            <h3>Named palette profile</h3>
            <div className="vfx-toolbar">
              <label>
                Palette name
                <input
                  value={profile.palette.name}
                  onChange={(e) =>
                    patch({
                      palette: { ...profile.palette, name: e.target.value },
                    })
                  }
                />
              </label>
              <label>
                Authority ID · blank means unassigned
                <input
                  value={profile.palette.authorityId ?? ""}
                  onChange={(e) =>
                    patch({
                      palette: {
                        ...profile.palette,
                        authorityId: e.target.value || null,
                      },
                    })
                  }
                />
              </label>
              <label>
                Package palette reference · optional
                <input
                  value={profile.palette.packageReference ?? ""}
                  onChange={(e) =>
                    patch({
                      palette: {
                        ...profile.palette,
                        packageReference: e.target.value || null,
                      },
                    })
                  }
                />
              </label>
              <label>
                <input
                  type="checkbox"
                  checked={profile.palette.enforce}
                  onChange={(e) =>
                    patch({
                      palette: {
                        ...profile.palette,
                        enforce: e.target.checked,
                      },
                    })
                  }
                />
                Validate exact RGB subset
              </label>
            </div>
            <label>
              RGB colors · separate with spaces
              <textarea
                value={colorsText}
                onChange={(e) => {
                  setColorsText(e.target.value.toUpperCase());
                  patch({
                    palette: {
                      ...profile.palette,
                      colors: e.target.value
                        .toUpperCase()
                        .split(/[\s,]+/)
                        .filter(Boolean),
                    },
                  });
                }}
              />
            </label>
            <div className="environment-swatches">
              {profile.palette.colors
                .filter((c) => /^#[\da-f]{6}$/i.test(c))
                .map((c, i) => (
                  <span style={{ background: c }} title={c} key={i} />
                ))}
            </div>
            <p>
              RGB validation and alpha validation are separate. No nearest-color
              conversion happens unless explicitly selected for a frame. A blank
              authority ID remains unassigned.
            </p>
            <details>
              <summary>Reuse a palette profile as JSON</summary>
              <textarea
                aria-label="Palette profile JSON"
                value={paletteJSON}
                onChange={(e) => setPaletteJSON(e.target.value)}
              />
              <div className="vfx-toolbar">
                <button
                  className={button}
                  onClick={() =>
                    setPaletteJSON(JSON.stringify(profile.palette, null, 2))
                  }
                >
                  Show profile JSON
                </button>
                <button
                  className={button}
                  onClick={() => {
                    try {
                      const p = JSON.parse(paletteJSON);
                      if (
                        typeof p.name !== "string" ||
                        !Array.isArray(p.colors) ||
                        p.colors.some(
                          (c: unknown) =>
                            typeof c !== "string" || !/^#[\da-f]{6}$/i.test(c),
                        ) ||
                        typeof p.enforce !== "boolean"
                      )
                        throw new Error("Invalid RGB profile");
                      patch({
                        palette: {
                          name: p.name,
                          authorityId: p.authorityId || null,
                          packageReference: p.packageReference || null,
                          colors: p.colors,
                          enforce: p.enforce,
                        },
                      });
                    } catch (e) {
                      setError(String(e));
                    }
                  }}
                >
                  Use profile in this draft
                </button>
              </div>
            </details>
          </section>
          <button
            className={button}
            disabled={!activeImport || !!pending}
            onClick={propose}
          >
            Create non-destructive processing preview
          </button>
          <div className="vfx-crops">
            {reply?.frames.map((p, i) => (
              <figure key={i}>
                <img
                  src={reply.crops[i]?.dataUrl}
                  alt={`Source frame ${i + 1}`}
                  width={96}
                />
                <img
                  src={p.dataUrl}
                  alt={`Processed frame ${i + 1}`}
                  width={96}
                />
                <figcaption>
                  Source → fixed cell ·{" "}
                  {revision?.frames[i]?.report.clippedPixels || 0} clipped ·{" "}
                  {revision?.frames[i]?.report.partialAlphaPixels || 0} partial
                  alpha
                </figcaption>
              </figure>
            ))}
          </div>
        </div>
        <div hidden={tab !== "Timing & preview"}>
          <VfxPlayer
            images={
              reply?.frames.length ? reply.frames.map((p) => p.dataUrl) : []
            }
            profile={profile}
            holds={holds}
            reports={revision?.frames.map((f) => f.report)}
          />
          <section className="vfx-section">
            <h3>
              Explicit advancing tick holds · PROPOSED until owner approval
            </h3>
            <div className="vfx-toolbar">
              {holds.map((h, i) => (
                <NumberField
                  key={i}
                  label={`Frame ${i + 1} hold ticks`}
                  value={h}
                  min={1}
                  max={36000}
                  change={(n) =>
                    setHolds((v) => v.map((t, j) => (i === j ? n : t)))
                  }
                />
              ))}
              <NumberField
                label="Simulation ticks per second"
                value={profile.tickHz}
                min={1}
                max={1000}
                change={(n) => patch({ tickHz: n })}
              />
            </div>
            <p>
              Total: {holds.reduce((a, b) => a + b, 0)} advancing ticks. Frozen
              effect-clock ticks consume no holds.
            </p>
            <div className="vfx-toolbar">
              <label>
                <input
                  type="checkbox"
                  checked={profile.looping}
                  onChange={(e) => patch({ looping: e.target.checked })}
                />
                Loop effect
              </label>
              <label>
                Hit-stop policy
                <select
                  value={profile.hitStopPolicy}
                  onChange={(e) =>
                    patch({
                      hitStopPolicy: e.target
                        .value as VfxProfile["hitStopPolicy"],
                    })
                  }
                >
                  <option value="pause">
                    Pause effect clock during hit stop · proposed default
                  </option>
                  <option value="continue">
                    Continue during hit stop · explicit exception
                  </option>
                </select>
              </label>
              <label>
                Completion
                <select
                  value={profile.completion}
                  onChange={(e) =>
                    patch({
                      completion: e.target.value as VfxProfile["completion"],
                    })
                  }
                >
                  <option value="hide">Hide renderer at approved expiry</option>
                  <option value="empty_terminal">
                    Finish on approved empty terminal frame
                  </option>
                </select>
              </label>
              <label>
                <input
                  type="checkbox"
                  checked={profile.allowEmptyTerminal}
                  onChange={(e) =>
                    patch({ allowEmptyTerminal: e.target.checked })
                  }
                />
                Permit fully transparent terminal frame
              </label>
              <label>
                Source orientation
                <select
                  value={profile.sourceOrientation}
                  onChange={(e) => patch({ sourceOrientation: e.target.value })}
                >
                  {["east", "west", "north", "south", "none"].map((v) => (
                    <option key={v}>{v}</option>
                  ))}
                </select>
              </label>
              <label>
                Mirror rule
                <select
                  value={profile.mirrorPolicy}
                  onChange={(e) => patch({ mirrorPolicy: e.target.value })}
                >
                  <option value="horizontal_about_anchor">
                    Horizontal about contact anchor
                  </option>
                  <option value="none">No mirroring</option>
                </select>
              </label>
            </div>
            <button
              className={button}
              disabled={!activeImport || !!pending}
              onClick={propose}
            >
              Save timing/layout as new review revision
            </button>
          </section>
        </div>
        <div hidden={tab !== "Review & exports"}>
          <section className="vfx-section">
            <h3>Owner review · exact revision</h3>
            <p className="vfx-path">
              {revision?.id || "Create a processing preview first"}
            </p>
            {errors.map((e, i) => (
              <p role="alert" key={i}>
                {e}
              </p>
            ))}
            {revision?.frames.map((f, i) => (
              <details key={f.id}>
                <summary>
                  Frame {i + 1}:{" "}
                  {f.report.errors.length ? "BLOCKED" : "technical checks pass"}{" "}
                  · {f.report.opaquePixels} visible pixels
                </summary>
                <p>
                  Unexpected colors:{" "}
                  {f.report.unexpectedColors.join(" ") || "none"}; alpha errors:{" "}
                  {f.report.partialAlphaPixels}; clipping:{" "}
                  {f.report.clippedPixels}.
                </p>
                {f.report.warnings.map((w) => (
                  <p key={w}>{w}</p>
                ))}
                <small className="vfx-path">SHA-256 {f.sha256}</small>
              </details>
            ))}
            <label>
              Owner / reviewer
              <input
                value={reviewer}
                onChange={(e) => setReviewer(e.target.value)}
                maxLength={160}
              />
            </label>
            {(
              [
                ["artwork", "Artwork and meaningful disconnected particles"],
                [
                  "regionsAndAnchor",
                  "Regions, fixed canvas, pixel pitch and shared anchor",
                ],
                ["palette", "Exact RGB subset and palette authority status"],
                ["timing", "Ordered positive hold ticks and loop behavior"],
                [
                  "hitStopAndCompletion",
                  "Hit-stop clock and completion/hide behavior",
                ],
              ] as const
            ).map(([key, label]) => (
              <label className="vfx-check" key={key}>
                <input
                  type="checkbox"
                  checked={checks[key]}
                  onChange={(e) =>
                    setChecks((c) => ({ ...c, [key]: e.target.checked }))
                  }
                />
                {label}
              </label>
            ))}
            <button
              className={button}
              disabled={
                !revision ||
                !!revision.approval ||
                !!pending ||
                dirty ||
                errors.length > 0 ||
                !reviewer.trim() ||
                !Object.values(checks).every(Boolean)
              }
              onClick={() =>
                run({
                  operation: "approve",
                  revision_id: revision!.id,
                  reviewer,
                  checks,
                })
              }
            >
              Approve exact VFX revision
            </button>
          </section>
          <section className="vfx-section">
            <h3>Validated export</h3>
            <p>
              PNG + manifest.json + source.json + byte-identical originals +
              review PNG sequence. Guides and backgrounds are excluded. Engine
              import adapters are not implemented here.
            </p>
            <NumberField
              label="Sheet gutter"
              value={gutter}
              min={0}
              max={64}
              change={setGutter}
            />
            <button
              className={button}
              disabled={
                !revision?.approval || dirty || !!pending || errors.length > 0
              }
              onClick={() =>
                run({ operation: "export", revision_id: revision!.id, gutter })
              }
            >
              Export approved VFX
            </button>
            {[...effect.exports].reverse().map((x) => (
              <div className="vfx-history-row" key={x.id}>
                <strong>{x.exportedAt}</strong>
                <small className="vfx-path">{x.path}</small>
                <button
                  className={button}
                  onClick={() =>
                    run({ operation: "verify_export", export_id: x.id })
                  }
                >
                  Verify saved export
                </button>
                <button
                  className={button}
                  onClick={() =>
                    run({ operation: "open_export", export_id: x.id })
                  }
                >
                  Open export folder
                </button>
              </div>
            ))}
          </section>
          <section className="vfx-section">
            <h3>Recoverable revision history</h3>
            <p>
              Restoring selects prior immutable frames and their holds/profile.
              New previews retain all prior exports.
            </p>
            {[...effect.revisions].reverse().map((r) => (
              <div className="vfx-history-row" key={r.id}>
                <strong>
                  {r.createdAt} · {r.approval ? "owner approved" : "draft"} ·{" "}
                  {r.frames.length} frames
                </strong>
                <small>
                  {r.profile.cell.join("×")} · holds{" "}
                  {r.frames.map((f) => f.holdTicks).join(",")} · anchor{" "}
                  {r.profile.anchor.join(",")}
                </small>
                <button
                  className={button}
                  disabled={r.id === revision?.id}
                  onClick={() =>
                    run({ operation: "restore", revision_id: r.id })
                  }
                >
                  Restore revision
                </button>
                {r.importId === activeImport?.id &&
                  r.frames.map((f, i) => (
                    <button
                      key={f.id}
                      className={button}
                      onClick={() => {
                        setSettings((v) =>
                          v.map((p, j) => (i === j ? f.processing : p)),
                        );
                        setHolds((v) =>
                          v.map((h, j) => (i === j ? f.holdTicks : h)),
                        );
                        setMessage(
                          `Frame ${i + 1} settings restored to the draft; process and review before export.`,
                        );
                      }}
                    >
                      Use frame {i + 1} settings
                    </button>
                  ))}
              </div>
            ))}
          </section>
        </div>
      </fieldset>
    </section>
  );
}

export function VfxPlayer({
  images,
  profile,
  holds,
  reports,
}: {
  images: string[];
  profile: VfxProfile;
  holds: number[];
  reports?: VfxReport[];
}) {
  const [tick, setTick] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [hitStop, setHitStop] = useState(false);
  const [fps, setFps] = useState(30);
  const [zoom, setZoom] = useState(8);
  const [background, setBackground] = useState("checker");
  const [mirror, setMirror] = useState(false);
  const [crosshair, setCrosshair] = useState(true);
  const [temporary, setTemporary] = useState<string | null>(null);
  const [contrast, setContrast] = useState(false);
  const current = frameAtTick(tick, holds, profile.looping, profile.completion);
  const report = current.index === null ? undefined : reports?.[current.index];
  const mirroredClipping =
    mirror && report?.bounds
      ? 2 * profile.anchor[0] - report.bounds[0] - report.bounds[2] < 0 ||
        2 * profile.anchor[0] - report.bounds[0] > profile.cell[0]
      : false;
  useEffect(() => {
    setTick(0);
    setPlaying(false);
  }, [images.join("|"), holds.join(","), profile.looping]);
  useEffect(() => {
    if (!playing) return;
    let fraction = 0;
    const timer = window.setInterval(() => {
      if (hitStop && profile.hitStopPolicy === "pause") return;
      fraction += profile.tickHz / fps;
      const delta = Math.floor(fraction);
      fraction -= delta;
      setTick((t) =>
        advanceVfxClock(t, delta, hitStop, false, profile.hitStopPolicy),
      );
    }, 1000 / fps);
    return () => window.clearInterval(timer);
  }, [playing, hitStop, profile.hitStopPolicy, profile.tickHz, fps]);
  useEffect(() => {
    if (current.completed) setPlaying(false);
  }, [current.completed]);
  return (
    <section className="vfx-section" aria-label="Fixed-cell VFX preview">
      <h3>Fixed-cell preview</h3>
      {(!!report?.clippedPixels || mirroredClipping) && (
        <p role="alert" className="vfx-notice">
          Clipping warning:{" "}
          {mirroredClipping
            ? "mirrored art extends outside the shared cell"
            : "candidate lost nontransparent pixels"}
          . Correct cell/offset/pitch and review.
        </p>
      )}
      {!!report?.edgePixels && (
        <p>
          Visible pixels touch the cell edge. Review transparent safety padding.
        </p>
      )}
      <div className="vfx-toolbar">
        <button
          className={button}
          onClick={() => {
            if (current.completed) setTick(0);
            setPlaying((p) => !p);
          }}
          disabled={!images.length}
        >
          {playing ? <Pause size={16} /> : <Play size={16} />}{" "}
          {playing ? "Pause" : "Play"}
        </button>
        <button className={button} onClick={() => setTick(0)}>
          Restart
        </button>
        <button
          className={button}
          onClick={() =>
            setTick((t) =>
              advanceVfxClock(t, 1, hitStop, false, profile.hitStopPolicy),
            )
          }
        >
          <StepForward size={16} />
          Step one tick
        </button>
        <button
          className={button}
          onClick={() => {
            const next =
              ((current.index ?? -1) + 1) % Math.max(holds.length, 1);
            setTick(holds.slice(0, next).reduce((a, b) => a + b, 0));
            setPlaying(false);
          }}
        >
          Next frame
        </button>
        <label>
          Nearest zoom
          <select
            value={zoom}
            onChange={(e) => setZoom(Number(e.target.value))}
          >
            {[1, 2, 4, 8, 12, 16].map((n) => (
              <option key={n} value={n}>
                {n}×{n === 1 ? " native" : ""}
              </option>
            ))}
          </select>
        </label>
        <label>
          Background
          <select
            value={background}
            onChange={(e) => setBackground(e.target.value)}
          >
            {["checker", "dark", "light"].map((v) => (
              <option key={v}>{v}</option>
            ))}
          </select>
        </label>
        <NumberField
          label="Preview FPS · study only"
          value={fps}
          min={1}
          max={120}
          change={(v) => setFps(Math.min(120, Math.max(1, v)))}
        />
      </div>
      <div className="vfx-toolbar">
        <label>
          <input
            type="checkbox"
            checked={hitStop}
            onChange={(e) => setHitStop(e.target.checked)}
          />
          Simulate hit stop
        </label>
        <label>
          <input
            type="checkbox"
            checked={crosshair}
            onChange={(e) => setCrosshair(e.target.checked)}
          />
          Contact anchor crosshair
        </label>
        <label>
          <input
            type="checkbox"
            checked={mirror}
            onChange={(e) => setMirror(e.target.checked)}
            disabled={profile.mirrorPolicy === "none"}
          />
          Mirror about anchor
        </label>
        <label>
          <input
            type="checkbox"
            checked={contrast}
            onChange={(e) => setContrast(e.target.checked)}
          />
          Grayscale contrast study
        </label>
        <label>
          Temporary gameplay background
          <input
            type="file"
            accept="image/png,image/jpeg"
            onChange={(e) => {
              const file = e.target.files?.[0];
              if (!file) return;
              const reader = new FileReader();
              reader.onload = () => setTemporary(String(reader.result));
              reader.readAsDataURL(file);
            }}
          />
        </label>
        <button className={button} onClick={() => setTemporary(null)}>
          Clear temporary background
        </button>
      </div>
      <div
        className="vfx-preview-stage"
        style={{ backgroundImage: temporary ? `url(${temporary})` : undefined }}
      >
        <div
          data-testid="vfx-cell"
          className={`vfx-cell ${background}`}
          style={{
            width: profile.cell[0] * zoom,
            height: profile.cell[1] * zoom,
            filter: contrast ? "grayscale(1)" : undefined,
          }}
        >
          {current.index !== null && images[current.index] && (
            <img
              alt={`VFX frame ${current.index + 1}`}
              src={images[current.index]}
              style={{
                width: "100%",
                height: "100%",
                transform: mirror ? "scaleX(-1)" : undefined,
                transformOrigin: `${profile.anchor[0] * zoom}px ${profile.anchor[1] * zoom}px`,
              }}
            />
          )}
          {crosshair && (
            <>
              <span
                className="vfx-anchor-h"
                style={{ top: profile.anchor[1] * zoom }}
              />
              <span
                className="vfx-anchor-v"
                style={{ left: profile.anchor[0] * zoom }}
              />
            </>
          )}
        </div>
      </div>
      <p role="status">
        Tick {tick} / {current.total} ·{" "}
        {current.completed
          ? "Completed · renderer hidden / transparent terminal"
          : current.index === null
            ? "No frames"
            : `Frame ${current.index + 1}`}{" "}
        · anchor ({profile.anchor.join(",")})
        {hitStop && profile.hitStopPolicy === "pause" ? " · CLOCK FROZEN" : ""}
      </p>
      <p>
        Cell outline shows transparent padding.
        Guides/background/contrast/mirroring are review overlays only; exported
        source pixels stay unchanged.
      </p>
    </section>
  );
}
