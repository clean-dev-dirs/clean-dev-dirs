<pre>
        ▀▀█                                    █                           █    ▀                 
  ▄▄▄     █     ▄▄▄    ▄▄▄   ▄ ▄▄           ▄▄▄█   ▄▄▄   ▄   ▄          ▄▄▄█  ▄▄▄     ▄ ▄▄   ▄▄▄  
 █▀  ▀    █    █▀  █  ▀   █  █▀  █         █▀ ▀█  █▀  █  ▀▄ ▄▀         █▀ ▀█    █     █▀  ▀ █   ▀ 
 █        █    █▀▀▀▀  ▄▀▀▀█  █   █   ▀▀▀   █   █  █▀▀▀▀   █▄█    ▀▀▀   █   █    █     █      ▀▀▀▄ 
 ▀█▄▄▀    ▀▄▄  ▀█▄▄▀  ▀▄▄▀█  █   █         ▀█▄██  ▀█▄▄▀    █           ▀█▄██  ▄▄█▄▄   █     ▀▄▄▄▀ 
</pre>

<p align="center">
  <a href="https://crates.io/crates/clean-dev-dirs"><img src="https://img.shields.io/crates/v/clean-dev-dirs.svg" alt="Crates.io Version"></a>
  <a href="https://git.tomplanche.com/clean-dev-dirs/clean-dev-dirs/src/branch/main/LICENSE-MIT"><img src="https://img.shields.io/crates/l/clean-dev-dirs" alt="License"></a>
</p>

`clean-dev-dirs` is a fast CLI tool that finds your development projects and deletes their build directories to free disk space. It supports Rust, Node.js, Python, Go, Java/Kotlin, C/C++, Swift, .NET/C#, Ruby, Elixir, Deno, PHP, Haskell, Dart/Flutter, Zig, and Scala. It can also clean the global caches of package managers and build tools.

By default, the tool moves the directories to the system trash. Thus, you can restore them.

Created and maintained by [Tom Planche](https://tomplanche.com). The organization exists only to host the Homebrew tap next to the main repository.

## Install

```bash
cargo install clean-dev-dirs
```

With Homebrew:

```bash
brew install clean-dev-dirs/clean-dev-dirs/clean-dev-dirs
```

[Installation](https://git.tomplanche.com/clean-dev-dirs/clean-dev-dirs/src/branch/main/docs/installation.md) gives the other methods.

## Use

1. Preview the cleanup. This command deletes nothing:

    ```bash
    clean-dev-dirs ~/Projects --dry-run
    ```

2. Clean the projects. The tool asks for a confirmation first:

    ```bash
    clean-dev-dirs ~/Projects
    ```

More commands:

```bash
# Choose the projects in a list
clean-dev-dirs ~/Projects --interactive

# Clean only the Rust projects that you did not compile in the last 30 days
clean-dev-dirs ~/Projects -p rust --keep-days 30

# Clean the global caches of Cargo, npm, pnpm, Go, Maven, and others
clean-dev-dirs caches --dry-run
clean-dev-dirs caches
```

Run `clean-dev-dirs --help` to see all the options.

## Documentation

- [Usage](https://git.tomplanche.com/clean-dev-dirs/clean-dev-dirs/src/branch/main/docs/usage.md): filters, sort order, interactive selection, trash, and permanent deletion.
- [Global caches](https://git.tomplanche.com/clean-dev-dirs/clean-dev-dirs/src/branch/main/docs/global-caches.md): the `caches` subcommand.
- [Configuration](https://git.tomplanche.com/clean-dev-dirs/clean-dev-dirs/src/branch/main/docs/configuration.md): default values in a configuration file.
- [JSON output](https://git.tomplanche.com/clean-dev-dirs/clean-dev-dirs/src/branch/main/docs/json-output.md): results for scripts.
- [Command reference](https://git.tomplanche.com/clean-dev-dirs/clean-dev-dirs/src/branch/main/docs/command-reference.md): all the commands and options.
- [Project detection](https://git.tomplanche.com/clean-dev-dirs/clean-dev-dirs/src/branch/main/docs/project-detection.md): the supported project types, and what the tool deletes.
- [Sizes](https://git.tomplanche.com/clean-dev-dirs/clean-dev-dirs/src/branch/main/docs/sizes.md): size formats, and how the tool measures a directory.
- [Safety](https://git.tomplanche.com/clean-dev-dirs/clean-dev-dirs/src/branch/main/docs/safety.md): how the tool prevents unwanted deletions.
- [Contributing](https://git.tomplanche.com/clean-dev-dirs/clean-dev-dirs/src/branch/main/docs/contributing.md): add support for a new language.

## License

You can use this project under the [MIT License](https://git.tomplanche.com/clean-dev-dirs/clean-dev-dirs/src/branch/main/LICENSE-MIT) or the [Apache License 2.0](https://git.tomplanche.com/clean-dev-dirs/clean-dev-dirs/src/branch/main/LICENSE-APACHE), at your choice.

## Support

The primary repository is on [git.tomplanche.com](https://git.tomplanche.com/clean-dev-dirs/clean-dev-dirs), the self-hosted Forgejo instance of [Tom Planche](https://tomplanche.com). GitHub is a mirror.

- Issues: [Forgejo Issues](https://git.tomplanche.com/clean-dev-dirs/clean-dev-dirs/issues). If you cannot use Forgejo, use [GitHub Issues](https://github.com/clean-dev-dirs/clean-dev-dirs/issues).
- Crates.io: [clean-dev-dirs](https://crates.io/crates/clean-dev-dirs)
