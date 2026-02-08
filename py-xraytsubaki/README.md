## py-xraytsubaki

Minimal stable Python API for batch processing and array-based pipeline execution.

### Stable Functions

- `run_batch_qas_trans(paths: List[str]) -> Tuple[int, List[Tuple[int, str, str]]]`
  - Runs `find_e0 -> normalize -> calc_background -> fft` on a batch.
  - Returns:
    - `processed_count`
    - `errors`: `(index, category, message)` entries
  - `category` is one of: `io`, `data`, `normalization`, `background`, `fft`, `math`, `group`.

- `run_pipeline_arrays(energy: numpy.ndarray, mu: numpy.ndarray) -> dict`
  - Runs the same core pipeline for one spectrum provided as arrays.
  - Returns a dictionary containing `e0`, `k`, `chi`, and `chir_mag` when available.

### Zero-Copy Interop Notes

- Inputs to `run_pipeline_arrays` are accepted as `PyReadonlyArray1<f64>`, which borrows NumPy memory
  without an intermediate Python-side copy.
- Core processing currently uses nalgebra-owned buffers internally, so one Rust-side copy is still required
  when entering the core pipeline.
