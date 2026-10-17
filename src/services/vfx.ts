import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { VfxRequest } from "../vfxTypes";
import { traceVfx } from "../domain/vfxDiagnostics";
let requestId = 0;
export const vfxDesktop = {
  command: async <T>(
    request: VfxRequest,
    projectId?: string,
    effectId?: string,
  ) => {
    const event = {
      requestId: ++requestId,
      operation: request.operation,
      projectId,
      effectId,
    };
    traceVfx({ kind: "command-start", ...event });
    try {
      const result = await invoke<T>("vfx_command", {
        projectId: projectId ?? null,
        effectId: effectId ?? null,
        request,
      });
      traceVfx({ kind: "command-ok", ...event });
      return result;
    } catch (error) {
      traceVfx({ kind: "command-failed", ...event });
      throw error;
    }
  },
  choosePNG: (multiple: boolean) =>
    open({
      multiple,
      directory: false,
      title: "Import VFX PNG · original bytes preserved",
      filters: [{ name: "PNG", extensions: ["png"] }],
    }),
};
