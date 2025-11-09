<!-- OPENSPEC:START -->
# OpenSpec Instructions

These instructions are for AI assistants working in this project.

Always open `@/openspec/AGENTS.md` when the request:
- Mentions planning or proposals (words like proposal, spec, change, plan)
- Introduces new capabilities, breaking changes, architecture shifts, or big performance/security work
- Sounds ambiguous and you need the authoritative spec before coding

Use `@/openspec/AGENTS.md` to learn:
- How to create and apply change proposals
- Spec format and conventions
- Project structure and guidelines

Keep this managed block so 'openspec update' can refresh the instructions.

<!-- OPENSPEC:END -->

# Project-Specific Instructions

## Autonomous Work Mode

**Default Behavior**: When tasks are clear and testable, proceed with implementation autonomously without asking for approval.

**Execute Immediately When**:
- Bug fixes with clear reproduction steps
- Refactoring with existing test coverage
- Code improvements with validation tests available
- Implementation tasks with well-defined acceptance criteria
- Performance optimizations with benchmarks to verify
- Any task where success can be objectively validated

**Only Ask for Approval When**:
- Requirements are genuinely ambiguous or underspecified
- Multiple valid implementation approaches exist with significant trade-offs
- Breaking changes that affect public APIs
- Architectural decisions with long-term implications
- Critical system changes without automated validation
- Blocked by missing information or access

**Validation Requirements**:
- Run tests after implementation to verify correctness
- Execute benchmarks for performance-related changes
- Use `cargo check`, `cargo clippy`, and `cargo test` to validate Rust code
- Report results only after validation is complete

**Communication Style**:
- Brief progress updates during complex tasks
- Report completion with test/benchmark results
- Ask clarifying questions upfront before starting work
- Focus on outcomes and evidence, not seeking permission

**Example Workflow**:
```
✅ Right: "Refactoring XASGroup::normalize() → Running tests → All 47 tests passed"
❌ Wrong: "Should I refactor XASGroup::normalize()? Let me know if you want me to proceed"

✅ Right: "Fixed AUTOBK convergence issue → Benchmark: 8.2s → 7.5s (9% improvement)"
❌ Wrong: "I found the issue. Do you want me to fix it?"
```