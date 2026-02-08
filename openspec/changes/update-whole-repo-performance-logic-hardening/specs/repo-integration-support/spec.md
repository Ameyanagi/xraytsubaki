## ADDED Requirements

### Requirement: Core-First CI Isolation
Repository CI SHALL isolate core crate validation from GUI-target failures to speed and de-risk feedback.

#### Scenario: Core crate check independence
- **WHEN** GUI-related jobs fail
- **THEN** core crate compile/test status remains independently visible
- **AND** core-only changes are validated without GUI job coupling

### Requirement: Python Batch Interface Stability
Python bindings SHALL expose a minimal stable batch-processing interface that maps core typed failures predictably.

#### Scenario: Batch error mapping behavior
- **WHEN** Python batch processing encounters per-spectrum failures
- **THEN** bindings expose structured failure details including failing index and cause category
- **AND** behavior is stable across patch releases unless explicitly versioned

### Requirement: GUI Core Invocation Boundary
GUI workflows SHALL call optimized core APIs asynchronously for large dataset processing.

#### Scenario: Large dataset processing from GUI
- **WHEN** a GUI operation triggers processing of large spectrum sets
- **THEN** the GUI dispatches work asynchronously to core APIs
- **AND** UI responsiveness is preserved while core processing runs
