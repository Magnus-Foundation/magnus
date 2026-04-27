# Magnus — Claude code-style notes

See [AGENTS.md](AGENTS.md) for PR/MIP conventions.

## Code comments

- Keep comments short. One line is usually enough.
- Don't reference task IDs, plan groups, version numbers (e.g. "G0 stub", "v3.8.2 §4", "added in T4"). Code lives in source; design lives in `transfer-station/*.md`. Commit messages link the two.
- Don't restate what well-named code already shows.
- Comment WHY, not WHAT — only when WHY is non-obvious (constraint, invariant, workaround, surprising behaviour).
- No multi-paragraph docstrings on internal helpers. Functions in public APIs may have brief `///` summaries.
