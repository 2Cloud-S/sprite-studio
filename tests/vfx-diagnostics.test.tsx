import { afterEach, expect, it, vi } from "vitest";
import { vfxDesktop } from "../src/services/vfx";
import { traceVfx } from "../src/domain/vfxDiagnostics";
const mock = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mock.invoke }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
afterEach(() => {
  delete globalThis.__SPRITE_STUDIO_VFX_TRACE__;
  mock.invoke.mockReset();
});
it("traces command completion/failure without recording request payloads", async () => {
  globalThis.__SPRITE_STUDIO_VFX_TRACE__ = [];
  mock.invoke
    .mockResolvedValueOnce({ ok: true })
    .mockRejectedValueOnce(new Error("failed"));
  await vfxDesktop.command(
    {
      operation: "approve_import",
      import_id: "source",
      reviewer: "private name",
    },
    "p",
    "e",
  );
  await expect(
    vfxDesktop.command({ operation: "load" }, "p", "e"),
  ).rejects.toThrow("failed");
  expect(globalThis.__SPRITE_STUDIO_VFX_TRACE__.map((e) => e.kind)).toEqual([
    "command-start",
    "command-ok",
    "command-start",
    "command-failed",
  ]);
  expect(JSON.stringify(globalThis.__SPRITE_STUDIO_VFX_TRACE__)).not.toContain(
    "private name",
  );
});
it("is opt-in and bounded instead of adding a polling or persistence loop", () => {
  traceVfx({ kind: "editor-state", operation: "idle" });
  expect(globalThis.__SPRITE_STUDIO_VFX_TRACE__).toBeUndefined();
  globalThis.__SPRITE_STUDIO_VFX_TRACE__ = [];
  for (let i = 0; i < 500; i++)
    traceVfx({ kind: "editor-state", requestId: i });
  expect(globalThis.__SPRITE_STUDIO_VFX_TRACE__).toHaveLength(300);
  expect(globalThis.__SPRITE_STUDIO_VFX_TRACE__[0].requestId).toBe(200);
});
