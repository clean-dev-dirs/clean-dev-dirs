# Global caches

Package managers and build tools keep large caches outside of your projects. These global caches contain downloaded crates, npm packages, Maven artifacts, Go modules, and Xcode build products. They are often larger than all the build directories of your projects together.

The `caches` subcommand finds these global caches, measures them, and cleans them.

## Preview the global caches

Run this command first:

```bash
clean-dev-dirs caches --dry-run
```

The command shows each global cache, its size, and the total reclaimable space. It does not delete anything.

## Clean the global caches

```bash
# Clean all the global caches, with a confirmation prompt
clean-dev-dirs caches

# Clean only some global caches
clean-dev-dirs caches --only cargo,npm

# Choose the global caches in a list
clean-dev-dirs caches -i

# Delete permanently, without a confirmation prompt
clean-dev-dirs caches --permanent -y

# Get the result as JSON
clean-dev-dirs caches --dry-run --json
```

Before you clean a global cache, close the IDEs and stop the builds that use it. A build that runs during the cleanup can fail.

## Supported global caches

| Kind | Directories |
|------|-------------|
| `cargo` | `$CARGO_HOME/registry` and `$CARGO_HOME/git`. The default `$CARGO_HOME` is `~/.cargo`. |
| `npm` | `~/.npm/_cacache`, or `$npm_config_cache/_cacache` |
| `pnpm` | The pnpm store: `~/Library/pnpm/store`, `~/.local/share/pnpm/store`, or `%LOCALAPPDATA%\pnpm\store`. If `$npm_config_store_dir` is set, the tool uses it. |
| `yarn` | The Yarn classic cache and `~/.yarn/berry/cache`, or `$YARN_CACHE_FOLDER` |
| `maven` | `~/.m2/repository` |
| `gradle` | `$GRADLE_USER_HOME/caches` and `$GRADLE_USER_HOME/wrapper/dists`. The default `$GRADLE_USER_HOME` is `~/.gradle`. |
| `go` | The module cache (`$GOMODCACHE`, default `~/go/pkg/mod`) and the build cache (`$GOCACHE`) |
| `pip` | The pip cache: `~/Library/Caches/pip` or `~/.cache/pip`, or `$PIP_CACHE_DIR` |
| `uv` | The uv cache: `~/.cache/uv`, or `$UV_CACHE_DIR` |
| `xcode` | `~/Library/Developer/Xcode/DerivedData` (macOS only) |

When an environment variable moves a global cache, the tool uses the variable and ignores the default locations.

The tool does not show the global caches that are empty.

## What the tool never deletes

Each tool downloads or builds its global cache again when it needs it. Thus, the deletion of a global cache is safe.

The `caches` subcommand deletes only the directories in the table above. It never deletes configuration files, installed binaries, or credentials. For example:

- It deletes `~/.cargo/registry`, but never `~/.cargo/bin`.
- It deletes `~/.m2/repository`, but never `~/.m2/settings.xml`.

The tool also refuses the root of the file system and your home directory, even if an environment variable points to them.

## Trash and permanent deletion

By default, the tool moves the global caches to the system trash. The trash keeps the disk space until you empty it. To free the space at once, use `--permanent`.

WARNING: A permanent deletion cannot be undone. Use `--dry-run` first to see what the tool deletes.

The `use_trash` and `dry_run` values of the [configuration file](configuration.md) also apply to the `caches` subcommand.

## Read-only Go modules

The Go module cache contains read-only directories. A normal deletion of these directories fails. With `--permanent`, the tool makes the directories writable and then deletes them.

## Limitation on macOS

On macOS, pnpm copies its packages into your projects as APFS clones. A clone shares its data with the original file. Thus, the size that the tool shows for the pnpm store can be larger than the disk space that its deletion frees.

## Options

[Command reference](command-reference.md#caches) gives all the options of the `caches` subcommand.
