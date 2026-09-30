"""Build a draft sheet and timing GIF from immutable normalized frames."""

import json
import sys

from PIL import Image
import numpy as np


def main() -> None:
    config = json.load(sys.stdin)
    width, height = config["cellWidth"], config["cellHeight"]
    columns = config["columns"]
    count = len(config["frames"])
    rows = (count + columns - 1) // columns
    padding, spacing = config["padding"], config["spacing"]
    sheet_width = 2 * padding + columns * width + (columns - 1) * spacing
    sheet_height = 2 * padding + rows * height + (rows - 1) * spacing
    sheet = Image.new("RGBA", (sheet_width, sheet_height), (0, 0, 0, 0))
    aligned_frames = []

    for index, frame_info in enumerate(config["frames"]):
        with Image.open(frame_info["path"]) as source:
            source = source.convert("RGBA")
            if source.size != (width, height):
                raise ValueError("A source frame no longer matches the fixed cell")
            pixels = np.asarray(source, dtype=np.uint8).copy()
            pixels[pixels[:, :, 3] == 0, :3] = 0
            source = Image.fromarray(pixels, "RGBA")
            aligned = Image.new("RGBA", (width, height), (0, 0, 0, 0))
            aligned.paste(source, (frame_info["x"], frame_info["y"]))
        x = padding + (index % columns) * (width + spacing)
        y = padding + (index // columns) * (height + spacing)
        sheet.paste(aligned, (x, y))
        aligned_frames.append(aligned)

    sheet.save(config["sheetPath"], format="PNG", compress_level=9)
    # GIF is a review aid; game timing comes from the manifest and, for
    # KangiFight, the authoritative hold_ticks register.
    duration_ms = max(20, round(1000 / config["fps"] / 10) * 10)
    kwargs = {"format": "GIF", "save_all": True, "append_images": aligned_frames[1:],
              "duration": [duration_ms] * count, "disposal": 2, "optimize": False}
    if config["looping"]:
        kwargs["loop"] = 0
    aligned_frames[0].save(config["gifPath"], **kwargs)
    with Image.open(config["gifPath"]) as gif:
        gif_frames = gif.n_frames
        total_duration = 0
        for index in range(gif_frames):
            gif.seek(index)
            total_duration += gif.info.get("duration", 0)
    if total_duration != count * duration_ms:
        raise ValueError("GIF playback duration does not match the source frame timing")
    print(json.dumps({"sheetWidth": sheet_width, "sheetHeight": sheet_height,
                      "rows": rows, "gifDurationMs": duration_ms, "gifFrameCount": gif_frames,
                      "gifTotalDurationMs": total_duration}))


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
