import type { Animation, AutoFitRun, BatchAutoFitRun, BatchSnapReview, CleanupReview, Facing, Project, SnapReview } from "../types";

export function latestSnapReview(project: Project, facing: Facing): SnapReview | null {
  const sourceId = project.anchors[facing]?.activeImportId;
  if (!sourceId) return null;
  const activeId = project.anchors[facing]?.activeSnapId;
  for (let index = project.snapReviews.length - 1; index >= 0; index--) {
    const review = project.snapReviews[index];
    if (review.facing === facing && review.sourceImportId === sourceId && (!review.autoFitRunId || review.id === activeId)) return review;
  }
  return null;
}

export function latestAutoFitRun(project: Project, facing: Facing): AutoFitRun | null {
  const sourceId = project.anchors[facing]?.activeImportId;
  if (!sourceId) return null;
  for (let index = project.autoFitRuns.length - 1; index >= 0; index--) {
    const run = project.autoFitRuns[index];
    if (run.facing === facing && run.sourceImportId === sourceId
      && run.cellWidth === project.runtime.cellWidth && run.cellHeight === project.runtime.cellHeight
      && run.pivotX === project.runtime.pivotX && run.pivotY === project.runtime.pivotY
      && run.neutralHeightTarget === project.runtime.neutralHeightTarget
      && run.neutralTolerancePx === project.runtime.neutralTolerancePx) return run;
  }
  return null;
}

export function latestBatchSnapReview(animation: Animation): BatchSnapReview | null {
  for (let index = animation.batchSnaps.length - 1; index >= 0; index--) {
    const review = animation.batchSnaps[index];
    if (review.extractionId === animation.activeExtractionId
      && review.sourceFrameIds.join() === animation.activeRawFrameIds.join()
      && (!review.autoFitRunId || review.id === animation.activeBatchSnapId)) return review;
  }
  return null;
}

export function latestBatchAutoFitRun(project: Project, animation: Animation): BatchAutoFitRun | null {
  for (let index = animation.batchAutoFitRuns.length - 1; index >= 0; index--) {
    const run = animation.batchAutoFitRuns[index];
    if (run.extractionId === animation.activeExtractionId
      && run.sourceFrameIds.join() === animation.activeRawFrameIds.join()
      && run.cellWidth === project.runtime.cellWidth && run.cellHeight === project.runtime.cellHeight
      && run.pivotX === project.runtime.pivotX && run.pivotY === project.runtime.pivotY
      && run.neutralHeightTarget === project.runtime.neutralHeightTarget
      && run.neutralTolerancePx === project.runtime.neutralTolerancePx) return run;
  }
  return null;
}

export function latestCleanupReview(project: Project, facing: Facing): CleanupReview | null {
  const sourceId = project.anchors[facing]?.activeImportId;
  const snapId = project.anchors[facing]?.activeSnapId;
  if (!sourceId || !snapId) return null;
  const stage = project.imports.find(source => source.id === sourceId)?.stages.cleanup;
  if (stage === "review") {
    for (let index = project.cleanupReviews.length - 1; index >= 0; index--) {
      const review = project.cleanupReviews[index];
      if (review.facing === facing && review.sourceImportId === sourceId && review.sourceSnapId === snapId) return review;
    }
  }
  const activeId = project.anchors[facing]?.activeCleanupId;
  return project.cleanupReviews.find(review => review.id === activeId && review.sourceImportId === sourceId && review.sourceSnapId === snapId) ?? null;
}
