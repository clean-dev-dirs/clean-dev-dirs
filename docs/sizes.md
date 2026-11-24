# Sizes

## Size formats

The `--keep-size` option accepts these formats:

| Format | Examples | Base |
|--------|----------|------|
| Decimal units | `100KB`, `1.5MB`, `2GB` | 1000 |
| Binary units | `100KiB`, `1.5MiB`, `2GiB` | 1024 |
| Bytes | `1000000` | Not applicable |

Examples:

```bash
clean-dev-dirs --keep-size 100KB    # 100 kilobytes
clean-dev-dirs --keep-size 1.5MB    # 1.5 megabytes
clean-dev-dirs --keep-size 2GiB     # 2 gibibytes
clean-dev-dirs --keep-size 500000   # 500,000 bytes
```

The tool shows all sizes in decimal units (base 1000).

## How the tool measures a directory

The size of a directory is the disk space that its deletion frees. It is not the apparent size of its files. The tool applies these rules:

- Allocated blocks (Unix): each file counts the blocks that it uses on the disk, like `du`. A sparse file counts only its allocated blocks. A small file counts a full block.
- Hard links in the directory: if a file has more than one hard link in the same directory, the tool counts the file one time.
- Hard links to files outside the directory: if a file also has a hard link outside the directory, the tool counts the file zero times. The data of this file stays on the disk after the deletion. On Linux, pnpm fills `node_modules` with hard links to its store, so this rule applies to pnpm projects.
- Symbolic links: the tool does not follow them.

## Limitations

- On Windows, the tool uses the apparent size of each file. It does not detect hard links.
- On macOS, pnpm uses APFS clones by default, not hard links. A clone shares its data with the original file, and the tool does not detect this shared data. Thus, the size of a `node_modules` directory or of the pnpm store can be larger than the space that its deletion frees.
