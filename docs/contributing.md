# Contributing

Contributions are welcome. The primary repository is on [git.tomplanche.com](https://git.tomplanche.com/clean-dev-dirs/clean-dev-dirs), the self-hosted Forgejo instance of [Tom Planche](https://tomplanche.com). GitHub is a mirror.

- To report a problem, open an issue on [Forgejo](https://git.tomplanche.com/clean-dev-dirs/clean-dev-dirs/issues). If you cannot use Forgejo, use [GitHub](https://github.com/clean-dev-dirs/clean-dev-dirs/issues).
- To propose a change, open a pull request on [Forgejo](https://git.tomplanche.com/clean-dev-dirs/clean-dev-dirs/pulls). If you cannot use Forgejo, use [GitHub](https://github.com/clean-dev-dirs/clean-dev-dirs/pulls).

## Development commands

```bash
cargo build                # Build
cargo test                 # Run all the tests
cargo fmt                  # Format the code
cargo clippy --workspace --release --all-targets --all-features -- -D warnings   # Lint
```

The pre-commit hook runs `cargo fmt`, `cargo clippy`, `cargo audit`, and `cargo outdated`. The pre-push hook runs the tests and the build. `build.rs` installs these hooks with hooksmith.

The Clippy configuration denies the `pedantic` and `nursery` groups. Your code must pass Clippy with no warning.

## Add support for a new language

The example below adds a language with the name `YourLanguage`. Use the Zig or Scala support as a model: their code is short.

1. Add a variant to the `ProjectType` enum in `src/project/project.rs`:

   ```rust
   pub enum ProjectType {
       // ...
       YourLanguage,
   }
   ```

2. In the same file, add the tag of the language to the `Display` implementation, for example `[yl]`.

3. Add a variant to the `ProjectFilter` enum in `src/config/filter.rs`. Write a doc comment on the variant: Clap shows it in the help of `--project-type`.

4. In `src/scanner.rs`, write a detection function. The function returns a `Project` when the directory contains the marker file and at least one build directory:

   ```rust
   fn detect_your_language_project(path: &Path) -> Option<Project> {
       let marker = path.join("your_config.conf");
       let build_dir = path.join("build");

       if !marker.exists() || !build_dir.exists() {
           return None;
       }

       let name = Self::fallback_to_directory_name(path);

       Some(Project::new(
           ProjectType::YourLanguage,
           path.to_path_buf(),
           vec![BuildArtifacts { path: build_dir, size: 0 }],
           name,
       ))
   }
   ```

   The scanner calculates the sizes later. If the marker file contains a project name, write a function that reads it.

5. In `detect_project()` in `src/scanner.rs`, call your function through `try_detect(ProjectFilter::YourLanguage, ...)`.

6. If the scan must not go into some directories of the language, add them to `is_excluded_directory()` in `src/scanner.rs`.

7. Add the language to the `match` expressions that list all the types:

   - `icon_for_project_type()` in `src/project/projects.rs`: the tag.
   - `src/output.rs`: the type name in the JSON output.
   - `src/filtering.rs`: the position of the type for `--sort type`.
   - `src/executables.rs`: the list of types with no executables to keep, or a new function that keeps them.

   The compiler shows an error for each `match` that does not handle the new variant.

8. Add tests in `src/scanner.rs`. Use the tests of the Zig or Scala support as a model.

9. Add the language to [Project detection](project-detection.md), and to the list of values of `--project-type` in [Usage](usage.md) and [Command reference](command-reference.md).

## Test a new language

Make sure that your tests cover these cases:

- Each marker file, if the language has more than one.
- Each build directory, if the build tools use different names.
- A marker file with no name, or with a name that is not valid. The detection must not fail.
- The speed of the scan. The detection must not make the scan much slower.

## Pull request

In the description of your pull request:

1. Explain what the change does.
2. Give an example of the project structure for a new language.
3. Say how you tested the change on real projects.
