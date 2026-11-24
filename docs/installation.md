# Installation

You can install `clean-dev-dirs` in 3 ways. All 3 ways give the same `clean-dev-dirs` command.

## From crates.io

This method is the recommended method. It is necessary to have Rust and Cargo on your computer.

```bash
cargo install clean-dev-dirs
```

To update the tool, run the same command again.

## With Homebrew (macOS and Linux)

```bash
brew install clean-dev-dirs/clean-dev-dirs/clean-dev-dirs
```

## From the source code

1. Clone the repository from Forgejo:

   ```bash
   git clone https://git.tomplanche.com/clean-dev-dirs/clean-dev-dirs.git
   ```

   If you cannot use Forgejo, clone the GitHub mirror:

   ```bash
   git clone https://github.com/clean-dev-dirs/clean-dev-dirs.git
   ```

2. Go into the repository:

   ```bash
   cd clean-dev-dirs
   ```

3. Build and install the tool:

   ```bash
   cargo install --path .
   ```

## Requirements

To build the tool, you need:

- A Rust toolchain that supports the Rust 2024 edition.
- Cargo.

## Check the installation

Run this command:

```bash
clean-dev-dirs --version
```

The command shows the installed version.

## Next step

Read [Usage](usage.md).
