import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  cleanup,
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import App from "../src/App";
import { VfxPlayer, VfxWorkspace } from "../src/VfxWorkspace";
import {
  advanceVfxClock,
  defaultProcessing,
  frameAtTick,
  lightStudy,
  vfxValueKey,
  mirroredPixelX,
} from "../src/domain/vfx";
import type { VfxEffect, VfxReply, VfxRequest } from "../src/vfxTypes";
import { useStudio } from "../src/store/studio";
import { StrictMode } from "react";
const api = vi.hoisted(() => ({ command: vi.fn(), choosePNG: vi.fn() }));
vi.mock("../src/services/vfx", () => ({ vfxDesktop: api }));
vi.mock("../src/services/environment", () => ({
  environmentDesktop: {
    library: vi.fn(async () => ({ owners: [], failures: [] })),
    command: vi.fn(),
    choosePNG: vi.fn(),
  },
}));
vi.mock("../src/AnimationWorkspace", () => ({
  AnimationWorkspace: () => <div>Character animations retained</div>,
}));
vi.mock("../src/SnapStage", () => ({ SnapStage: () => null }));
vi.mock("../src/CleanupStage", () => ({ CleanupStage: () => null }));
vi.mock("../src/ReferenceExportPanel", () => ({
  ReferenceExportPanel: () => null,
}));
const image = {
  dataUrl: "data:image/png;base64,AA==",
  width: 24,
  height: 24,
  mimeType: "image/png",
};
const lib = {
  projects: [
    {
      id: "world",
      name: "VFX collection",
      effects: [
        {
          id: "effect-one",
          name: "Light Contact",
          effectId: "effect.contact.light",
          package: "Study",
          status: "draft",
          recoveryNotice: null,
        },
      ],
    },
  ],
  failures: [],
};
function fixture(approved = false): VfxReply {
  const profile = lightStudy();
  const effect: VfxEffect = {
    schemaVersion: 1,
    id: "effect-one",
    projectId: "world",
    effectId: "effect.contact.light",
    name: "Light Contact",
    package: "Study",
    draftProfile: profile,
    sources: [],
    imports: [
      {
        id: "import-one",
        regions: [0, 1, 2].map(() => ({
          sourceId: "raw-one",
          rect: [0, 0, 24, 24],
        })),
        approvedBy: "Owner",
        approvedAt: "date",
        createdAt: "date",
      },
    ],
    pendingImportId: null,
    activeImportId: "import-one",
    activeRevisionId: "revision-one",
    generation: 3,
    exports: [],
    revisions: [
      {
        id: "revision-one",
        importId: "import-one",
        profile,
        createdAt: "date",
        approval: approved
          ? {
              reviewer: "Owner",
              approvedAt: "date",
              revisionHash: "hash",
              checks: {
                artwork: true,
                regionsAndAnchor: true,
                palette: true,
                timing: true,
                hitStopAndCompletion: true,
              },
            }
          : null,
        frames: [2, 3, 3].map((h, i) => ({
          id: `frame-${i}`,
          sourceId: "raw-one",
          rect: [0, 0, 24, 24],
          path: `frame-${i}.png`,
          sha256: `hash-${i}`,
          processing: defaultProcessing(),
          holdTicks: h,
          report: {
            size: [24, 24],
            opaquePixels: 3,
            partialAlphaPixels: 0,
            transparentRgbPixels: 0,
            unexpectedColors: [],
            clippedPixels: 0,
            edgePixels: 0,
            possibleGreenFringePixels: 0,
            bounds: [11, 11, 2, 2],
            errors: [],
            warnings: [],
          },
        })),
      },
    ],
  };
  return {
    effect,
    recoveryNotice: null,
    crops: [image, image, image],
    originals: [],
    frames: [image, image, image],
    result: null,
  };
}
beforeEach(() => {
  HTMLElement.prototype.scrollIntoView = vi.fn();
  localStorage.clear();
  api.command.mockReset();
  api.choosePNG.mockReset();
  api.choosePNG.mockResolvedValue(null);
  api.command.mockImplementation(async (r: VfxRequest) =>
    r.operation === "library"
      ? lib
      : r.operation === "create_project"
        ? { library: lib, createdProjectId: "world" }
        : r.operation === "save_draft"
          ? { ...fixture(), effect: { ...fixture().effect, draft: r.draft } }
          : fixture(),
  );
  useStudio.setState({
    boot: vi.fn(async () => {}),
    settings: {
      workspaceRoot: "E:/test",
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
  window.requestAnimationFrame = (f) => window.setTimeout(() => f(0), 0);
});
afterEach(cleanup);
async function openEffect() {
  render(<VfxWorkspace workspace="E:/test" />);
  await screen.findByRole("option", { name: "VFX collection" });
  fireEvent.change(screen.getByLabelText("VFX project"), {
    target: { value: "world" },
  });
  fireEvent.click(await screen.findByRole("button", { name: /Light Contact/ }));
  await screen.findByRole("heading", { name: "Light Contact", exact: true });
  await screen.findByRole("button", { name: "Approve these frame regions" });
}
const wireValue = <T,>(value: T): T =>
  JSON.parse(
    JSON.stringify(value, (_key, v) =>
      v && typeof v === "object" && !Array.isArray(v)
        ? Object.fromEntries(
            Object.entries(v).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)),
          )
        : v,
    ),
  );
const operationCalls = (operation: string) =>
  api.command.mock.calls.filter(([r]) => r.operation === operation);
const deferred = <T,>() => {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
};
describe("dedicated VFX workspace", () => {
  it("reopens an approved effect with a sorted saved draft without one idle command", async () => {
    const saved = fixture(true);
    saved.effect.draft = {
      profile: saved.effect.revisions[0].profile,
      settings: saved.effect.revisions[0].frames.map((f) => f.processing),
      holds: [2, 3, 3],
      baseRevisionId: "revision-one",
      importId: "import-one",
    };
    api.command.mockImplementation(async (r: VfxRequest) =>
      r.operation === "library" ? lib : wireValue(saved),
    );
    await openEffect();
    expect(screen.getByText("OWNER APPROVED")).toBeTruthy();
    const calls = api.command.mock.calls.length;
    vi.useFakeTimers();
    try {
      await act(async () => {
        await vi.advanceTimersByTimeAsync(30000);
      });
      expect(api.command.mock.calls).toHaveLength(calls);
      expect(operationCalls("save_draft")).toHaveLength(0);
      expect(screen.getByText("OWNER APPROVED")).toBeTruthy();
      expect(
        document.querySelector(".vfx-editor")?.getAttribute("data-operation"),
      ).toBe("idle");
    } finally {
      vi.useRealTimers();
    }
  });
  it("imports a three-pose board, approves its regions once, and keeps every region", async () => {
    let saved = wireValue(fixture());
    api.command.mockImplementation(async (r: VfxRequest) => {
      if (r.operation === "library") return lib;
      if (r.operation === "import_frames") {
        saved = fixture();
        saved.effect.revisions = [];
        saved.effect.activeRevisionId = null;
        saved.effect.activeImportId = null;
        saved.effect.pendingImportId = "import-one";
        saved.effect.imports[0].approvedBy = null;
        saved.effect.imports[0].approvedAt = null;
        saved.effect.imports[0].regions = [0, 1, 2].map((i) => ({
          sourceId: "raw-one",
          rect: [i * 724, 0, 724, 724],
        }));
      }
      if (r.operation === "approve_import") {
        saved.effect.pendingImportId = null;
        saved.effect.activeImportId = "import-one";
        saved.effect.imports[0].approvedBy = "Owner";
        saved.effect.imports[0].approvedAt = "date";
      }
      if (r.operation === "save_draft") saved.effect.draft = r.draft;
      return wireValue(saved);
    });
    await openEffect();
    fireEvent.change(screen.getByLabelText("Import kind"), {
      target: { value: "board" },
    });
    api.choosePNG.mockResolvedValue("three-pose-board.png");
    fireEvent.click(
      screen.getByRole("button", { name: "Import VFX PNG board" }),
    );
    await screen.findByRole("heading", {
      name: "Approve detected frame regions",
    });
    expect(operationCalls("import_frames")[0][0].board_count).toBe(3);
    fireEvent.change(screen.getByLabelText("Region reviewer"), {
      target: { value: "Owner" },
    });
    fireEvent.click(
      screen.getByRole("button", { name: "Approve these frame regions" }),
    );
    await screen.findByRole("heading", { name: "Approved frame regions" });
    expect(operationCalls("approve_import")).toHaveLength(1);
    for (let i = 1; i <= 3; i++)
      expect(
        (screen.getByLabelText(`Frame ${i} region Width`) as HTMLInputElement)
          .value,
      ).toBe("724");
    expect(
      (
        screen.getByRole("button", {
          name: "Approve these frame regions",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
  });
  it("serializes process behind draft persistence without accepting an older snapshot afterward", async () => {
    const save = deferred<VfxReply>();
    api.command.mockImplementation(async (r: VfxRequest) => {
      if (r.operation === "library") return lib;
      if (r.operation === "save_draft") return save.promise;
      if (r.operation === "process") {
        const newer = fixture();
        newer.effect.generation = 5;
        newer.effect.activeRevisionId = "new-revision";
        newer.effect.revisions[0].id = "new-revision";
        newer.effect.revisions[0].profile = r.profile;
        return wireValue(newer);
      }
      return wireValue(fixture());
    });
    await openEffect();
    fireEvent.click(
      screen.getByRole("button", { name: "Process & align", exact: true }),
    );
    fireEvent.change(screen.getByLabelText("Contact anchor X"), {
      target: { value: "10" },
    });
    await waitFor(() => expect(operationCalls("save_draft")).toHaveLength(1));
    fireEvent.click(
      screen.getByRole("button", {
        name: "Create non-destructive processing preview",
      }),
    );
    expect(operationCalls("process")).toHaveLength(0);
    const draft = operationCalls("save_draft")[0][0].draft;
    await act(async () =>
      save.resolve(
        wireValue({
          ...fixture(),
          effect: { ...fixture().effect, draft, generation: 4 },
        }),
      ),
    );
    await waitFor(() => expect(operationCalls("process")).toHaveLength(1));
    await waitFor(() =>
      expect(
        document.querySelector(".vfx-editor")?.getAttribute("data-operation"),
      ).toBe("idle"),
    );
    expect(screen.getByText("new-revision")).toBeTruthy();
    expect(
      (screen.getByLabelText("Contact anchor X") as HTMLInputElement).value,
    ).toBe("10");
  });
  it("ignores the older StrictMode opening response without saving or resetting approval", async () => {
    const old = deferred<VfxReply>();
    let loads = 0;
    localStorage.setItem("sprite-studio:vfx:E:/test:project", "world");
    localStorage.setItem("sprite-studio:vfx:E:/test:effect", "effect-one");
    api.command.mockImplementation(async (r: VfxRequest) => {
      if (r.operation === "library") return lib;
      if (r.operation === "load" && ++loads === 1) return old.promise;
      return wireValue(fixture(true));
    });
    render(
      <StrictMode>
        <VfxWorkspace workspace="E:/test" />
      </StrictMode>,
    );
    await screen.findByText("OWNER APPROVED");
    await act(async () => old.resolve(wireValue(fixture(false))));
    expect(screen.getByText("OWNER APPROVED")).toBeTruthy();
    expect(operationCalls("save_draft")).toHaveLength(0);
    expect(operationCalls("approve")).toHaveLength(0);
  });
  it("retains edits made during a draft save and never disables the editor for that save", async () => {
    const first = deferred<VfxReply>();
    api.command.mockImplementation(async (r: VfxRequest) => {
      if (r.operation === "library") return lib;
      if (r.operation === "save_draft") {
        if (operationCalls("save_draft").length === 1) return first.promise;
        return wireValue({
          ...fixture(),
          effect: { ...fixture().effect, draft: r.draft },
        });
      }
      return wireValue(fixture());
    });
    await openEffect();
    fireEvent.click(
      screen.getByRole("button", { name: "Timing & preview", exact: true }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Step one tick" }));
    fireEvent.change(screen.getByLabelText("Contact anchor X"), {
      target: { value: "10" },
    });
    await waitFor(() => expect(operationCalls("save_draft")).toHaveLength(1));
    expect(
      (document.querySelector(".vfx-editor-body") as HTMLFieldSetElement)
        .disabled,
    ).toBe(false);
    fireEvent.change(screen.getByLabelText("Contact anchor X"), {
      target: { value: "9" },
    });
    const request = operationCalls("save_draft")[0][0];
    await act(async () =>
      first.resolve(
        wireValue({
          ...fixture(),
          effect: { ...fixture().effect, draft: request.draft },
        }),
      ),
    );
    expect(
      (screen.getByLabelText("Contact anchor X") as HTMLInputElement).value,
    ).toBe("9");
    await waitFor(() => expect(operationCalls("save_draft")).toHaveLength(2));
    expect(operationCalls("save_draft")[1][0].draft.profile.anchor[0]).toBe(9);
    expect(screen.getByRole("status").textContent).toContain("Tick 1");
  });
  it("approves a processed revision once and keeps export/approval stable after read-only refresh", async () => {
    let saved = wireValue(fixture());
    const approve = deferred<VfxReply>();
    api.command.mockImplementation(async (r: VfxRequest) => {
      if (r.operation === "library") return lib;
      if (r.operation === "approve") return approve.promise;
      return saved;
    });
    await openEffect();
    fireEvent.click(
      screen.getByRole("button", { name: "Review & exports", exact: true }),
    );
    fireEvent.change(screen.getByLabelText("Owner / reviewer"), {
      target: { value: "Owner" },
    });
    for (const c of document.querySelectorAll(".vfx-check input"))
      fireEvent.click(c);
    const action = screen.getByRole("button", {
      name: "Approve exact VFX revision",
    });
    fireEvent.click(action);
    fireEvent.click(action);
    await waitFor(() => expect(operationCalls("approve")).toHaveLength(1));
    saved = wireValue(fixture(true));
    await act(async () => approve.resolve(saved));
    await waitFor(() =>
      expect(
        (
          screen.getByRole("button", {
            name: "Export approved VFX",
          }) as HTMLButtonElement
        ).disabled,
      ).toBe(false),
    );
    expect((action as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByText("OWNER APPROVED")).toBeTruthy();
    const calls = api.command.mock.calls.length;
    vi.useFakeTimers();
    try {
      await act(async () => {
        await vi.advanceTimersByTimeAsync(30000);
      });
      expect(api.command.mock.calls).toHaveLength(calls);
      expect(
        document.querySelector(".vfx-editor")?.getAttribute("data-operation"),
      ).toBe("idle");
    } finally {
      vi.useRealTimers();
    }
    cleanup();
    localStorage.clear();
    await openEffect();
    expect(screen.getByText("OWNER APPROVED")).toBeTruthy();
    expect(operationCalls("approve")).toHaveLength(1);
    expect(operationCalls("save_draft")).toHaveLength(0);
  });
  it("ignores an obsolete load when rapidly switching effects", async () => {
    const oldLoad = deferred<VfxReply>();
    const library = structuredClone(lib);
    library.projects[0].effects.push({
      ...lib.projects[0].effects[0],
      id: "effect-two",
      name: "Second effect",
    });
    api.command.mockImplementation(
      async (r: VfxRequest, _p: string, e: string) => {
        if (r.operation === "library") return library;
        if (r.operation === "load" && e === "effect-one")
          return oldLoad.promise;
        const second = fixture(true);
        second.effect.id = "effect-two";
        second.effect.name = "Second effect";
        return wireValue(second);
      },
    );
    render(<VfxWorkspace workspace="E:/test" />);
    await screen.findByRole("option", { name: "VFX collection" });
    fireEvent.change(screen.getByLabelText("VFX project"), {
      target: { value: "world" },
    });
    fireEvent.click(screen.getByRole("button", { name: /Light Contact/ }));
    fireEvent.click(screen.getByRole("button", { name: /Second effect/ }));
    await screen.findByRole("heading", { name: "Second effect", exact: true });
    await act(async () => oldLoad.resolve(wireValue(fixture())));
    expect(
      screen.queryByRole("heading", { name: "Light Contact", exact: true }),
    ).toBeNull();
    expect(screen.getByText("OWNER APPROVED")).toBeTruthy();
    expect(operationCalls("save_draft")).toHaveLength(0);
  });
  it("retains failed-save edits and recovers with a single explicit retry", async () => {
    let fail = true;
    api.command.mockImplementation(async (r: VfxRequest) => {
      if (r.operation === "library") return lib;
      if (r.operation === "save_draft") {
        if (fail) throw new Error("disk unavailable");
        return wireValue({
          ...fixture(),
          effect: { ...fixture().effect, draft: r.draft },
        });
      }
      return wireValue(fixture());
    });
    await openEffect();
    fireEvent.click(
      screen.getByRole("button", { name: "Timing & preview", exact: true }),
    );
    fireEvent.change(screen.getByLabelText("Frame 2 hold ticks"), {
      target: { value: "7" },
    });
    await screen.findByRole("button", { name: "Retry draft save" });
    expect(
      (screen.getByLabelText("Frame 2 hold ticks") as HTMLInputElement).value,
    ).toBe("7");
    expect(operationCalls("save_draft")).toHaveLength(1);
    expect(
      (document.querySelector(".vfx-editor-body") as HTMLFieldSetElement)
        .disabled,
    ).toBe(false);
    fail = false;
    fireEvent.click(screen.getByRole("button", { name: "Retry draft save" }));
    await waitFor(() =>
      expect(
        screen.queryByRole("button", { name: "Retry draft save" }),
      ).toBeNull(),
    );
    expect(operationCalls("save_draft")).toHaveLength(2);
  });
  it("stays idle for 30 seconds after region approval with real Rust JSON ordering", async () => {
    // serde_json::Value objects use sorted keys, unlike our former JS-only mocks.
    const wire = <T,>(value: T): T =>
      JSON.parse(
        JSON.stringify(value, (_key, v) =>
          v && typeof v === "object" && !Array.isArray(v)
            ? Object.fromEntries(
                Object.entries(v).sort(([a], [b]) => a.localeCompare(b)),
              )
            : v,
        ),
      );
    let saved = fixture();
    saved.effect.revisions = [];
    saved.effect.activeRevisionId = null;
    saved.effect.activeImportId = null;
    saved.effect.pendingImportId = "import-one";
    saved.effect.imports[0].approvedAt = null;
    saved.effect.imports[0].approvedBy = null;
    const finishSaves: (() => void)[] = [];
    api.command.mockImplementation(async (r: VfxRequest) => {
      if (r.operation === "library") return wire(lib);
      if (r.operation === "approve_import") {
        saved.effect.activeImportId = "import-one";
        saved.effect.pendingImportId = null;
        saved.effect.imports[0].approvedBy = "Owner";
        saved.effect.imports[0].approvedAt = "date";
      }
      if (r.operation === "save_draft") {
        saved.effect.draft = r.draft;
        return new Promise((resolve) =>
          finishSaves.push(() => resolve(wire(saved))),
        );
      }
      return wire(saved);
    });
    await openEffect();
    const fieldset = document.querySelector(
      ".vfx-editor-body",
    ) as HTMLFieldSetElement;
    const disabledTransitions: boolean[] = [];
    const observer = new MutationObserver((records) => {
      if (records.some((r) => r.attributeName === "disabled"))
        disabledTransitions.push(fieldset.disabled);
    });
    observer.observe(fieldset, { attributes: true });
    vi.useFakeTimers();
    try {
      fireEvent.change(screen.getByLabelText("Region reviewer"), {
        target: { value: "Owner" },
      });
      fireEvent.click(
        screen.getByRole("button", { name: "Approve these frame regions" }),
      );
      for (let i = 0; i < 100; i++) {
        await act(async () => {
          await vi.advanceTimersByTimeAsync(300);
        });
        expect(
          fieldset.disabled,
          "background autosave must not disable foreground controls",
        ).toBe(false);
        await act(async () => {
          finishSaves.shift()?.();
        });
      }
      const saves = api.command.mock.calls.filter(
        ([r]) => r.operation === "save_draft",
      );
      expect(
        saves.length,
        `disabled transitions: ${disabledTransitions.join(",")}`,
      ).toBeLessThanOrEqual(1);
      expect(
        api.command.mock.calls.filter(
          ([r]) => r.operation === "approve_import",
        ),
      ).toHaveLength(1);
      expect(disabledTransitions.length).toBeLessThanOrEqual(4);
      expect(fieldset.disabled).toBe(false);
      expect(
        screen.getByRole("heading", { name: "Approved frame regions" }),
      ).toBeTruthy();
    } finally {
      observer.disconnect();
      vi.useRealTimers();
    }
  });
  it("navigates independently and preserves existing character/environment destinations", async () => {
    render(<App />);
    const nav = screen.getByRole("navigation", { name: "Main workspaces" });
    fireEvent.click(
      within(nav).getByRole("button", { name: "Visual Effects" }),
    );
    expect(
      within(nav)
        .getByRole("button", { name: "Visual Effects" })
        .getAttribute("aria-current"),
    ).toBe("page");
    expect(
      screen.getByRole("heading", { name: "Keep every spark." }),
    ).toBeTruthy();
    expect(useStudio.getState().project).toBeNull();
    fireEvent.click(
      within(nav).getByRole("button", { name: "Environment Assets" }),
    );
    expect(
      screen.getByRole("button", { name: "New environment project" }),
    ).toBeTruthy();
    fireEvent.click(
      within(nav).getByRole("button", { name: "Character Animations" }),
    );
    expect(
      within(nav)
        .getByRole("button", { name: "Character Animations" })
        .getAttribute("aria-current"),
    ).toBe("page");
  });
  it("creates game-independent proposed VFX without automatic approval", async () => {
    render(<VfxWorkspace workspace="E:/test" />);
    await screen.findByRole("option", { name: "VFX collection" });
    fireEvent.change(screen.getByLabelText("VFX project"), {
      target: { value: "world" },
    });
    fireEvent.click(screen.getByRole("button", { name: "New effect" }));
    fireEvent.change(screen.getByLabelText("Starting profile"), {
      target: { value: "light" },
    });
    fireEvent.click(
      screen.getByRole("button", { name: "Create draft effect" }),
    );
    await waitFor(() =>
      expect(api.command).toHaveBeenCalledWith(
        expect.objectContaining({
          operation: "create_effect",
          effect_id: "effect.contact.light",
          profile: expect.objectContaining({
            cell: [24, 24],
            anchor: [12, 12],
            palette: expect.objectContaining({ authorityId: null }),
          }),
        }),
        "world",
      ),
    );
    expect(
      api.command.mock.calls.some(([r]) => r.operation === "approve"),
    ).toBe(false);
  });
  it("imports ordered frames through VFX and cancellation makes no mutation", async () => {
    await openEffect();
    api.command.mockClear();
    fireEvent.click(
      screen.getByRole("button", { name: "Import VFX PNG frames" }),
    );
    await waitFor(() => expect(api.choosePNG).toHaveBeenCalledWith(true));
    expect(api.command).not.toHaveBeenCalled();
    api.choosePNG.mockResolvedValue(["c.png", "a.png", "b.png"]);
    fireEvent.click(
      screen.getByRole("button", { name: "Import VFX PNG frames" }),
    );
    await waitFor(() =>
      expect(api.command).toHaveBeenCalledWith(
        {
          operation: "import_frames",
          paths: ["c.png", "a.png", "b.png"],
          board_count: null,
          rectangles: null,
        },
        "world",
        "effect-one",
      ),
    );
  });
  it("sends explicit sheet rectangles without component detection", async () => {
    await openEffect();
    fireEvent.change(screen.getByLabelText("Import kind"), {
      target: { value: "rectangles" },
    });
    api.choosePNG.mockResolvedValue("board.png");
    fireEvent.change(screen.getByLabelText(/Rectangles JSON/), {
      target: { value: "[[4,3,24,24],[30,3,24,24]]" },
    });
    fireEvent.click(
      screen.getByRole("button", { name: "Import VFX PNG board" }),
    );
    await waitFor(() =>
      expect(api.command).toHaveBeenCalledWith(
        {
          operation: "import_frames",
          paths: ["board.png"],
          board_count: null,
          rectangles: [
            [4, 3, 24, 24],
            [30, 3, 24, 24],
          ],
        },
        "world",
        "effect-one",
      ),
    );
  });
  it("requires exact region approval and named reviewer", async () => {
    const pending = fixture();
    pending.effect.pendingImportId = "pending-one";
    pending.effect.imports.push({
      ...pending.effect.imports[0],
      id: "pending-one",
      approvedBy: null,
      approvedAt: null,
    });
    api.command.mockImplementation(async (r: VfxRequest) =>
      r.operation === "library" ? lib : pending,
    );
    await openEffect();
    const approve = screen.getByRole("button", {
      name: "Approve these frame regions",
    }) as HTMLButtonElement;
    expect(approve.disabled).toBe(true);
    fireEvent.change(screen.getByLabelText("Region reviewer"), {
      target: { value: "Owner" },
    });
    fireEvent.click(approve);
    await waitFor(() =>
      expect(api.command).toHaveBeenCalledWith(
        {
          operation: "approve_import",
          import_id: "pending-one",
          reviewer: "Owner",
        },
        "world",
        "effect-one",
      ),
    );
  });
  it("retains RGB separators and unassigned palette authority", async () => {
    await openEffect();
    fireEvent.click(
      screen.getByRole("button", { name: "Process & align", exact: true }),
    );
    const field = screen.getByLabelText(
      /RGB colors · separate/,
    ) as HTMLTextAreaElement;
    fireEvent.change(field, { target: { value: "#FFF1D2 " } });
    expect(field.value).toBe("#FFF1D2 ");
    fireEvent.change(field, { target: { value: "#FFF1D2 #38344B" } });
    fireEvent.click(
      screen.getByRole("button", {
        name: "Create non-destructive processing preview",
      }),
    );
    await waitFor(() =>
      expect(api.command).toHaveBeenCalledWith(
        expect.objectContaining({
          operation: "process",
          profile: expect.objectContaining({
            palette: expect.objectContaining({
              colors: ["#FFF1D2", "#38344B"],
              authorityId: null,
            }),
          }),
          settings: expect.arrayContaining([
            expect.objectContaining({
              pixelPitch: 1,
              remapPalette: false,
              offset: [0, 0],
            }),
          ]),
        }),
        "world",
        "effect-one",
      ),
    );
  });
  it("keeps preview FPS separate from authoritative holds and saves clock policy explicitly", async () => {
    await openEffect();
    fireEvent.click(
      screen.getByRole("button", { name: "Timing & preview", exact: true }),
    );
    api.command.mockClear();
    fireEvent.change(screen.getByLabelText("Preview FPS · study only"), {
      target: { value: "12" },
    });
    expect(api.command).not.toHaveBeenCalled();
    expect(
      (screen.getByLabelText("Frame 2 hold ticks") as HTMLInputElement).value,
    ).toBe("3");
    fireEvent.change(screen.getByLabelText("Frame 2 hold ticks"), {
      target: { value: "5" },
    });
    fireEvent.click(
      screen.getByRole("button", {
        name: "Save timing/layout as new review revision",
      }),
    );
    await waitFor(() =>
      expect(api.command).toHaveBeenCalledWith(
        expect.objectContaining({
          operation: "process",
          holds: [2, 5, 3],
          profile: expect.objectContaining({
            hitStopPolicy: "pause",
            looping: false,
            tickHz: 60,
          }),
        }),
        "world",
        "effect-one",
      ),
    );
  });
  it("gates artwork/timing/palette approval and exports by exact revision", async () => {
    await openEffect();
    fireEvent.click(
      screen.getByRole("button", { name: "Review & exports", exact: true }),
    );
    const approve = screen.getByRole("button", {
      name: "Approve exact VFX revision",
    }) as HTMLButtonElement;
    expect(approve.disabled).toBe(true);
    expect(
      (
        screen.getByRole("button", {
          name: "Export approved VFX",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    fireEvent.change(screen.getByLabelText("Owner / reviewer"), {
      target: { value: "Owner" },
    });
    for (const c of within(
      screen.getByRole("region", { name: "VFX effect editor" }),
    ).queryAllByRole("checkbox")) {
      if ((c as HTMLInputElement).closest(".vfx-check")) fireEvent.click(c);
    }
    fireEvent.click(approve);
    await waitFor(() =>
      expect(api.command).toHaveBeenCalledWith(
        expect.objectContaining({
          operation: "approve",
          revision_id: "revision-one",
          reviewer: "Owner",
        }),
        "world",
        "effect-one",
      ),
    );
  });
  it("blocks export of unsaved edits while retaining the approved revision", async () => {
    api.command.mockImplementation(async (r: VfxRequest) =>
      r.operation === "library" ? lib : fixture(true),
    );
    await openEffect();
    fireEvent.click(
      screen.getByRole("button", { name: "Process & align", exact: true }),
    );
    fireEvent.change(screen.getByLabelText("Contact anchor X"), {
      target: { value: "10" },
    });
    fireEvent.click(
      screen.getByRole("button", { name: "Review & exports", exact: true }),
    );
    expect(
      (
        screen.getByRole("button", {
          name: "Export approved VFX",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    expect(screen.getByText(/Unsaved configuration/)).toBeTruthy();
  });
  it("saves draft edits independently and restores saved offsets and holds on reopen", async () => {
    await openEffect();
    fireEvent.click(
      screen.getByRole("button", { name: "Timing & preview", exact: true }),
    );
    fireEvent.change(screen.getByLabelText("Frame 2 hold ticks"), {
      target: { value: "7" },
    });
    await waitFor(() =>
      expect(api.command).toHaveBeenCalledWith(
        expect.objectContaining({
          operation: "save_draft",
          draft: expect.objectContaining({
            holds: [2, 7, 3],
            baseRevisionId: "revision-one",
            importId: "import-one",
          }),
        }),
        "world",
        "effect-one",
      ),
    );
    const saved = fixture();
    saved.effect.draft = {
      profile: lightStudy(),
      settings: [
        defaultProcessing(),
        { ...defaultProcessing(), offset: [-2, 1] },
        defaultProcessing(),
      ],
      holds: [2, 7, 3],
      baseRevisionId: "revision-one",
      importId: "import-one",
    };
    cleanup();
    api.command.mockImplementation(async (r: VfxRequest) =>
      r.operation === "library" ? lib : saved,
    );
    localStorage.clear();
    await openEffect();
    fireEvent.click(
      screen.getByRole("button", { name: "Timing & preview", exact: true }),
    );
    expect(
      (screen.getByLabelText("Frame 2 hold ticks") as HTMLInputElement).value,
    ).toBe("7");
    fireEvent.click(
      screen.getByRole("button", { name: "Process & align", exact: true }),
    );
    expect(
      (screen.getByLabelText("Frame 2 offset X") as HTMLInputElement).value,
    ).toBe("-2");
  });
});
describe("effect clock and anchor", () => {
  it("compares draft values independently of object-key order, not frame/hold order", () => {
    const value = {
      profile: lightStudy(),
      settings: [defaultProcessing()],
      holds: [2, 3, 3],
      baseRevisionId: "rev",
      importId: "imp",
    };
    expect(JSON.stringify(wireValue(value))).not.toBe(JSON.stringify(value));
    expect(vfxValueKey(wireValue(value))).toBe(vfxValueKey(value));
    expect(vfxValueKey({ ...value, holds: [3, 2, 3] })).not.toBe(
      vfxValueKey(value),
    );
  });
  it("uses half-open positive integer holds and hides a non-loop at eight advancing ticks", () => {
    const holds = [2, 3, 3];
    expect(
      [0, 1, 2, 4, 5, 7, 8].map((t) => frameAtTick(t, holds, false).index),
    ).toEqual([0, 0, 1, 1, 2, 2, null]);
    expect(frameAtTick(8, holds, false).completed).toBe(true);
    expect(frameAtTick(8, holds, true).index).toBe(0);
    expect(frameAtTick(8, holds, false, "empty_terminal").index).toBe(2);
    expect(frameAtTick(0, [0, 3], false).index).toBeNull();
  });
  it("hit stop and pause do not consume frame holds", () => {
    let t = 2;
    t = advanceVfxClock(t, 60, true, false, "pause");
    expect(t).toBe(2);
    expect(frameAtTick(t, [2, 3, 3], false).index).toBe(1);
    expect(advanceVfxClock(t, 1, false, false, "pause")).toBe(3);
    expect(advanceVfxClock(t, 1, true, false, "continue")).toBe(3);
    expect(advanceVfxClock(t, 60, false, true, "continue")).toBe(2);
  });
  it("mirrors whole pixel centers about the shared boundary anchor without pivot drift", () => {
    expect(mirroredPixelX(11, 12)).toBe(12);
    expect(mirroredPixelX(12, 12)).toBe(11);
    expect(mirroredPixelX(2, 12)).toBe(21);
    expect((11.5 + 12.5) / 2).toBe(12);
    expect(mirroredPixelX(mirroredPixelX(2, 7), 7)).toBe(2);
  });
  it("previews native 1x and frozen tick stepping without mutating export pixels", () => {
    render(
      <VfxPlayer
        images={[image.dataUrl, image.dataUrl, image.dataUrl]}
        holds={[2, 3, 3]}
        profile={lightStudy()}
      />,
    );
    fireEvent.change(screen.getByLabelText("Nearest zoom"), {
      target: { value: "1" },
    });
    expect(screen.getByTestId("vfx-cell").style.width).toBe("24px");
    fireEvent.click(screen.getByLabelText("Simulate hit stop"));
    fireEvent.click(screen.getByRole("button", { name: "Step one tick" }));
    expect(screen.getByRole("status").textContent).toContain("Tick 0");
    fireEvent.click(screen.getByLabelText("Mirror about anchor"));
    expect(screen.getByAltText("VFX frame 1").style.transformOrigin).toBe(
      "12px 12px",
    );
  });
});
