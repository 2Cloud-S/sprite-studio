// Opt-in, bounded, memory-only diagnostics; no artwork, payloads or file paths.
// Enable in DevTools with window.__SPRITE_STUDIO_VFX_TRACE__ = [].
export interface VfxTraceEvent {
  at: number;
  kind: "command-start" | "command-ok" | "command-failed" | "editor-state";
  requestId?: number;
  operation?: string;
  projectId?: string;
  effectId?: string;
  busy?: boolean;
  saving?: boolean;
  importId?: string | null;
  revisionId?: string | null;
  approved?: boolean;
  dirty?: boolean;
}
declare global {
  var __SPRITE_STUDIO_VFX_TRACE__: VfxTraceEvent[] | undefined;
}
export function traceVfx(event: Omit<VfxTraceEvent, "at">) {
  const buffer = globalThis.__SPRITE_STUDIO_VFX_TRACE__;
  if (!Array.isArray(buffer)) return;
  buffer.push({ at: Date.now(), ...event });
  if (buffer.length > 300) buffer.splice(0, buffer.length - 300);
}
