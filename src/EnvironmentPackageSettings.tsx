import { useEffect, useState } from "react";
import type {
  EnvironmentAssetType,
  EnvironmentRequest,
  EnvironmentState,
  PaletteProfile,
} from "./environmentTypes";

export const OUTBACK_PALETTE: PaletteProfile = {
  id: "biome.sunscorched.v1",
  name: "Sunscorched Outback · approved ten colors",
  colors: [
    "#F3B56B",
    "#D9704C",
    "#A44950",
    "#5A303B",
    "#814032",
    "#A95438",
    "#D37A45",
    "#966644",
    "#C68B52",
    "#47766A",
  ],
  slots: [],
  enforce: true,
};
export const UNRESTRICTED_PALETTE: PaletteProfile = {
  id: "custom.original",
  name: "Original colors · unrestricted",
  colors: [],
  slots: [],
  enforce: false,
};

export function PaletteEditor({
  value,
  change,
  locked = false,
}: {
  value: PaletteProfile;
  change: (p: PaletteProfile) => void;
  locked?: boolean;
}) {
  const [json, setJSON] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [colorsText, setColorsText] = useState(value.colors.join(" "));
  const [slotsText, setSlotsText] = useState(value.slots.join(","));
  useEffect(() => {
    if (
      colorsText
        .split(/[\s,]+/)
        .filter(Boolean)
        .join(" ") !== value.colors.join(" ")
    )
      setColorsText(value.colors.join(" "));
  }, [value.colors]);
  useEffect(() => {
    if (
      slotsText
        .split(",")
        .map((s) => s.trim())
        .join(",") !== value.slots.join(",")
    )
      setSlotsText(value.slots.join(","));
  }, [value.slots]);
  return (
    <fieldset disabled={locked} className="environment-parameters">
      <legend>Palette profile · conversion is always explicit</legend>
      <label>
        Profile{" "}
        <select
          value={
            value.id === "biome.sunscorched.v1"
              ? "outback"
              : value.enforce
                ? "custom"
                : "original"
          }
          onChange={(e) =>
            change(
              e.target.value === "outback"
                ? OUTBACK_PALETTE
                : e.target.value === "original"
                  ? UNRESTRICTED_PALETTE
                  : {
                      ...value,
                      id: "custom.palette",
                      name: "Custom palette",
                      enforce: true,
                      colors: value.colors.length
                        ? value.colors
                        : ["#000000", "#FFFFFF"],
                    },
            )
          }
        >
          <option value="original">
            Original colors · no palette restriction
          </option>
          <option value="outback">
            Sunscorched Outback · exact approved ten
          </option>
          <option value="custom">Custom palette profile</option>
        </select>
      </label>
      <label>
        Profile name{" "}
        <input
          value={value.name}
          onChange={(e) =>
            change({ ...value, name: e.target.value, id: "custom.palette" })
          }
        />
      </label>
      <label>
        <input
          type="checkbox"
          checked={value.enforce}
          onChange={(e) =>
            change({
              ...value,
              enforce: e.target.checked,
              id: "custom.palette",
            })
          }
        />
        Enforce colors as a technical gate
      </label>
      <label>
        Colors · #RRGGBB separated by spaces
        <textarea
          value={colorsText}
          onChange={(e) => {
            setColorsText(e.target.value.toUpperCase());
            change({
              ...value,
              id: "custom.palette",
              colors: e.target.value
                .toUpperCase()
                .split(/[\s,]+/)
                .filter(Boolean),
              slots: [],
            });
          }}
        />
      </label>
      <label>
        Named slots · comma separated, optional
        <input
          value={slotsText}
          onChange={(e) => {
            setSlotsText(e.target.value);
            change({
              ...value,
              slots: e.target.value
                ? e.target.value.split(",").map((s) => s.trim())
                : [],
            });
          }}
        />
      </label>
      <div className="environment-swatches">
        {value.colors
          .filter((c) => /^#[\da-f]{6}$/i.test(c))
          .map((c, i) => (
            <span
              key={i}
              style={{ background: c }}
              title={`${value.slots[i] || i + 1}: ${c}`}
            />
          ))}
      </div>
      <details>
        <summary>Import / export profile JSON</summary>
        <label>
          Paste a palette profile
          <textarea value={json} onChange={(e) => setJSON(e.target.value)} />
        </label>
        <div className="environment-actions">
          <button
            className="button ghost"
            type="button"
            onClick={() => {
              try {
                const p = JSON.parse(json) as PaletteProfile;
                if (
                  !p.id ||
                  !p.name ||
                  !Array.isArray(p.colors) ||
                  !p.colors.every((c) => /^#[\da-f]{6}$/i.test(c)) ||
                  !Array.isArray(p.slots) ||
                  typeof p.enforce !== "boolean"
                )
                  throw new Error(
                    "Expected id, name, colors, slots and enforce fields",
                  );
                change(p);
                setError(null);
              } catch (e) {
                setError(String(e));
              }
            }}
          >
            Import pasted JSON
          </button>
          <button
            className="button ghost"
            type="button"
            onClick={() => {
              void navigator.clipboard
                .writeText(JSON.stringify(value, null, 2))
                .then(() => setCopied(true))
                .catch((e) => setError(String(e)));
            }}
          >
            {copied ? "Profile copied" : "Copy profile JSON"}
          </button>
        </div>
        {error && (
          <p role="alert" className="environment-alert">
            {error}
          </p>
        )}
      </details>
      <small>
        Changing the profile does not alter image pixels. Save and Process
        proposes a new remap only when you enable palette remapping.
      </small>
    </fieldset>
  );
}

export function PackageSettings({
  environment: s,
  disabled,
  run,
}: {
  environment: EnvironmentState;
  disabled: boolean;
  run: (r: EnvironmentRequest) => void;
}) {
  const [name, setName] = useState(s.displayName);
  const [biome, setBiome] = useState(s.biomeId);
  const [arena, setArena] = useState(s.arenaId);
  const [canvas, setCanvas] = useState(s.canvas);
  const [palette, setPalette] = useState(s.palette);
  const [guides, setGuides] = useState(s.guides);
  const [markersText, setMarkersText] = useState(s.guides.markersX.join(","));
  const [assetName, setAssetName] = useState("");
  const [assetType, setAssetType] =
    useState<EnvironmentAssetType>("background_layer");
  const [size, setSize] = useState<[number, number]>([64, 64]);
  const [duplicateName, setDuplicateName] = useState(`${s.displayName} copy`);
  return (
    <div className="environment-settings">
      <fieldset disabled={disabled} className="environment-parameters">
        <legend>Package configuration</legend>
        <label>
          Display name{" "}
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            maxLength={160}
          />
        </label>
        <div className="environment-actions">
          <label>
            Biome ID · optional{" "}
            <input value={biome} onChange={(e) => setBiome(e.target.value)} />
          </label>
          <label>
            Arena ID · optional{" "}
            <input value={arena} onChange={(e) => setArena(e.target.value)} />
          </label>
        </div>
        <div className="environment-actions">
          {[0, 1].map((i) => (
            <label key={i}>
              Canvas {i === 0 ? "width" : "height"}
              <input
                disabled={!!s.templateId}
                type="number"
                min={1}
                max={4096}
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
        </div>
        <PaletteEditor
          value={palette}
          change={setPalette}
          locked={!!s.templateId}
        />
        <div className="environment-actions">
          <label>
            Ground guide Y{" "}
            <input
              type="number"
              min={0}
              value={guides.groundY ?? ""}
              onChange={(e) =>
                setGuides({
                  ...guides,
                  groundY:
                    e.target.value === "" ? null : Number(e.target.value),
                })
              }
            />
          </label>
          <label>
            HUD exclusion line Y{" "}
            <input
              type="number"
              min={0}
              value={guides.hudY ?? ""}
              onChange={(e) =>
                setGuides({
                  ...guides,
                  hudY: e.target.value === "" ? null : Number(e.target.value),
                })
              }
            />
          </label>
          <label>
            Position / gameplay markers X{" "}
            <input
              value={markersText}
              onChange={(e) => setMarkersText(e.target.value)}
            />
          </label>
        </div>
        <p>
          Guides are review overlays, never exported. Canvas/palette changes
          stale only affected assets in this package; older files and approvals
          stay in history.
        </p>
        <button
          className="button primary"
          disabled={!name.trim()}
          onClick={() =>
            run({
              operation: "configure",
              name,
              biome,
              arena,
              canvas,
              palette,
              guides: {
                ...guides,
                markersX: markersText
                  .split(/[,\s]+/)
                  .filter(Boolean)
                  .map(Number),
              },
            })
          }
        >
          Save package configuration
        </button>
        {s.templateId && (
          <small>
            Outback template keeps 480×270, its ten-color palette and Y=224
            surface rule. Duplicate configuration for a custom environment.
          </small>
        )}
      </fieldset>
      <fieldset disabled={disabled} className="environment-parameters">
        <legend>Add an asset slot</legend>
        <label>
          Name{" "}
          <input
            value={assetName}
            onChange={(e) => setAssetName(e.target.value)}
            placeholder="e.g. distant mountains"
          />
        </label>
        <label>
          Type{" "}
          <select
            value={assetType}
            onChange={(e) =>
              setAssetType(e.target.value as EnvironmentAssetType)
            }
          >
            <option value="background_layer">
              Full-canvas background layer
            </option>
            <option value="prop">Independent prop</option>
            <option value="tile">Tile / repeating texture</option>
            <option value="overlay">Static overlay</option>
          </select>
        </label>
        {assetType !== "background_layer" && (
          <div className="environment-actions">
            {[0, 1].map((i) => (
              <label key={i}>
                Native {i === 0 ? "width" : "height"}
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
        <button
          className="button ghost"
          disabled={!assetName.trim()}
          onClick={() => {
            run({
              operation: "add_asset",
              name: assetName,
              asset_type: assetType,
              native_size: assetType === "background_layer" ? null : size,
              rules: {
                alpha: "binary",
                groundY: null,
                seamlessRequired: false,
              },
            });
            setAssetName("");
          }}
        >
          Add asset
        </button>
      </fieldset>
      <fieldset disabled={disabled} className="environment-parameters">
        <legend>Active package membership · source history is retained</legend>
        {s.layers.map((l) => (
          <div className="environment-export-row" key={l.assetId}>
            <span>
              {l.name} · {l.removed ? "removed" : l.assetType}
            </span>
            <button
              className="button ghost"
              onClick={() =>
                run({
                  operation: l.removed ? "restore_asset" : "remove_asset",
                  order: l.order,
                })
              }
            >
              {l.removed ? "Restore to package" : "Remove from package"}
            </button>
          </div>
        ))}
      </fieldset>
      <fieldset disabled={disabled} className="environment-parameters">
        <legend>Duplicate configuration only</legend>
        <label>
          New name{" "}
          <input
            value={duplicateName}
            onChange={(e) => setDuplicateName(e.target.value)}
          />
        </label>
        <p>
          Copies slots, rules, palette and layout into an independent Custom
          package. It does not duplicate artwork, approvals or exports.
        </p>
        <button
          className="button ghost"
          disabled={!duplicateName.trim()}
          onClick={() => run({ operation: "duplicate", name: duplicateName })}
        >
          Duplicate package configuration
        </button>
      </fieldset>
    </div>
  );
}
