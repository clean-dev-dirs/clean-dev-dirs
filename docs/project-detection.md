# Project detection

The tool detects a project when a directory contains 2 things:

- A marker file of the ecosystem, for example `Cargo.toml` or `package.json`.
- At least one build directory of that ecosystem, for example `target/` or `node_modules/`.

The tool deletes only the build directories. It never deletes the source code or the marker files.

For each project, the tool also tries to find the project name in the marker file. If it cannot find a name, it uses the name of the directory.

## Summary

| Tag | Type | Marker files | Deleted directories |
|-----|------|--------------|---------------------|
| `[rs]` | Rust | `Cargo.toml` | `target/` |
| `[js]` | Node.js | `package.json` | `node_modules/` |
| `[py]` | Python | `requirements.txt`, `setup.py`, `pyproject.toml`, `setup.cfg`, `Pipfile`, `pipenv.lock`, `poetry.lock` | Cache and build directories (see below) |
| `[go]` | Go | `go.mod` | `vendor/` |
| `[java]` | Java/Kotlin | `pom.xml`, `build.gradle`, `build.gradle.kts` | `target/` (Maven) or `build/` (Gradle) |
| `[cpp]` | C/C++ | `CMakeLists.txt` or `Makefile` | `build/` |
| `[swift]` | Swift | `Package.swift` | `.build/` |
| `[net]` | .NET/C# | A `.csproj` file | `bin/` and `obj/` |
| `[rb]` | Ruby | `Gemfile` | `.bundle/` and `vendor/bundle/` |
| `[ex]` | Elixir | `mix.exs` | `_build/` |
| `[deno]` | Deno | `deno.json` or `deno.jsonc` | `vendor/` or `node_modules/` |
| `[php]` | PHP | `composer.json` | `vendor/` |
| `λ` | Haskell | `stack.yaml`, `cabal.project`, or a `.cabal` file | `.stack-work/` (Stack) or `dist-newstyle/` (Cabal) |
| `[dart]` | Dart/Flutter | `pubspec.yaml` | `.dart_tool/` and `build/` |
| `[zig]` | Zig | `build.zig` | `zig-cache/` and `zig-out/` |
| `[scala]` | Scala | `build.sbt` | `target/` |

The tag is the label that the tool shows in front of each project.

## Details per type

### Rust

- Detection: `Cargo.toml` and `target/`.
- Workspaces: the members of a Cargo workspace share the `target/` of the workspace root. The tool detects only the workspace root and skips the members.
- Name: `[package] name` in `Cargo.toml`.

### Node.js

- Detection: `package.json` and `node_modules/`.
- Name: the `name` field in `package.json`.

### Python

- Detection: at least one marker file and at least one cache or build directory.
- Deleted directories: all the directories in this list that exist:
  - `__pycache__`
  - `.pytest_cache`
  - `venv` and `.venv`
  - `build` and `dist`
  - `.eggs`, `.tox`, and `.coverage`
  - The `*.egg-info` directories in the project root
- Name: the project name in `pyproject.toml` (`[project]` or `[tool.poetry]`), or in `setup.py`.

### Go

- Detection: `go.mod` and `vendor/`.
- Name: the module path in `go.mod`.

### Java/Kotlin

- Detection with Maven: `pom.xml` and `target/`.
- Detection with Gradle: `build.gradle` or `build.gradle.kts`, and `build/`.
- Name: `<artifactId>` in `pom.xml`, or `rootProject.name` in `settings.gradle`.

### C/C++

- Detection: `CMakeLists.txt` or `Makefile`, and `build/`.
- Name: `project()` in `CMakeLists.txt`.

### Swift

- Detection: `Package.swift` and `.build/`.
- Name: `name:` in `Package.swift`.

### .NET/C#

- Detection: at least one `.csproj` file, and `bin/` or `obj/`.
- Name: the name of the `.csproj` file.

### Ruby

- Detection: `Gemfile`, and `.bundle/` or `vendor/bundle/`.
- Name: the `name` field in a `.gemspec` file.

### Elixir

- Detection: `mix.exs` and `_build/`.
- Name: the `app:` atom in `mix.exs`.

### Deno

- Detection: `deno.json` or `deno.jsonc`, and `vendor/`. As a second rule, the tool also detects `node_modules/` when there is no `package.json`.
- Name: the `name` field in `deno.json` or `deno.jsonc`.

### PHP

- Detection: `composer.json` and `vendor/`.
- Name: the `name` field in `composer.json`. The tool keeps the part after `/`.

### Haskell

- Detection with Stack: `stack.yaml` and `.stack-work/`.
- Detection with Cabal: `cabal.project` or a `.cabal` file, and `dist-newstyle/`.
- Name: the `name:` field in a `.cabal` file, then in `package.yaml`.

### Dart/Flutter

- Detection: `pubspec.yaml`, and `.dart_tool/` or `build/`.
- Name: the `name:` field in `pubspec.yaml`.

### Zig

- Detection: `build.zig`, and `zig-cache/` or `zig-out/`.
- Name: the name of the directory. `build.zig` has no standard name field.

### Scala

- Detection: `build.sbt` and `target/`.
- Name: the `name := "..."` line in `build.sbt`.
