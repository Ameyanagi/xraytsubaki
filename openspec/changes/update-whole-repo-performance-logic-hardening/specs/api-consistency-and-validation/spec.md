## ADDED Requirements

### Requirement: Path API Generalization
Path-based spectrum loading APIs SHALL accept generic path references rather than `&String`.

#### Scenario: Flexible path argument type
- **WHEN** callers pass `&str`, `String`, or `Path`-compatible values
- **THEN** loading APIs accept them via `AsRef<Path>` without requiring intermediate string ownership changes

### Requirement: seq/par/default Semantic Consistency
Sequential, parallel, and default processing variants SHALL have consistent error semantics and deterministic state transition behavior.

#### Scenario: Equivalent failure semantics across modes
- **WHEN** equivalent input failures occur in seq and par variants
- **THEN** both modes return the same error class with equivalent spectrum-index context
- **AND** behavior does not diverge into panic vs result mismatch

### Requirement: AUTOBK Knot-Domain Validation
AUTOBK knot-domain construction SHALL be covered by explicit numerical tests.

#### Scenario: Knot domain validity check
- **WHEN** AUTOBK builds knot domains for representative energy/k ranges
- **THEN** tests verify domain bounds and placement consistency
- **AND** invalid knot-domain construction is caught by automated tests
