# Usage

This page shows how to clean the build directories of your projects. To clean the global caches of package managers, read [Global caches](global-caches.md).

## How a cleanup works

When you run `clean-dev-dirs`, the tool does these steps:

1. It scans the directories that you give, and all their subdirectories.
2. It detects the projects and their build directories. [Project detection](project-detection.md) gives the rules for each type of project.
3. It applies your filters and your sort order.
4. It shows the projects and the total reclaimable space.
5. It asks for a confirmation.
6. It moves the build directories to the system trash.

## Choose the directories

```bash
# Scan the current directory
clean-dev-dirs

# Scan one directory
clean-dev-dirs ~/Projects

# Scan more than one directory
clean-dev-dirs ~/Projects ~/work/client ~/personal/code
```

If you give 2 directories that overlap, the tool lists each project only one time.

## Preview a cleanup

Use `--dry-run` to see the projects and the reclaimable space. The tool does not delete anything.

```bash
clean-dev-dirs ~/Projects --dry-run
```

Example output:

```
Found 15 projects

Found projects:
  [rs] 13 Rust projects (21.47 GB)
  [js] 2 Node.js projects (251.67 MB)
  Total reclaimable space: 21.72 GB

[dry-run] Complete. Would free up 21.72 GB
```

## Choose the projects in a list

Use `--interactive` (or `-i`) to select the projects in a list. All the projects are selected at the start.

```bash
clean-dev-dirs ~/Projects --interactive
```

## Filter by project type

Use `--project-type` (or `-p`) to clean only one type of project:

```bash
clean-dev-dirs -p rust
clean-dev-dirs -p node
```

The values are `all` (default), `rust`, `node`, `python`, `go`, `java`, `cpp`, `swift`, `dotnet`, `ruby`, `elixir`, `deno`, `php`, `haskell`, `dart`, `zig`, and `scala`.

## Filter by size

Use `--keep-size` (or `-s`) to ignore the projects with small build directories:

```bash
# Clean only the projects with more than 100 MB of build files
clean-dev-dirs --keep-size 100MB
```

[Sizes](sizes.md) gives the size formats.

## Filter by age

Use `--keep-days` (or `-d`) to ignore the projects that you compiled recently:

```bash
# Clean only the projects that you did not compile in the last 30 days
clean-dev-dirs --keep-days 30
```

The value 0 disables this filter.

## Filter by name

Use `--name` to clean only the projects with a matching name. The pattern is a glob by default. To use a regular expression, start the pattern with `regex:`.

```bash
# Glob: the names that start with "my-app"
clean-dev-dirs --name "my-app*"

# Glob: "app-" and one more character
clean-dev-dirs --name "app-?"

# Regular expression: the names that start with "client-"
clean-dev-dirs --name "regex:^client-"
```

## Sort the projects

Use `--sort` to change the order of the projects in the list:

| Value | Order |
|-------|-------|
| `size` | Largest first |
| `age` | Oldest first |
| `name` | Alphabetical |
| `type` | Grouped by project type |

Add `--reverse` to reverse the order:

```bash
# Smallest first
clean-dev-dirs --sort size --reverse
```

## Keep the compiled executables

Use `--keep-executables` (or `-k`) to copy the compiled outputs to `<project>/bin/` before the tool deletes the build directory:

```bash
clean-dev-dirs -p rust -k
```

The copy depends on the project type:

- Rust: the tool copies the executables from `target/release/` and `target/debug/` to `bin/release/` and `bin/debug/`.
- Python: the tool copies the `.whl` files from `dist/` and the `.so` and `.pyd` extensions from `build/` to `bin/`.
- Other types: the tool copies nothing. Their build directories contain dependencies or outputs that the tool cannot keep in a useful form.

If you use `--interactive` without `-k`, the tool asks if you want to keep the executables.

## Move to the trash or delete permanently

By default, the tool moves each build directory to the system trash (Trash on macOS and Linux, Recycle Bin on Windows). You can restore a directory from the trash.

```bash
# Default: move to the trash
clean-dev-dirs

# Delete permanently
clean-dev-dirs --permanent
```

WARNING: A permanent deletion cannot be undone. Use `--dry-run` first to see what the tool deletes.

The trash keeps the disk space until you empty it. To free the space at once, use `--permanent`. To make permanent deletion the default, read [Configuration](configuration.md).

## Skip the confirmation

Use `--yes` (or `-y`) to clean without a confirmation prompt:

```bash
clean-dev-dirs ~/Projects --keep-days 30 --yes
```

## Change the scan

| Option | Effect |
|--------|--------|
| `--threads <N>` (or `-t`) | Use N threads for the scan. The value 0 uses all the CPU cores. |
| `--verbose` (or `-v`) | Show the access errors that occur during the scan. |
| `--skip <DIR>` | Skip this directory during the scan. You can use this option more than one time. |
| `--ignore <DIR>` | Ignore this directory completely during the scan. You can use this option more than one time. |
| `--max-depth <N>` | Do not go deeper than N levels below each directory. The value 1 scans only the direct children. |

## Examples

```bash
# Clean the Rust projects that you did not compile in the last 90 days
clean-dev-dirs ~/Projects -p rust --keep-days 90

# Find the largest build directories, without a deletion
clean-dev-dirs ~/Projects --sort size --dry-run

# Choose the oldest projects first in a list
clean-dev-dirs ~/code --sort age --interactive

# Clean the Rust projects and keep their executables
clean-dev-dirs ~/Projects -p rust -k

# Delete permanently, without a prompt
clean-dev-dirs ~/Projects --permanent --yes
```
