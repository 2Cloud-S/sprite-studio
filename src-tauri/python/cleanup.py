"""Deterministic, non-smoothing cleanup of one recovered pixel grid."""

import argparse
import json
import sys

import cv2
import numpy as np
from PIL import Image

PROCESSOR_VERSION = "sprite-studio-cleanup-1"
FIT_ERROR_PREFIX = "SPRITE_STUDIO_FIT:"


def suggested_runtime(cell_width: int, cell_height: int, pivot_x: int, pivot_y: int,
                      bbox_width: int, bbox_height: int) -> dict | None:
    """Grow an editable cell without changing the pixel grid or bottom margin."""
    horizontal_offset = pivot_x - cell_width // 2
    bottom_margin = cell_height - 1 - pivot_y
    width = cell_width
    while width <= 1024:
        suggested_x = width // 2 + horizontal_offset
        left = suggested_x - bbox_width // 2
        if 0 <= left and left + bbox_width <= width and 0 <= suggested_x < width:
            break
        width = ((width + 16) // 16) * 16
    height = cell_height
    while height <= 1024 and height - 1 - bottom_margin < bbox_height - 1:
        height = ((height + 16) // 16) * 16
    if width > 1024 or height > 1024:
        return None
    return {
        "cellWidth": width,
        "cellHeight": height,
        "pivotX": width // 2 + horizontal_offset,
        "pivotY": height - 1 - bottom_margin,
    }


def color_value(value: str, pixels: np.ndarray) -> np.ndarray:
    if value == "auto":
        corners = [tuple(pixels[y, x, :3]) for y, x in ((0, 0), (0, -1), (-1, 0), (-1, -1))]
        chosen = max(set(corners), key=lambda candidate: (corners.count(candidate), -corners.index(candidate)))
        return np.array(chosen, dtype=np.int16)
    return np.array([int(value[index:index + 2], 16) for index in (0, 2, 4)], dtype=np.int16)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("source")
    parser.add_argument("cleaned")
    parser.add_argument("normalized")
    parser.add_argument("background")
    parser.add_argument("tolerance", type=int)
    parser.add_argument("min_area", type=int)
    parser.add_argument("cell_width", type=int)
    parser.add_argument("cell_height", type=int)
    parser.add_argument("pivot_x", type=int)
    parser.add_argument("pivot_y", type=int)
    parser.add_argument("fringe_cleanup", nargs="?", default="1")
    parser.add_argument("--inspect-only", action="store_true")
    args = parser.parse_args()

    with Image.open(args.source) as image:
        pixels = np.asarray(image.convert("RGBA"), dtype=np.uint8).copy()
    height, width = pixels.shape[:2]
    background = color_value(args.background, pixels)
    difference = pixels[:, :, :3].astype(np.int16) - background
    chroma = np.sum(difference.astype(np.int32) ** 2, axis=2) <= args.tolerance ** 2
    if args.fringe_cleanup == "1" and int(background[1]) > int(background[0]) + 80 and int(background[1]) > int(background[2]) + 80:
        # The snapper can turn a bright-green backdrop into isolated, much
        # darker green pixels at the contour. A narrow RGB radius misses them.
        # Key only green-derived colors within two pixels of the original
        # chroma region; never remove green globally from the character.
        near = np.max(np.abs(difference), axis=2) <= max(48, args.tolerance * 2)
        adjacent_one = cv2.dilate(chroma.astype(np.uint8), np.ones((3, 3), dtype=np.uint8)).astype(bool)
        adjacent_two = cv2.dilate(chroma.astype(np.uint8), np.ones((5, 5), dtype=np.uint8)).astype(bool)
        red = pixels[:, :, 0].astype(np.int16)
        green = pixels[:, :, 1].astype(np.int16)
        blue = pixels[:, :, 2].astype(np.int16)
        green_bleed = (green >= 60) & (green >= red + 40) & (green >= blue + 40)
        green_bleed &= (red <= max(64, int(background[0]) + 50))
        green_bleed &= (blue <= max(64, int(background[2]) + 50))
        chroma |= (near & adjacent_one) | (green_bleed & adjacent_two)
    foreground = (pixels[:, :, 3] >= 128) & ~chroma
    before = int(np.count_nonzero(foreground))

    if args.min_area > 1:
        count, labels, stats, _ = cv2.connectedComponentsWithStats(foreground.astype(np.uint8), 8)
        keep = np.zeros(count, dtype=bool)
        keep[1:] = stats[1:, cv2.CC_STAT_AREA] >= args.min_area
        foreground &= keep[labels]
    remaining = int(np.count_nonzero(foreground))
    if remaining == 0:
        raise ValueError("Cleanup removed the whole foreground; change the chroma color or tolerance")

    ys, xs = np.nonzero(foreground)
    x0, x1 = int(xs.min()), int(xs.max()) + 1
    y0, y1 = int(ys.min()), int(ys.max()) + 1
    bbox_width, bbox_height = x1 - x0, y1 - y0
    placement_x = args.pivot_x - bbox_width // 2
    placement_y = args.pivot_y - bbox_height + 1
    fits = placement_x >= 0 and placement_y >= 0 and placement_x + bbox_width <= args.cell_width and placement_y + bbox_height <= args.cell_height
    if args.inspect_only:
        print(json.dumps({
            "processorVersion": PROCESSOR_VERSION,
            "foregroundWidth": bbox_width,
            "foregroundHeight": bbox_height,
            "foregroundPixels": remaining,
            "removedSpecklePixels": before - remaining,
            "placementX": placement_x,
            "placementY": placement_y,
            "fits": fits,
        }))
        return
    if not fits:
        raise ValueError(FIT_ERROR_PREFIX + json.dumps({
            "foregroundWidth": bbox_width,
            "foregroundHeight": bbox_height,
            "cellWidth": args.cell_width,
            "cellHeight": args.cell_height,
            "pivotX": args.pivot_x,
            "pivotY": args.pivot_y,
            "suggested": suggested_runtime(args.cell_width, args.cell_height,
                                            args.pivot_x, args.pivot_y, bbox_width, bbox_height),
        }, separators=(",", ":")))

    pixels[:, :, 3] = np.where(foreground, 255, 0).astype(np.uint8)
    pixels[~foreground, :3] = 0
    cropped = pixels[y0:y1, x0:x1].copy()
    Image.fromarray(cropped, "RGBA").save(args.cleaned, format="PNG")
    cell = np.zeros((args.cell_height, args.cell_width, 4), dtype=np.uint8)
    cell[placement_y:placement_y + bbox_height, placement_x:placement_x + bbox_width] = cropped
    Image.fromarray(cell, "RGBA").save(args.normalized, format="PNG")
    print(json.dumps({
        "processorVersion": PROCESSOR_VERSION,
        "backgroundHex": "".join(f"{int(v):02x}" for v in background),
        "foregroundPixels": remaining,
        "removedSpecklePixels": before - remaining,
        "bboxWidth": bbox_width,
        "bboxHeight": bbox_height,
        "placementX": placement_x,
        "placementY": placement_y,
    }))


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
