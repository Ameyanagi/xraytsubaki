"""Run against the built wheel, not the source package."""
import unittest
from pathlib import Path
import numpy as np
import rexafs

FIXTURE = Path(__file__).resolve().parents[2] / "crates/rexafs/tests/testfiles/Ru_QAS.dat"

class SpectrumTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        data = np.loadtxt(FIXTURE)
        cls.energy, cls.mu = data[:, 0], np.log(data[:, 1] / data[:, 2])

    def spectrum(self):
        return rexafs.Spectrum.from_arrays(self.energy, self.mu)

    def test_terminal_stage_matches_explicit_chain(self):
        spectrum = self.spectrum()
        self.assertIsNone(spectrum.chi())
        self.assertIs(spectrum.fft(), spectrum)
        explicit = self.spectrum().find_e0().normalize().calc_background().fft()
        for other in (explicit,):
            self.assertEqual(spectrum.e0(), other.e0())
            for name in ("norm", "chi", "r", "chir_mag"):
                np.testing.assert_allclose(getattr(spectrum, name)(), getattr(other, name)())
        np.testing.assert_allclose(spectrum.chir_mag(), np.hypot(spectrum.chir_real(), spectrum.chir_imag()), atol=1e-10)

    def test_owned_arrays_and_input_conversion(self):
        energy, mu = self.energy.copy(), self.mu.copy()
        spectrum = rexafs.Spectrum(energy.tolist(), mu).fft()
        old_chi = spectrum.chi()
        saved_chi = old_chi.copy()
        old_chi[:] = 0
        np.testing.assert_allclose(spectrum.chi(), saved_chi)
        np.testing.assert_array_equal(energy, self.energy)
        np.testing.assert_array_equal(mu, self.mu)
        spectrum.set_e0(spectrum.e0() + 0.25).fft()
        np.testing.assert_array_equal(old_chi, np.zeros_like(old_chi))

    def test_parameters_and_invalidation(self):
        norm = rexafs.PrePostEdge()
        norm.pre_edge_start = -200
        norm.pre_edge_end = -65
        bkg = rexafs.AUTOBK()
        bkg.rbkg = 1.2
        ft = rexafs.XrayFFTF()
        ft.kweight = 1
        spectrum = (self.spectrum()
            .set_normalization_method(rexafs.NormalizationMethod.PrePostEdge(norm))
            .set_background_method(rexafs.BackgroundMethod.AUTOBK(bkg))
            .set_fft(ft).fft())
        chi, old_r = spectrum.chi(), spectrum.chir_mag()
        ft.kweight = 3
        spectrum.set_fft(ft)
        self.assertIsNone(spectrum.r())
        spectrum.fft()
        np.testing.assert_array_equal(spectrum.chi(), chi)
        self.assertFalse(np.allclose(spectrum.chir_mag(), old_r))
        edge = spectrum.e0() + 0.25
        spectrum.set_e0(edge)
        self.assertIsNone(spectrum.norm())
        self.assertIsNone(spectrum.chi())
        self.assertIsNone(spectrum.r())
        self.assertEqual(spectrum.fft().e0(), edge)
        spectrum.set_spectrum(self.energy, self.mu)
        self.assertIsNone(spectrum.e0())
        self.assertIsNone(spectrum.chi())
        self.assertIsNotNone(spectrum.fft().r())

    def test_fixed_lambda_is_configurable_and_zero_disables_both_clamps(self):
        bkg = rexafs.AUTOBK()
        self.assertEqual(bkg.clamp_scale_policy, "FixedPenalty")
        self.assertEqual(bkg.clamp_lambda, 0.001)
        bkg.kmax = 12.0
        bkg.clamp_lo = 2
        bkg.clamp_hi = 5
        def chi():
            return self.spectrum().set_background_method(rexafs.BackgroundMethod.AUTOBK(bkg)).calc_background().chi()
        initial = chi()
        bkg.clamp_lambda = 1.0
        self.assertGreater(np.linalg.norm(chi() - initial), 1e-6)
        bkg.clamp_lambda = 0.0
        zero = chi()
        bkg.nclamp = 0
        np.testing.assert_array_equal(chi(), zero)
        for invalid in (-1.0, float("nan"), float("inf")):
            bkg.clamp_lambda = invalid
            with self.assertRaisesRegex(RuntimeError, "clamp_lambda"):
                chi()

    def test_unsupported_methods_are_not_replaced(self):
        spectrum = self.spectrum().set_normalization_method(rexafs.NormalizationMethod.new_mback())
        with self.assertRaisesRegex(ValueError, "MBack"):
            spectrum.fft()
        spectrum.set_normalization_method().set_background_method(rexafs.BackgroundMethod.new_ilpbkg())
        with self.assertRaisesRegex(RuntimeError, "ILPBkg"):
            spectrum.fft()
        self.assertIsNone(spectrum.r())
        self.assertIsNotNone(spectrum.set_background_method().fft().r())

    def test_invalid_input_and_parameters_raise(self):
        for energy, mu in [([], []), ([2, 1, 3], [1]), ([1, 2, 2], [1, 2, 3]),
                           ([1, np.nan, 3], [1, 2, 3]), ([1, 2, 3], [1, np.inf, 3]),
                           ([[1, 2]], [[1, 2]])]:
            with self.subTest(energy=energy), self.assertRaises(ValueError):
                rexafs.Spectrum.from_arrays(energy, mu)
        with self.assertRaises(ValueError):
            self.spectrum().set_e0(float("nan")).fft()
        spectrum = self.spectrum().fft()
        ft = rexafs.XrayFFTF()
        ft.nfft = 0
        with self.assertRaises(RuntimeError):
            spectrum.set_fft(ft).fft()
        self.assertIsNone(spectrum.r())
        ft.nfft = 2048
        self.assertIsNotNone(spectrum.set_fft(ft).fft().r())

    def test_reader_and_removed_pipeline_facade(self):
        spectrum = rexafs.io.read_qas_transmission(FIXTURE).fft()
        self.assertIsInstance(spectrum, rexafs.Spectrum)
        np.testing.assert_allclose(spectrum.chi(), self.spectrum().fft().chi())
        for name in ("process", "ProcessedSpectrum", "process_qas_batch", "run_pipeline_arrays", "run_batch_qas_trans"):
            self.assertFalse(hasattr(rexafs, name), name)

if __name__ == "__main__":
    unittest.main()
