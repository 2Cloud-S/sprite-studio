"""Find foreground pose groups without assuming a rigid board grid."""

import argparse
import json
import sys
from pathlib import Path

import cv2
import numpy as np
from PIL import Image


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("source")
    parser.add_argument("output_dir")
    parser.add_argument("background")
    parser.add_argument("tolerance", type=int)
    parser.add_argument("min_area", type=int)
    parser.add_argument("merge_gap", type=int)
    args = parser.parse_args()
    with Image.open(args.source) as source:
        pixels = np.asarray(source.convert("RGBA"), dtype=np.uint8)
    height, width = pixels.shape[:2]
    if args.background == "auto":
        corners = [tuple(pixels[y, x, :3]) for y, x in ((0, 0), (0, -1), (-1, 0), (-1, -1))]
        background = max(set(corners), key=lambda candidate: (corners.count(candidate), -corners.index(candidate)))
    else:
        background = tuple(int(args.background[i:i + 2], 16) for i in (0, 2, 4))
    difference = pixels[:, :, :3].astype(np.int16) - np.array(background, dtype=np.int16)
    key = np.sum(difference.astype(np.int32) ** 2, axis=2) <= args.tolerance ** 2
    foreground = (pixels[:, :, 3] >= 128) & ~key
    if args.merge_gap:
        size = args.merge_gap * 2 + 1
        grouped = cv2.dilate(foreground.astype(np.uint8), np.ones((size, size), np.uint8))
    else:
        grouped = foreground.astype(np.uint8)
    count, labels, _, _ = cv2.connectedComponentsWithStats(grouped, 8)
    found = []
    for label in range(1, count):
        ys, xs = np.nonzero(foreground & (labels == label))
        if len(xs) < args.min_area:
            continue
        x0, x1 = max(0, int(xs.min()) - 2), min(width, int(xs.max()) + 3)
        y0, y1 = max(0, int(ys.min()) - 2), min(height, int(ys.max()) + 3)
        found.append((y0, x0, x1 - x0, y1 - y0, len(xs)))
    # Group by visual row before sorting X: poses in one row often have
    # different heights, so sorting by top edge alone interleaves columns.
    rows = []
    for box in sorted(found, key=lambda item: item[0] + item[3] / 2):
        center = box[0] + box[3] / 2
        matches = [row for row in rows if abs(center - row["center"]) <= max(8, min(box[3], row["height"]) * 0.5)]
        if matches:
            row = min(matches, key=lambda item: abs(center - item["center"]))
            row["boxes"].append(box)
            row["center"] = sum(item[0] + item[3] / 2 for item in row["boxes"]) / len(row["boxes"])
            row["height"] = sum(item[3] for item in row["boxes"]) / len(row["boxes"])
        else:
            rows.append({"center": center, "height": box[3], "boxes": [box]})
    found = [box for row in sorted(rows, key=lambda item: item["center"]) for box in sorted(row["boxes"], key=lambda item: item[1])]
    if not found:
        raise ValueError("No pose groups detected; change the chroma or component settings")
    if len(found) > 64:
        raise ValueError("More than 64 groups detected; increase minimum area or merge gap")
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    boxes = []
    for index, (y, x, w, h, area) in enumerate(found):
        filename = f"candidate-{index:02d}.png"
        Image.fromarray(pixels[y:y + h, x:x + w], "RGBA").save(output_dir / filename, format="PNG")
        boxes.append({"x": x, "y": y, "width": w, "height": h,
                      "foregroundPixels": area, "filename": filename})
    print(json.dumps({"backgroundHex": "".join(f"{v:02x}" for v in background), "boxes": boxes}))


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
