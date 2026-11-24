# clean-dev-dirs documentation

`clean-dev-dirs` finds development projects on your disk and deletes their build directories to free disk space. It supports 16 ecosystems. It can also clean the global caches of package managers and build tools.

## Pages

| Page | Content |
|------|---------|
| [Installation](installation.md) | Install the tool with Cargo, Homebrew, or from the source code |
| [Usage](usage.md) | Clean projects: directories, filters, sort order, interactive selection, trash |
| [Global caches](global-caches.md) | Clean the caches of Cargo, npm, pnpm, Yarn, Maven, Gradle, Go, pip, uv, and Xcode |
| [Configuration](configuration.md) | Set default values in a configuration file |
| [JSON output](json-output.md) | Get the results as JSON for scripts |
| [Command reference](command-reference.md) | All the commands and options |
| [Project detection](project-detection.md) | How the tool finds each type of project, and what it deletes |
| [Sizes](sizes.md) | The size formats, and how the tool measures a build directory |
| [Safety](safety.md) | How the tool prevents unwanted deletions |
| [Contributing](contributing.md) | Add support for a new language |
| [Maintaining](maintaining.md) | The forges, the release process, and the setup of the infrastructure |

## Inspiration

This project started from [cargo-clean-all](https://github.com/dnlmlr/cargo-clean-all), a tool that cleans only Rust projects. `clean-dev-dirs` adds these features:

- Support for 16 ecosystems, not only Rust.
- A parallel scan of the file system.
- More filters: size, age, name, and project type.
- The cleanup of global caches.

## Acknowledgments

The tool uses these open-source crates:

- [Clap](https://crates.io/crates/clap): command-line parsing.
- [Rayon](https://crates.io/crates/rayon): parallel scan and parallel deletion.
- [jwalk](https://crates.io/crates/jwalk): parallel walk of the directory tree.
- [WalkDir](https://crates.io/crates/walkdir): walk of a build directory to measure its size.
- [Colored](https://crates.io/crates/colored): colors in the terminal.
- [Indicatif](https://crates.io/crates/indicatif): progress bars and spinners.
- [Inquire](https://crates.io/crates/inquire): interactive prompts and lists.
- [Humansize](https://crates.io/crates/humansize): human-readable sizes.
- [Serde](https://crates.io/crates/serde), [serde_json](https://crates.io/crates/serde_json), and [TOML](https://crates.io/crates/toml): JSON output and the configuration file.
- [dirs](https://crates.io/crates/dirs): platform directories.
- [trash](https://crates.io/crates/trash): the system trash.
