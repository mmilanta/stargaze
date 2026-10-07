"""Check experiment ordering and that aggregate reports expose timing reversals."""
import unittest
import tempfile
from pathlib import Path
import optimization_study as study


class StudyTests(unittest.TestCase):
    def test_bracketed_reversed_rounds(self):
        rounds = study.plan(['a', 'b'], 2)
        self.assertEqual([v for _, v in rounds[0]], ['reference', 'a', 'b', 'reference'])
        self.assertEqual([v for _, v in rounds[1]], ['reference', 'b', 'a', 'reference'])
        paths = [p for runs in rounds for p, _ in runs]
        self.assertEqual(len(paths), len(set(paths)))

    def test_range_keeps_regression_and_improvement(self):
        def comparison(speedup, milliseconds):
            return {'timings': [{'case': 'night', 'mode': 'advancing', 'speedup': speedup,
                                'baseline': {'gpu_median_ms': 10},
                                'candidate': {'gpu_median_ms': milliseconds}}]}
        rows = study.summarize([
            ('a', 1, 'before', comparison(1.2, 8)),
            ('a', 1, 'after', comparison(0.9, 8)),
            ('a', 2, 'before', comparison(1.1, 9)),
            ('a', 2, 'after', comparison(0.8, 9)),
        ])
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]['speedup_min'], 0.8)
        self.assertEqual(rows[0]['speedup_max'], 1.2)
        self.assertEqual(rows[0]['candidate_median_ms'], 8.5)

    def test_raw_candidate_with_hdr_difference_cannot_claim_equivalence(self):
        comparison = {'timings': [], 'images': [{'linear_rgb_rmse': 0.001, 'display_mae_255': 0}]}
        with tempfile.TemporaryDirectory() as temp:
            out = Path(temp)
            study.publish(out, [('sky-any-hit', 1, 'before', comparison)], 1)
            report = (out/'README.md').read_text()
            self.assertIn('DIFFERENCES', report)
            self.assertNotIn('sky-any-hit: identical captures', report)


if __name__ == '__main__':
    unittest.main()
