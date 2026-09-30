import type { ReactNode } from "react";

export function WorkflowTip({ label, children }: { label: string; children: ReactNode }) {
  return <div className="workflow-tip" role="note"><strong>{label}</strong><span>{children}</span></div>;
}
