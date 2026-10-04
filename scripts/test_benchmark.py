"""Checks that comparison reports catch differences and reject incompatible runs."""
import copy
import importlib.util
from pathlib import Path
import unittest
import tempfile

spec = importlib.util.spec_from_file_location("benchmark", Path(__file__).with_name("benchmark.py"))
b = importlib.util.module_from_spec(spec)
spec.loader.exec_module(b)


class BenchmarkTests(unittest.TestCase):
    def test_known_image_error_and_meter_channel_exclusion(self):
        a = bytes((10, 20, 30, 255))
        same, diff = b.image_metrics(a, a, [1., 2., 3., 42.], [1., 2., 3., 99.])
        self.assertEqual(same["linear_rgb_rmse"], 0)
        self.assertEqual(same["display_mae_255"], 0)
        self.assertEqual(diff, bytes((0, 0, 0, 255)))
        changed, _ = b.image_metrics(a, bytes((13, 20, 30, 255)), [1., 2., 3., 0.], [2., 2., 3., 0.])
        self.assertEqual(changed["display_mae_255"], 1)
        self.assertAlmostEqual(changed["linear_rgb_rmse"], (1/3)**.5)
        self.assertEqual(changed["pixels_changed_over_1_255_percent"], 100)

    def test_median_p95_and_repeat_spread(self):
        mode = {"repeats": [[{"gpu_ms": x, "wall_ms": x+1} for x in [1, 3]],
                            [{"gpu_ms": x, "wall_ms": x+1} for x in [2, 100]]]}
        s = b.summary(mode)
        self.assertEqual(s["gpu_median_ms"], 2.5)
        self.assertAlmostEqual(s["gpu_p95_ms"], 85.45)
        self.assertEqual(s["repeat_medians_ms"], [2, 51])

    def test_png_round_trip_and_missing_hdr_is_explicit(self):
        pixels = bytes((10, 20, 30, 255, 255, 0, 99, 255))
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "test.png"
            b.png_write(path, 2, 1, pixels)
            self.assertEqual(b.png_read(path), pixels)
        metrics, _ = b.image_metrics(pixels, pixels)
        self.assertEqual(metrics["display_mae_255"], 0)
        self.assertIsNone(metrics["linear_rgb_rmse"])

    def test_reject_changed_gpu_settings_or_scene(self):
        report = {"suite_version": 1, "adapter": {"name": "GPU"}, "trace_size": [4, 4],
                  "cases": [{"name": "scene", "frame_fingerprints": ["a"], "images": []}]}
        config = {key: 1 for key in b.COMPATIBLE}
        b.check_compatible(report, copy.deepcopy(report), config, config)
        for mutate in [lambda r: r["adapter"].update(name="other"),
                       lambda r: r["cases"][0].update(frame_fingerprints=["b"]),
                       lambda r: r.update(cases=[])]:
            candidate = copy.deepcopy(report)
            mutate(candidate)
            with self.assertRaises(ValueError):
                b.check_compatible(report, candidate, config, config)
        candidate_config = dict(config, bounces=2)
        with self.assertRaises(ValueError):
            b.check_compatible(report, report, config, candidate_config)


if __name__ == "__main__":
    unittest.main()
