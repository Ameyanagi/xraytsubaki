"""Run against an installed wheel, not the source package."""
import unittest
from pathlib import Path

import numpy as np
import rexafs

FIXTURE = Path(__file__).resolve().parents[2] / "crates/rexafs/tests/testfiles/Ru_QAS.dat"


class ProcessingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        data = np.loadtxt(FIXTURE)
        cls.energy, cls.mu = data[:, 0], np.log(data[:, 1] / data[:, 2])

    def test_real_spectrum_and_owned_outputs(self):
        energy, mu = self.energy.copy(), self.mu.copy()
        output = rexafs.process(energy, mu)
        self.assertIsInstance(output, rexafs.ProcessedSpectrum)
        self.assertGreater(output.e0, energy[0])
        self.assertEqual(output.k.shape, output.chi.shape)
        for name in ["chir_mag", "chir_re", "chir_im"]:
            self.assertEqual(getattr(output, name).shape, output.r.shape)
        np.testing.assert_allclose(output.chir_mag, np.hypot(output.chir_re, output.chir_im), atol=1e-10)
        np.testing.assert_array_equal(energy, self.energy)
        np.testing.assert_array_equal(mu, self.mu)
        self.assertTrue(np.isfinite(output.chi).all())
        self.assertTrue(np.isfinite(output.chir_mag).all())

    def test_strided_input_and_edge_override(self):
        # Original data columns are strided, and remain accepted by the binding.
        result = rexafs.process(self.energy, self.mu)
        explicit = rexafs.process(self.energy.tolist(), self.mu, e0=result.e0 + 0.25)
        self.assertEqual(explicit.e0, result.e0 + 0.25)

    def test_invalid_arrays_raise_python_errors(self):
        for energy, mu in [([], []), ([2, 1, 3], [1]), ([1, 2, 2], [1, 2, 3]),
                           ([1, np.nan, 3], [1, 2, 3]), ([1, 2, 3], [1, np.inf, 3]),
                           ([[1, 2]], [[1, 2]])]:
            with self.subTest(energy=energy), self.assertRaises(ValueError):
                rexafs.process(energy, mu)
        with self.assertRaises(ValueError):
            rexafs.process(self.energy, self.mu, e0=float("nan"))

    def test_batch_reports_original_indices_and_only_successes(self):
        result = rexafs.process_qas_batch([FIXTURE, FIXTURE.with_name("missing-rexafs.dat"), FIXTURE])
        self.assertEqual(result.processed_count, 2)
        self.assertEqual(len(result.errors), 1)
        self.assertEqual(result.errors[0].index, 1)
        self.assertEqual(result.errors[0].category, "io")
        self.assertEqual(rexafs.run_batch_qas_trans([]), (0, []))

    def test_compatibility_dictionary(self):
        output = rexafs.run_pipeline_arrays(self.energy, self.mu)
        self.assertEqual(set(output), {"e0", "k", "chi", "r", "chir_mag", "chir_re", "chir_im"})


if __name__ == "__main__":
    unittest.main()
