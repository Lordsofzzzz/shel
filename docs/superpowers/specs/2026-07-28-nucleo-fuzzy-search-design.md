# Fuzzy Search via nucleo for shel

## Summary

Replace shel's current SQL `LIKE` substring search with character-level fuzzy matching from the `nucleo` crate. Both the interactive TUI (`shel ui`) and the CLI search (`shel search`) use the same fuzzy scorer.

## Current state

- `db.rs:81`: SQL `LIKE %query%` (case-insensitive ASCII) — binary match, no ranking
- `tui.rs:53`: in-memory `command.contains(query)` after loading all unique entries
- `main.rs:88`: `db::search()` for CLI — same `LIKE` predicate
- Results ordered by timestamp desc only

No typo tolerance. "gti push" returns nothing even though "git push" exists.

## Chosen approach

**In-memory fuzzy matching via `nucleo::Matcher`.**

- All entries loaded once (existing behavior in TUI)
- `Matcher::fuzzy_match()` replaces `contains()` in `App::filter()`
- Results sorted by match score descending (best match first)
- Same matcher applied to `shel search` CLI
- No SQLite changes — `nucleo` has no SQLite extension, and in-memory is fast enough for typical history sizes (10k-100k entries)

## Files to create or modify

### `Cargo.toml`

Add dependency:

```toml
nucleo = "0.5"
```

### `src/search.rs` (new)

Public function:

- `fuzzy_search(entries: &[Entry], query: &str) -> Vec<(usize, u32)>`
  - Creates temporary `Matcher` (or takes `&mut Matcher`)
  - For empty query: returns all indices with score 0 (preserve original order)
  - For non-empty: calls `matcher.fuzzy_match(entry.command.as_bytes(), query.as_bytes())` on each
  - Filters out `None` results (no match), sorts remaining by score desc
  - Score tiebreaker: more recent timestamp first

### `src/lib.rs`

Add: `pub mod search;`

### `src/tui.rs`

Changes:

- `App::filtered`: `Vec<usize>` → `Vec<(usize, u32)>` (index + score)
- `App::matcher`: new field `Matcher` — created once in `App::new()`
- `App::filter()`: uses `matcher.fuzzy_match()` instead of `contains()`, sorts by score desc
- All `filtered` access: `self.filtered[idx].0` for entry index, `self.filtered[idx].1` for score
- `render()`: reads entry index from tuple, display unchanged
- `App::selected_index()` helper if needed

### `src/main.rs`

- `Cmd::Search` branch: load `db::list_all()`, apply `search::fuzzy_search()`, print in score order

## Data flow

```
User types in TUI
  → Event::Key('c')
  → app.query.push('c')
  → app.filter()
  → for each entry: matcher.fuzzy_match(command_bytes, query_bytes)
  → Vec<(usize, u32)> sorted desc by score
  → ListState updates to first match
  → render() draws filtered results

CLI: shel search "gti push"
  → db::list_all() → Vec<Entry>
  → search::fuzzy_search(&entries, "gti push")
  → print results in score order
```

## Performance

- `nucleo::Matcher` processes ~10k entries in under 1ms
- Matcher instance reused in `App` to avoid per-keystroke allocation overhead
- DB pre-filter only needed if history exceeds ~100k entries (defer)

## Testing

- Existing `test_filter_basic`, `test_filter_case_insensitive` etc. need updated assertions — order changes from timestamp-desc to score-desc
- Add test: fuzzy match with typos ("gti" → "git push")
- Add test: score ordering (high-score match before low-score)
- Add test: no match returns empty

## Rejected alternatives

1. **SQLite FTS5**: word-oriented, not character-level fuzzy. Wrong tool.
2. **fuzzy-matcher (SkimMatcherV2)**: simpler API but slower than nucleo. "nucleo" was chosen by user.
3. **DB-side scoring via custom function**: requires loading a Rust extension into SQLite. Not worth complexity.

## Build sequence

1. Add `nucleo` to `Cargo.toml`
2. Create `src/search.rs`
3. Register `pub mod search` in `src/lib.rs`
4. Update `src/tui.rs`: `App` struct, `filter()`, all `filtered` access
5. Update `src/main.rs`: `Cmd::Search` uses `fuzzy_search()`
6. `cargo test` — update assertions as needed
7. `cargo build` — verify no warnings
