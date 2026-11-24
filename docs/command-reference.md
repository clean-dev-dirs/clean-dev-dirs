# Command reference

```
clean-dev-dirs [OPTIONS] [DIRS]... [COMMAND]
```

| Command | Use |
|---------|-----|
| (none) | Clean the build directories of the projects. Read [Usage](usage.md). |
| `caches` | Clean the global caches. Read [Global caches](global-caches.md). |
| `config` | Manage the configuration file. Read [Configuration](configuration.md). |
| `help` | Show the help of the tool or of a command. |

Run `clean-dev-dirs --help` or `clean-dev-dirs <COMMAND> --help` to see the help in the terminal.

## Main command

### Arguments

| Argument | Description |
|----------|-------------|
| `[DIRS]...` | One or more directories to scan. The default is the current directory. If 2 directories contain the same project, the tool lists it only one time. |

### Project type

| Option | Short | Description |
|--------|-------|-------------|
| `--project-type <TYPE>` | `-p` | Clean only this type of project. The values are `all` (default), `rust`, `node`, `python`, `go`, `java`, `cpp`, `swift`, `dotnet`, `ruby`, `elixir`, `deno`, `php`, `haskell`, `dart`, `zig`, and `scala`. |

### Filters

| Option | Short | Description |
|--------|-------|-------------|
| `--keep-size <SIZE>` | `-s` | Ignore the projects with a build directory smaller than this size. [Sizes](sizes.md) gives the formats. |
| `--keep-days <DAYS>` | `-d` | Ignore the projects that you compiled in the last DAYS days. The value 0 disables this filter. |
| `--name <PATTERN>` | | Clean only the projects with a matching name. The pattern is a glob, or a regular expression that starts with `regex:`. |

### Sort order

| Option | Description |
|--------|-------------|
| `--sort <ORDER>` | Sort the projects. The values are `size` (largest first), `age` (oldest first), `name` (alphabetical), and `type` (grouped by type). The default is the order of the scan. |
| `--reverse` | Reverse the sort order. |

### Output

| Option | Description |
|--------|-------------|
| `--json` | Write the result as one JSON document. You cannot use it with `--interactive`. Read [JSON output](json-output.md). |

### Execution

| Option | Short | Description |
|--------|-------|-------------|
| `--yes` | `-y` | Do not ask for a confirmation. |
| `--dry-run` | | Show the projects and the reclaimable space. Do not delete anything. |
| `--interactive` | `-i` | Choose the projects in a list. |
| `--keep-executables` | `-k` | Copy the compiled executables to `<project>/bin/` before the cleanup. |
| `--permanent` | | Delete permanently. Do not move to the system trash. |

### Scan

| Option | Short | Description |
|--------|-------|-------------|
| `--threads <N>` | `-t` | Number of threads for the scan. The value 0 uses all the CPU cores (default). |
| `--verbose` | `-v` | Show the access errors that occur during the scan. |
| `--skip <DIR>` | | Skip this directory during the scan. You can use this option more than one time. |
| `--ignore <DIR>` | | Ignore this directory completely during the scan. You can use this option more than one time. |
| `--max-depth <N>` | | Maximum depth of the scan. The default is unlimited. |

## caches

```
clean-dev-dirs caches [OPTIONS]
```

| Option | Short | Description |
|--------|-------|-------------|
| `--only <KINDS>` | | Clean only these global caches. Separate the values with commas. The values are `cargo`, `npm`, `pnpm`, `yarn`, `maven`, `gradle`, `go`, `pip`, `uv`, and `xcode`. |
| `--dry-run` | | Show the global caches and the reclaimable space. Do not delete anything. |
| `--interactive` | `-i` | Choose the global caches in a list. |
| `--yes` | `-y` | Do not ask for a confirmation. |
| `--permanent` | | Delete permanently. Do not move to the system trash. |
| `--json` | | Write the result as one JSON document. |

## config

```
clean-dev-dirs config <COMMAND>
```

| Command | Description |
|---------|-------------|
| `config path` | Show the path of the configuration file on this platform. |
| `config show` | Show each setting with its value, or with its default value. |
| `config init` | Write a template of the configuration file. If the file exists, the command does not change it. |
