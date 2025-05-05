# Migration Plan: Replacing pip with uv

## Overview
This document outlines the plan to replace all pip usage with uv throughout the XrayTsubaki codebase. This migration will use uv for package management, dependency installation, and running Python scripts.

## Files Requiring Updates
The following files contain pip references that need to be updated:

1. BUILD_PYXRAYTSUBAKI.sh
2. FIXED_BUILD_SCRIPT.sh
3. pyxraytsubaki/INSTALL.md
4. pyxraytsubaki/README.md
5. PYTHON-BINDING-FIX.md
6. BUILD-PYTHON.md
7. py-xraytsubaki/README.md

## Replacement Patterns

### Command Replacements
| pip Command | uv Replacement |
|-------------|----------------|
| `pip install <package>` | `uv add <package>` |
| `pip install -r requirements.txt` | `uv pip install -r requirements.txt` |
| `pip install -e .` | `uv pip install -e .` |
| `python -m pip install <package>` | `uv add <package>` |

### Script Execution
Replace:
```bash
python script.py
```

With:
```bash
uv run python script.py
```

## Post-Migration Verification
1. Run all tests to verify functionality
2. Fix any failing tests related to the migration
3. Ensure all build scripts work correctly with uv

## Testing Strategy
1. After completing all replacements, run the test suite
2. Identify and fix any failing tests
3. Ensure all Python-dependent functionality continues to work

## Notes
- The CLAUDE.md file already contains references to using uv instead of pip, which confirms this is the correct approach.
- Some files may contain pip in contexts unrelated to package installation (like in documentation examples); these should be updated if they represent actionable commands.