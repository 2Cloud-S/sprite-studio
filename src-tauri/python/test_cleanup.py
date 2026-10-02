"""Regression checks for fixed-cell cleanup and reviewable fit guidance."""

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import numpy as np
from PIL import Image


SCRIPT = Path(__file__).with_name("cleanup.py")
MARKER = "SPRITE_STUDIO_FIT:"


class CleanupFitTests(unittest.TestCase):
    def test_dark_green_fringe_is_keyed_only_near_background(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            source = folder / "native.png"
            image = Image.new("RGB", (32, 32), (4, 247, 7))
            image.paste((210, 106, 53), (8, 8, 24, 24))
            image.putpixel((8, 12), (42, 95, 21))
            image.putpixel((9, 12), (20, 110, 11))
            image.putpixel((16, 16), (42, 95, 21))
            image.save(source)

            def run(fringe: str) -> np.ndarray:
                normalized = folder / f"normalized-{fringe}.png"
                result = subprocess.run(
                    [sys.executable, str(SCRIPT), str(source), str(folder / f"cleaned-{fringe}.png"),
                     str(normalized), "auto", "18", "0", "32", "32", "16", "31", fringe],
                    capture_output=True, text=True, check=False,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                with Image.open(normalized) as output:
                    return np.asarray(output.convert("RGBA"))

            disabled = run("0")
            enabled = run("1")
            # Normalization moves the 16x16 crop from (8,8) to (8,16).
            self.assertEqual(int(disabled[20, 8, 3]), 255)
            self.assertEqual(int(disabled[20, 9, 3]), 255)
            self.assertEqual(int(enabled[20, 8, 3]), 0)
            self.assertEqual(int(enabled[20, 9, 3]), 0)
            self.assertEqual(tuple(enabled[24, 16]), (42, 95, 21, 255))
            self.assertEqual(tuple(enabled[20, 10]), (210, 106, 53, 255))
            self.assertEqual(set(np.unique(enabled[:, :, 3]).tolist()), {0, 255})

    def test_inspection_uses_pivot_not_just_cell_dimensions(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            source = folder / "native.png"

            def inspect(box: tuple[int, int, int, int], pivot_y: int = 112) -> dict:
                image = Image.new("RGB", (160, 160), (3, 248, 3))
                image.paste((180, 80, 30), box)
                image.save(source)
                result = subprocess.run(
                    [sys.executable, str(SCRIPT), str(source), str(folder / "cleaned.png"),
                     str(folder / "normalized.png"), "auto", "0", "0", "128", "128",
                     "64", str(pivot_y), "1", "--inspect-only"],
                    capture_output=True, text=True, check=False,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertFalse((folder / "cleaned.png").exists())
                self.assertFalse((folder / "normalized.png").exists())
                return json.loads(result.stdout)

            pivot_overflow = inspect((15, 10, 115, 136))
            self.assertEqual((pivot_overflow["foregroundWidth"], pivot_overflow["foregroundHeight"]), (100, 126))
            self.assertEqual(pivot_overflow["placementY"], -13)
            self.assertFalse(pivot_overflow["fits"])

            one_pixel_wide = inspect((10, 10, 139, 80), pivot_y=127)
            self.assertEqual(one_pixel_wide["foregroundWidth"], 129)
            self.assertFalse(one_pixel_wide["fits"])

            usable_height = inspect((20, 10, 100, 124))
            self.assertEqual(usable_height["foregroundHeight"], 114)
            self.assertFalse(usable_height["fits"])

    def test_oversized_foreground_suggests_intact_larger_cell(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            source = folder / "native.png"
            cleaned = folder / "cleaned.png"
            normalized = folder / "normalized.png"
            image = Image.new("RGB", (158, 155), (3, 248, 3))
            image.paste((180, 80, 30), (11, 13, 140, 140))
            image.save(source)

            def run(width: int, height: int, pivot_x: int, pivot_y: int) -> subprocess.CompletedProcess[str]:
                return subprocess.run(
                    [sys.executable, str(SCRIPT), str(source), str(cleaned), str(normalized),
                     "auto", "18", "2", str(width), str(height), str(pivot_x), str(pivot_y)],
                    capture_output=True, text=True, check=False,
                )

            failed = run(128, 128, 64, 112)
            self.assertNotEqual(failed.returncode, 0)
            self.assertTrue(failed.stderr.startswith(MARKER))
            fit = json.loads(failed.stderr[len(MARKER):])
            self.assertEqual((fit["foregroundWidth"], fit["foregroundHeight"]), (129, 127))
            self.assertEqual(fit["suggested"], {
                "cellWidth": 144, "cellHeight": 144, "pivotX": 72, "pivotY": 128,
            })
            self.assertFalse(cleaned.exists())
            self.assertFalse(normalized.exists())

            suggestion = fit["suggested"]
            passed = run(suggestion["cellWidth"], suggestion["cellHeight"],
                         suggestion["pivotX"], suggestion["pivotY"])
            self.assertEqual(passed.returncode, 0, passed.stderr)
            metrics = json.loads(passed.stdout)
            self.assertEqual((metrics["bboxWidth"], metrics["bboxHeight"]), (129, 127))
            with Image.open(normalized) as result:
                self.assertEqual(result.size, (144, 144))
                self.assertEqual(int(np.count_nonzero(np.asarray(result.getchannel("A")))), 129 * 127)


if __name__ == "__main__":
    unittest.main()
