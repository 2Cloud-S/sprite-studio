import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { EnvironmentWorkspace } from "../src/EnvironmentWorkspace";
import { EnvironmentAssets } from "../src/EnvironmentAssets";
import {
  activeRevision,
  assetStatus,
  environmentCounts,
  matchesEnvironmentAsset,
} from "../src/domain/environment";
import type {
  EnvironmentState,
  EnvironmentReply,
  EnvironmentRequest,
} from "../src/environmentTypes";
import { useStudio } from "../src/store/studio";
import App from "../src/App";
import { useState } from "react";
import {
  PaletteEditor,
  UNRESTRICTED_PALETTE,
} from "../src/EnvironmentPackageSettings";

const mock = vi.hoisted(() => ({
  command: vi.fn(),
  library: vi.fn(),
  choosePNG: vi.fn(),
}));
vi.mock("../src/services/environment", () => ({ environmentDesktop: mock }));
vi.mock("../src/AnimationWorkspace", () => ({
  AnimationWorkspace: () => <div>Character animation workspace</div>,
}));
vi.mock("../src/SnapStage", () => ({ SnapStage: () => null }));
vi.mock("../src/CleanupStage", () => ({ CleanupStage: () => null }));
vi.mock("../src/ReferenceExportPanel", () => ({
  ReferenceExportPanel: () => null,
}));
const preview = {
  dataUrl: "data:image/png;base64,AA==",
  width: 8,
  height: 6,
  mimeType: "image/png",
};
const report = {
  width: 8,
  height: 6,
  origin: [0, 0],
  colorCount: 1,
  approvedColors: [],
  unexpectedColors: [],
  unexpectedColorCount: 0,
  opaquePixels: 1,
  transparentPixels: 47,
  partialAlphaPixels: 0,
  palettePass: true,
  alphaPass: true,
  canvasPass: true,
  groundPass: true,
  detectedPixelPitch: null,
  warnings: [],
  errors: [],
  valid: true,
  colorUsage: [],
  edgeMismatches: [0, 0],
};
function fixture(): EnvironmentState {
  return {
    schemaVersion: 2,
    processorVersion: "static.environment.v2",
    projectId: "world",
    biomeId: "",
    arenaId: "",
    paletteVersion: "original",
    canvas: [8, 6],
    origin: [0, 0],
    packageId: "package-test",
    displayName: "Custom sunset",
    templateId: null,
    palette: {
      id: "original",
      name: "Original",
      colors: [],
      slots: [],
      enforce: false,
    },
    guides: { groundY: 4, hudY: null, markersX: [2, 6] },
    masterReference: null,
    packageExports: [],
    compositeExports: [],
    exportHistory: [],
    revision: 0,
    layers: [
      {
        assetId: "sky",
        name: "Sky layer",
        assetType: "background_layer",
        order: 0,
        parameters: {
          resizeToCanvas: false,
          canvasMode: "preserve",
          offset: [0, 0],
          pixelPitch: 1,
          removeBackground: null,
          backgroundTolerance: 0,
          binaryAlpha: false,
          remapPalette: false,
        },
        currentSourceId: "source",
        currentRevisionId: "rev",
        sources: [
          {
            id: "source",
            originalName: "sky.png",
            path: "environments/package-test/original.png",
            sha256: "source-hash",
            report: report as never,
            importedAt: "",
          },
        ],
        revisions: [
          {
            id: "rev",
            sourceId: "source",
            sourceHash: "source-hash",
            path: "environments/package-test/processed.png",
            sha256: "processed-hash",
            parameters: {
              resizeToCanvas: false,
              canvasMode: "preserve",
              offset: [0, 0],
              pixelPitch: 1,
              removeBackground: null,
              backgroundTolerance: 0,
              binaryAlpha: false,
              remapPalette: false,
            },
            report: report as never,
            reviewer: null,
            approvedAt: null,
            createdAt: "",
          },
        ],
        exports: [],
        exportedRevisionIds: [],
        removed: false,
        rules: { alpha: "binary", groundY: null, seamlessRequired: false },
        nativeSize: null,
        pivot: null,
        position: [0, 0],
        modifiedAt: "",
      },
    ],
  };
}
const catalog = {
  packages: [
    {
      id: "package-test",
      name: "Custom sunset",
      canvas: [8, 6],
      assets: 1,
      revision: 0,
    },
  ],
  failures: [],
};
beforeEach(() => {
  localStorage.clear();
  mock.library.mockResolvedValue({
    owners: [{ id: "world", name: "World only", kind: "environment", catalog }],
    createdId: null,
    failures: [],
  });
  mock.command.mockImplementation(
    async (_id: string, r: EnvironmentRequest): Promise<EnvironmentReply> => ({
      environment: fixture(),
      catalog: catalog as never,
      recoveryNotice: null,
      result: r.operation.includes("preview") ? preview : null,
    }),
  );
  mock.choosePNG.mockResolvedValue(null);
  useStudio.setState({
    boot: vi.fn(async () => {}),
    settings: {
      workspaceRoot: "E:/synthetic",
      lastProjectId: null,
      snapperExecutable: null,
      snapperVersion: null,
      pythonExecutable: null,
      pythonEnvironment: null,
    },
    project: null,
    projects: [],
    deletedProjects: [],
    busy: false,
    error: null,
  });
  window.requestAnimationFrame = (f: FrameRequestCallback) =>
    window.setTimeout(() => f(0), 0);
});
afterEach(cleanup);
describe("environment UI", () => {
  it("exposes top-level menu without selecting a character and can switch back", async () => {
    render(<App />);
    const nav = screen.getByRole("navigation", { name: "Main workspaces" });
    fireEvent.click(
      within(nav).getByRole("button", { name: "Environment Assets" }),
    );
    expect(
      within(nav)
        .getByRole("button", { name: "Environment Assets" })
        .getAttribute("aria-current"),
    ).toBe("page");
    expect(
      screen.getByRole("button", { name: "New environment project" }),
    ).toBeTruthy();
    expect(useStudio.getState().project).toBeNull();
    fireEvent.click(
      within(nav).getByRole("button", { name: "Character Animations" }),
    );
    expect(
      within(nav)
        .getByRole("button", { name: "Character Animations" })
        .getAttribute("aria-current"),
    ).toBe("page");
    expect(useStudio.getState().project).toBeNull();
  });
  it("opens a package through the environment-specific project picker", async () => {
    render(<EnvironmentAssets workspace="E:/synthetic" />);
    await screen.findByRole("option", { name: "World only" });
    fireEvent.change(screen.getByLabelText("Environment project"), {
      target: { value: "world" },
    });
    fireEvent.change(screen.getByLabelText("Package"), {
      target: { value: "package-test" },
    });
    await screen.findByRole("heading", { name: "Custom sunset" });
    expect(mock.command).toHaveBeenCalledWith(
      "world",
      { operation: "load" },
      "package-test",
    );
  });
  it("shows library details, filters and uses exact package geometry in editor", async () => {
    render(
      <EnvironmentWorkspace
        projectId="world"
        packageId="package-test"
        onChange={() => {}}
      />,
    );
    await screen.findByRole("heading", { name: "Custom sunset" });
    fireEvent.click(
      screen.getByRole("button", { name: "Asset library", exact: true }),
    );
    expect(screen.getByRole("button", { name: /Sky layer/ })).toBeTruthy();
    fireEvent.change(screen.getByPlaceholderText("Name or asset ID"), {
      target: { value: "not found" },
    });
    expect(screen.queryByRole("button", { name: /Sky layer/ })).toBeNull();
    fireEvent.click(
      screen.getByRole("button", { name: "Asset editor", exact: true }),
    );
    expect(screen.getByText("Canvas operation → 8 × 6")).toBeTruthy();
    expect(
      screen.getByRole("img", { name: "Original · unchanged" }),
    ).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Import layer PNG" }));
    await waitFor(() => expect(mock.choosePNG).toHaveBeenCalled());
    expect(
      mock.command.mock.calls.filter((c) => c[1].operation === "import"),
    ).toHaveLength(0);
  });
  it("keeps visual approval separate and blocks unapproved package export", async () => {
    render(
      <EnvironmentWorkspace
        projectId="world"
        packageId="package-test"
        onChange={() => {}}
      />,
    );
    await screen.findByRole("heading", { name: "Custom sunset" });
    fireEvent.click(
      screen.getByRole("button", { name: "Export & validation" }),
    );
    expect(
      (
        screen.getByRole("button", {
          name: "Export selected approved package",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    fireEvent.click(
      screen.getByRole("button", { name: "Asset editor", exact: true }),
    );
    expect(
      (
        screen.getByRole("button", {
          name: "Approve processed revision",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
  });
  it("exports review compositor with explicit revision and opacity only", async () => {
    render(
      <EnvironmentWorkspace
        projectId="world"
        packageId="package-test"
        onChange={() => {}}
      />,
    );
    await screen.findByRole("heading", { name: "Custom sunset" });
    fireEvent.click(screen.getByRole("button", { name: "Layer compositor" }));
    fireEvent.change(screen.getByLabelText("Review opacity"), {
      target: { value: "0.5" },
    });
    fireEvent.click(
      screen.getByRole("button", { name: /Export review-only composite/ }),
    );
    await waitFor(() =>
      expect(mock.command).toHaveBeenCalledWith(
        "world",
        {
          operation: "review_composite",
          layers: [{ order: 0, revision: "rev", opacity: 0.5 }],
        },
        "package-test",
      ),
    );
  });
  it("keeps library thumbnails current while compositor reviews a historical revision", async () => {
    const state = fixture();
    state.layers[0].revisions.push({
      ...state.layers[0].revisions[0],
      id: "old",
    });
    const oldPreview = { ...preview, dataUrl: "data:image/png;base64,BB==" };
    mock.command.mockImplementation(
      async (_id: string, r: EnvironmentRequest) => ({
        environment: state,
        catalog,
        recoveryNotice: null,
        result: r.operation.includes("preview")
          ? r.operation === "revision_preview" && r.revision === "old"
            ? oldPreview
            : preview
          : null,
      }),
    );
    render(
      <EnvironmentWorkspace
        projectId="world"
        packageId="package-test"
        onChange={() => {}}
      />,
    );
    await screen.findByRole("heading", { name: "Custom sunset" });
    fireEvent.click(screen.getByRole("button", { name: "Layer compositor" }));
    fireEvent.change(screen.getByLabelText("Review revision for Sky layer"), {
      target: { value: "old" },
    });
    await waitFor(() =>
      expect(
        screen
          .getByRole("img", { name: "Sky layer", exact: true })
          .getAttribute("src"),
      ).toBe(oldPreview.dataUrl),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Asset library", exact: true }),
    );
    expect(
      screen
        .getByRole("img", { name: "Sky layer", exact: true })
        .getAttribute("src"),
    ).toBe(preview.dataUrl);
  });
});
describe("environment status model", () => {
  it("shows latest exports in cross-kind chronology, not grouped by asset", async () => {
    const state = fixture();
    state.exportHistory = [
      {
        path: "environments/package-test/exports/first",
        kind: "asset",
        exportedAt: "2026-10-11T10:00:00Z",
      },
      {
        path: "environments/package-test/package-exports/latest",
        kind: "package",
        exportedAt: "2026-10-11T11:00:00Z",
      },
    ];
    state.layers[0].exports.push(state.exportHistory[0].path);
    state.packageExports.push(state.exportHistory[1].path);
    mock.command.mockImplementation(async () => ({
      environment: state,
      catalog,
      result: null,
      recoveryNotice: null,
    }));
    render(
      <EnvironmentWorkspace
        projectId="world"
        packageId="package-test"
        onChange={() => {}}
      />,
    );
    await screen.findByRole("heading", { name: "Custom sunset" });
    expect(
      screen.getAllByRole("button", { name: "Open folder" })[0].parentElement
        ?.textContent,
    ).toContain("package-exports/latest");
  });
  it("keeps separators while typing multi-color custom palettes", () => {
    function PaletteForm() {
      const [p, setP] = useState(UNRESTRICTED_PALETTE);
      return <PaletteEditor value={p} change={setP} />;
    }
    render(<PaletteForm />);
    const field = screen.getByLabelText(
      "Colors · #RRGGBB separated by spaces",
    ) as HTMLTextAreaElement;
    fireEvent.change(field, { target: { value: "#000000 " } });
    expect(field.value).toBe("#000000 ");
    fireEvent.change(field, { target: { value: "#000000 #FFFFFF" } });
    expect(field.value).toBe("#000000 #FFFFFF");
  });
  it("distinguishes technically valid from artistically approved and excludes removed assets", () => {
    const s = fixture();
    expect(assetStatus(s.layers[0])).toBe("review");
    activeRevision(s.layers[0])!.approvedAt = "approved";
    expect(assetStatus(s.layers[0])).toBe("approved");
    expect(environmentCounts(s).approved).toBe(1);
    s.layers[0].removed = true;
    expect(environmentCounts(s).approved).toBe(0);
    expect(matchesEnvironmentAsset(s.layers[0], "", "", "")).toBe(false);
  });
  it("does not report stale output as current approval", () => {
    const s = fixture();
    s.layers[0].revisions[0].approvedAt = "previous approval";
    s.layers[0].currentRevisionId = null;
    expect(assetStatus(s.layers[0])).toBe("stale");
    expect(environmentCounts(s).approved).toBe(0);
    expect(
      matchesEnvironmentAsset(s.layers[0], "Sky", "background_layer", "stale"),
    ).toBe(true);
  });
});
