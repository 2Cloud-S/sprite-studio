import { useEffect, useRef, useState } from "react";
import { Layers3, Plus } from "lucide-react";
import { EnvironmentWorkspace } from "./EnvironmentWorkspace";
import { environmentDesktop } from "./services/environment";
import {
  PaletteEditor,
  OUTBACK_PALETTE,
  UNRESTRICTED_PALETTE,
} from "./EnvironmentPackageSettings";
import type {
  EnvironmentLibrary,
  EnvironmentCatalog,
  PaletteProfile,
} from "./environmentTypes";

export function EnvironmentAssets({ workspace }: { workspace: string }) {
  const preferenceKey = `sprite-studio:environment:${workspace}`;
  const [library, setLibrary] = useState<EnvironmentLibrary | null>(null);
  const [ownerId, setOwnerId] = useState(
    () => localStorage.getItem(preferenceKey + ":owner") || "",
  );
  const [packageId, setPackageId] = useState(
    () => localStorage.getItem(preferenceKey + ":package") || "",
  );
  const [creating, setCreating] = useState(false);
  const [newOwner, setNewOwner] = useState(false);
  const [name, setName] = useState("");
  const [projectName, setProjectName] = useState("");
  const [template, setTemplate] = useState(false);
  const [canvas, setCanvas] = useState<[number, number]>([480, 270]);
  const [palette, setPalette] = useState<PaletteProfile>(UNRESTRICTED_PALETTE);
  const [layers, setLayers] = useState(3);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const owner = library?.owners.find((o) => o.id === ownerId);
  const selectedPackage = owner?.catalog.packages.find(
    (p) => p.id === packageId,
  );
  useEffect(() => {
    let cancelled = false;
    environmentDesktop
      .library({ operation: "load" })
      .then((l) => {
        if (!cancelled) setLibrary(l);
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [workspace]);
  useEffect(() => {
    localStorage.setItem(preferenceKey + ":owner", ownerId);
    localStorage.setItem(preferenceKey + ":package", packageId);
  }, [preferenceKey, ownerId, packageId]);
  const changed = (catalog: EnvironmentCatalog, id: string) => {
    setLibrary((l) =>
      l
        ? {
            ...l,
            owners: l.owners.map((o) =>
              o.id === ownerId ? { ...o, catalog } : o,
            ),
          }
        : l,
    );
    setPackageId(id);
  };
  async function createOwner() {
    if (busyRef.current) return;
    busyRef.current = true;
    setBusy(true);
    setError(null);
    try {
      const l = await environmentDesktop.library({
        operation: "create_project",
        name: projectName,
      });
      setLibrary(l);
      setOwnerId(l.createdId!);
      setPackageId("");
      setNewOwner(false);
      setProjectName("");
      setCreating(true);
    } catch (e) {
      setError(String(e));
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  }
  async function createPackage() {
    if (!ownerId || busyRef.current) return;
    busyRef.current = true;
    setBusy(true);
    setError(null);
    try {
      const reply = await environmentDesktop.command(ownerId, {
        operation: "create",
        name,
        template,
        canvas,
        palette: template ? OUTBACK_PALETTE : palette,
        layer_count: layers,
      });
      changed(reply.catalog, reply.environment.packageId);
      setCreating(false);
      setName("");
    } catch (e) {
      setError(String(e));
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  }
  return (
    <div className="environment-assets">
      <header className="page-heading">
        <div>
          <span className="eyebrow">ENVIRONMENT ASSETS</span>
          <h1>Build a world, layer by layer.</h1>
          <p>
            Independent packages for backgrounds, props, tiles and overlays. No
            character cells or game-engine connection required.
          </p>
        </div>
      </header>
      <div className="environment-hub-controls">
        <label>
          Environment project{" "}
          <select
            value={ownerId}
            disabled={busy}
            onChange={(e) => {
              setOwnerId(e.target.value);
              setPackageId("");
              setCreating(false);
            }}
          >
            <option value="">Choose a project…</option>
            {library?.owners.map((o) => (
              <option key={o.id} value={o.id}>
                {o.name}
                {o.kind === "existing_project"
                  ? " · existing workspace project"
                  : ""}
              </option>
            ))}
          </select>
        </label>
        <button
          className="button ghost"
          disabled={busy}
          onClick={() => setNewOwner((v) => !v)}
        >
          <Plus size={16} />
          New environment project
        </button>
        {owner && (
          <>
            <label>
              Package{" "}
              <select
                disabled={busy}
                value={selectedPackage?.id || ""}
                onChange={(e) => {
                  setPackageId(e.target.value);
                  setCreating(false);
                }}
              >
                <option value="">Choose a package…</option>
                {owner.catalog.packages.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name} · {p.canvas.join("×")}
                  </option>
                ))}
              </select>
            </label>
            <button
              className="button primary"
              disabled={busy}
              onClick={() => setCreating((v) => !v)}
            >
              <Plus size={16} />
              Create package
            </button>
          </>
        )}
      </div>
      {error && (
        <p className="environment-alert" role="alert">
          {error}
        </p>
      )}
      {[...(library?.failures || []), ...(owner?.catalog.failures || [])].map(
        (f) => (
          <p key={f} className="environment-alert">
            {f} · metadata retained; do not reset the workspace.
          </p>
        ),
      )}
      {newOwner && (
        <form
          className="environment-parameters"
          onSubmit={(e) => {
            e.preventDefault();
            void createOwner();
          }}
        >
          <h2>New environment project</h2>
          <label>
            Project name{" "}
            <input
              autoFocus
              value={projectName}
              maxLength={160}
              onChange={(e) => setProjectName(e.target.value)}
              placeholder="e.g. My next game"
            />
          </label>
          <p>
            Creates environment-only storage under this workspace. No character
            project is created or selected.
          </p>
          <div className="environment-actions">
            <button
              className="button primary"
              disabled={busy || !projectName.trim()}
            >
              Create environment project
            </button>
            <button
              type="button"
              className="button ghost"
              onClick={() => setNewOwner(false)}
            >
              Cancel
            </button>
          </div>
        </form>
      )}
      {creating && owner && (
        <form
          className="environment-parameters"
          onSubmit={(e) => {
            e.preventDefault();
            void createPackage();
          }}
        >
          <h2>Create environment package</h2>
          <label>
            Package name{" "}
            <input
              autoFocus
              value={name}
              maxLength={160}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g. Canyon sunset"
            />
          </label>
          <label>
            Starting configuration{" "}
            <select
              value={template ? "outback" : "custom"}
              onChange={(e) => setTemplate(e.target.value === "outback")}
            >
              <option value="custom">
                Custom environment · game independent
              </option>
              <option value="outback">
                Sunscorched Outback · six layers / 480×270 / Y=224
              </option>
            </select>
          </label>
          {template ? (
            <p>
              Retains the exact six named layer slots, ten approved colors and
              surface validation. No artwork is generated.
            </p>
          ) : (
            <>
              <div className="environment-actions">
                {[0, 1].map((i) => (
                  <label key={i}>
                    Native {i === 0 ? "width" : "height"}
                    <input
                      type="number"
                      min={1}
                      max={4096}
                      required
                      value={canvas[i]}
                      onChange={(e) =>
                        setCanvas(
                          i === 0
                            ? [Number(e.target.value), canvas[1]]
                            : [canvas[0], Number(e.target.value)],
                        )
                      }
                    />
                  </label>
                ))}
                <label>
                  Initial layer slots{" "}
                  <input
                    type="number"
                    min={0}
                    max={128}
                    required
                    value={layers}
                    onChange={(e) => setLayers(Number(e.target.value))}
                  />
                </label>
              </div>
              <PaletteEditor value={palette} change={setPalette} />
            </>
          )}
          <div className="environment-actions">
            <button className="button primary" disabled={busy || !name.trim()}>
              Create package
            </button>
            <button
              type="button"
              className="button ghost"
              onClick={() => setCreating(false)}
            >
              Cancel
            </button>
          </div>
        </form>
      )}
      {!creating && !selectedPackage && (
        <section className="environment-package-library">
          <h2>
            <Layers3 size={21} />
            Biomes & packages{owner ? ` · ${owner.name}` : ""}
          </h2>
          <p>
            {!owner
              ? "Choose an existing workspace project above, or create an independent environment project."
              : `${owner.catalog.packages.length} registered packages. Open one or create a new configuration.`}
          </p>
          <div className="environment-asset-grid">
            {owner?.catalog.packages.map((p) => (
              <button
                key={p.id}
                className="environment-package-card"
                onClick={() => setPackageId(p.id)}
              >
                <Layers3 size={26} />
                <strong>{p.name}</strong>
                <span>
                  {p.canvas.join(" × ")} · {p.assets} asset slots
                </span>
                <small>{p.id}</small>
                <span>Open package →</span>
              </button>
            ))}
          </div>
        </section>
      )}
      {!creating && owner && selectedPackage && (
        <EnvironmentWorkspace
          key={`${owner.id}:${packageId}`}
          projectId={owner.id}
          packageId={packageId}
          onChange={changed}
        />
      )}
    </div>
  );
}
