import type { CleanupReview, Facing, Project, SnapReview } from "../types";

export function latestSnapReview(project: Project, facing: Facing): SnapReview | null {
  const sourceId = project.anchors[facing]?.activeImportId;
  if (!sourceId) return null;
  for (let index = project.snapReviews.length - 1; index >= 0; index--) {
    const review = project.snapReviews[index];
    if (review.facing === facing && review.sourceImportId === sourceId) return review;
  }
  return null;
}

export function latestCleanupReview(project: Project, facing: Facing): CleanupReview | null {
  const sourceId = project.anchors[facing]?.activeImportId;
  const snapId = project.anchors[facing]?.activeSnapId;
  if (!sourceId || !snapId) return null;
  for (let index = project.cleanupReviews.length - 1; index >= 0; index--) {
    const review = project.cleanupReviews[index];
    if (review.facing === facing && review.sourceImportId === sourceId && review.sourceSnapId === snapId) return review;
  }
  return null;
}
