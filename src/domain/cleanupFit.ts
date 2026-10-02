export interface CleanupFitDiagnostic {
  foregroundWidth: number;
  foregroundHeight: number;
  cellWidth: number;
  cellHeight: number;
  pivotX: number;
  pivotY: number;
  suggested: { cellWidth: number; cellHeight: number; pivotX: number; pivotY: number } | null;
}

const marker = "SPRITE_STUDIO_FIT:";

function wholePixel(value: unknown, maximum: number): value is number {
  return Number.isInteger(value) && Number(value) >= 0 && Number(value) <= maximum;
}

export function parseCleanupFit(error: unknown): CleanupFitDiagnostic | null {
  const raw = error instanceof Error ? error.message : String(error);
  const start = raw.indexOf(marker);
  if (start < 0) return null;
  try {
    const line = raw.slice(start + marker.length).split(/\r?\n/, 1)[0];
    const value: unknown = JSON.parse(line);
    if (!value || typeof value !== "object") return null;
    const item = value as Record<string, unknown>;
    for (const key of ["foregroundWidth", "foregroundHeight", "cellWidth", "cellHeight", "pivotX", "pivotY"]) {
      if (!wholePixel(item[key], 2048)) return null;
    }
    let suggested: CleanupFitDiagnostic["suggested"] = null;
    if (item.suggested && typeof item.suggested === "object") {
      const candidate = item.suggested as Record<string, unknown>;
      if (["cellWidth", "cellHeight", "pivotX", "pivotY"].every(key => wholePixel(candidate[key], 1024))) {
        suggested = candidate as CleanupFitDiagnostic["suggested"];
      }
    }
    return { ...item, suggested } as unknown as CleanupFitDiagnostic;
  } catch {
    return null;
  }
}

export function cleanupFitMessage(fit: CleanupFitDiagnostic, locked = false): string {
  const measured = `Foreground ${fit.foregroundWidth}×${fit.foregroundHeight} px cannot fit a ${fit.cellWidth}×${fit.cellHeight} cell at pivot (${fit.pivotX}, ${fit.pivotY}).`;
  const alternatives = locked
    ? " Auto-fit can retry coarser snap grids without changing the locked geometry; review a candidate before Apply."
    : fit.suggested
    ? ` For an editable project, expand to ${fit.suggested.cellWidth}×${fit.suggested.cellHeight} with pivot (${fit.suggested.pivotX}, ${fit.suggested.pivotY}); to keep the current cell, resnap at a coarser pixel size and review the result.`
    : " Keep this contract by resnapping at a coarser pixel size or revising the source.";
  return `${measured}${alternatives} Nothing was scaled or clipped.`;
}
