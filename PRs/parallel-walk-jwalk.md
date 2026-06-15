## Parallelize directory traversal with `jwalk`

### Summary

The scanner walked the filesystem with `walkdir`, which is single-threaded; `rayon` only parallelized the per-project size calculation and detection. For a tool whose dominant cost is `stat()`-ing large numbers of inodes, the walk itself was the most credible performance lever.

This PR replaces the detection walk with [`jwalk`](https://crates.io/crates/jwalk), which reads directories in parallel on the rayon pool, and prunes `node_modules` descent during the walk. Behaviour is unchanged: the same projects are detected, all 227 tests pass, and clippy (pedantic + nursery, `-D warnings`) is clean.

### Changes

**Parallel walk (`src/scanner.rs`)**

- Swapped `walkdir::WalkDir` for `jwalk::WalkDir` in `Scanner::scan_directory`. The walk now runs across the rayon pool instead of on a single thread.
- Added a `process_read_dir` callback that prunes `node_modules` directories (`read_children_path = None`). Nothing inside a `node_modules` tree is ever a project candidate (`should_scan_entry` already filtered it), so skipping descent is behaviour-preserving and avoids stat-ing the deepest, largest part of most JavaScript projects.
- `--threads 1` now uses `Parallelism::Serial`. jwalk's default `RayonDefaultPool` can stall when the global rayon pool only has a single thread, so the serial path keeps single-threaded scans correct.
- `should_scan_entry` and `detect_project` now take a `&Path` (+ an `is_dir` flag) instead of a `walkdir::DirEntry`, decoupling them from the walker implementation.

**Docs**

- Updated the acknowledgements in `README.md` (added jwalk; noted WalkDir is still used for size calculation).

### Benchmarks

Synthetic tree: ~72k inodes, 223 MB, 80 projects (40 Node with `node_modules`, 40 Rust with `target/`, plus plain trees). 12 alternating runs after warmup, release builds.

| Scenario | baseline (walkdir) | jwalk | speedup (median) |
|---|---|---|---|
| Full tree | 563 ms | 375 ms | **1.50x** |
| Plain trees, no `node_modules` (pure parallel walk) | 87 ms | 72 ms | 1.20x |
| Rust + `target/` (not pruned) | 64 ms | 55 ms | 1.17x |
| Node + `node_modules` (pruned) | 397 ms | 217 ms | **1.83x** |

Attribution: the parallel walk alone contributes ~1.2x; the bulk of the gain on JS-heavy trees comes from pruning `node_modules` descent (1.83x). Real dev directories are typically dominated by `node_modules`/`target`, so the combined effect should be at least as pronounced.

### Notes / follow-ups

- `walkdir` remains a dependency: `src/utils/size.rs::calculate_dir_size` still uses it. That size phase is also stat-intensive (it walks `target`/`node_modules` to sum sizes); parallelizing it via jwalk is a plausible next step, but it already runs inside a rayon `par_iter` at the project level, so nested parallelism should be benchmarked before committing to it.
- No public API or CLI change.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
