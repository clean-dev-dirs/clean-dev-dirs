# Configuration

You can write default values in a configuration file. Then you do not have to type the same options each time. An option on the command line always has priority over the configuration file.

## Location of the configuration file

| Platform | Path |
|----------|------|
| Linux and macOS | `~/.config/clean-dev-dirs/config.toml` |
| Windows | `%APPDATA%\clean-dev-dirs\config.toml` |

## Create the configuration file

1. Write a template of the configuration file:

   ```bash
   clean-dev-dirs config init
   ```

   The template contains all the settings as comments. If the file exists, the command does not change it.

2. Show the path of the configuration file:

   ```bash
   clean-dev-dirs config path
   ```

3. Open the file in your editor. Remove the `#` in front of each setting that you want to change, and set its value.

4. Show the values that the tool uses:

   ```bash
   clean-dev-dirs config show
   ```

   The command shows each setting with its value, or with its default value.

## Example

```toml
# Default project type
project_type = "rust"

# Default directories to scan. The tool expands "~".
dirs = ["~/Projects", "~/work/client"]

# Old setting for a single directory. The tool ignores it when "dirs" is set.
# dir = "~/Projects"

[filtering]
keep_size = "50MB"
keep_days = 7
sort = "size"             # "size", "age", "name", or "type"
reverse = false
# name_pattern = "my-*"   # glob, or "regex:^prefix-.*"

[scanning]
threads = 4
verbose = true
skip = [".cargo", "vendor"]
ignore = [".git"]
max_depth = 5             # Remove this line for an unlimited depth

[execution]
keep_executables = true
interactive = false
dry_run = false
use_trash = true          # Set to false for permanent deletion
```

All the settings are optional. Set only the settings that you need.

If the configuration file does not exist, the tool uses the default values. If the configuration file is not valid TOML, the tool shows an error message.

## Priority rules

The tool combines the command line and the configuration file with these rules:

| Type of value | Rule |
|---------------|------|
| Single value (`keep_size`, `threads`, `project_type`, `sort`) | The command line has priority. If it does not give the value, the tool uses the configuration file. If neither gives it, the tool uses the default value. |
| Directories (`dirs`, `dir`) | The command line has priority. Then the tool uses `dirs`, then `dir`, then the current directory. |
| On/off flag (`--dry-run`, `--verbose`, `--reverse`) | The flag is on if the command line gives it, or if the configuration file sets it to `true`. |
| List (`skip`, `ignore`) | The tool merges the 2 lists: first the values of the configuration file, then the values of the command line. |

Examples with the configuration file above:

```bash
# Uses keep_size = "50MB" from the file, but cleans Node.js projects
clean-dev-dirs -p node

# The command line value 200MB replaces the file value 50MB
clean-dev-dirs --keep-size 200MB

# Skips .cargo, vendor, and node_modules
clean-dev-dirs --skip node_modules
```

## Make permanent deletion the default

Set `use_trash` to `false`:

```toml
[execution]
use_trash = false
```

WARNING: With this setting, each cleanup deletes the directories permanently. You cannot restore them from the trash.

## Configuration and global caches

The `caches` subcommand uses only 2 settings of the configuration file: `use_trash` and `dry_run`. Read [Global caches](global-caches.md).
