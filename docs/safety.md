# Safety

The tool deletes directories. This page lists the features that prevent unwanted deletions.

## Before the deletion

- Read-only scan: the scan never changes a file.
- Preview: `--dry-run` shows what the tool deletes, and deletes nothing.
- Confirmation prompt: the tool asks "Clean N projects?" before each deletion. The default answer is "no". Use `--yes` (or `-y`) to skip the prompt.
- Interactive selection: `--interactive` (or `-i`) lets you choose each project in a list.
- Filters: `--keep-days` skips the projects that you compiled recently. `--keep-size` skips the small build directories.

## During the deletion

- System trash: by default, the tool moves the directories to the system trash. You can restore them from the trash. Use `--permanent` to delete them permanently.
- Build directories only: the tool deletes only the build directories that [Project detection](project-detection.md) lists. It never deletes the source code or the marker files.
- Errors: if the tool cannot delete a directory, it shows the error and continues with the other directories.

## Global caches

The `caches` subcommand has more protections:

- It deletes only the cache directories, never the configuration files, the installed binaries, or the credentials.
- It refuses the root of the file system and your home directory.
- It resolves symbolic links before it deletes a global cache.
- Before the confirmation prompt, it tells you to close the IDEs and stop the builds.

Read [Global caches](global-caches.md).

## Output

The tool shows colored output with a tag for each type of project, for example `[rs]` for Rust. [Project detection](project-detection.md) gives the list of tags.
