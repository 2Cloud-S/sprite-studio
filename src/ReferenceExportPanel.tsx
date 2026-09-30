import { Download, FolderOpen } from "lucide-react";
import { useStudio } from "./store/studio";
import { readablePath } from "./domain/paths";
import type { Facing, Project } from "./types";

export function ReferenceExportPanel({ project, facing }: { project: Project; facing: Facing }) {
  const { busy, referenceExportFolder, exportReference, openReferenceExportFolder } = useStudio();
  const ready = !!project.anchors[facing]?.activeCleanupId;

  return <section className="reference-export-panel" aria-labelledby="reference-export-title">
    <div>
      <span className="eyebrow">FINAL STEP · LOCAL EXPORT</span>
      <h2 id="reference-export-title">Export this reference</h2>
      <p>Save the applied, fixed-cell PNG with its cutout, snapped grid, nearest-neighbour reference, and draft manifest. This does not approve production art.</p>
    </div>
    {!ready && <p className="export-rule">Apply cleanup above before exporting this facing.</p>}
    <div className="reference-export-actions">
      <button className="button primary" disabled={!ready || busy} onClick={() => void exportReference()}><Download size={16} /> {referenceExportFolder ? "Export again" : "Export reference"}</button>
      {referenceExportFolder && <button className="button secondary" disabled={busy} onClick={() => void openReferenceExportFolder()}><FolderOpen size={16} /> Open export folder</button>}
    </div>
    {referenceExportFolder && <div className="export-location"><span>Saved locally</span><code title={readablePath(referenceExportFolder)}>{readablePath(referenceExportFolder)}</code></div>}
  </section>;
}
