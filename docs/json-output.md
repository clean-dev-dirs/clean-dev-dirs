# JSON output

Use `--json` to get the result as one JSON document. You can then use the result in a script, send it to `jq`, or show it in a dashboard.

## Rules

When `--json` is on:

- The tool writes only the JSON document to stdout. It shows no colors and no progress bars.
- The tool does not ask for a confirmation.
- You cannot use `--interactive`.

WARNING: Without `--dry-run`, `--json` cleans the projects at once, with no confirmation prompt. To only read the results, add `--dry-run`.

## Examples

```bash
# List all the projects as JSON, without a deletion
clean-dev-dirs --json --dry-run

# Clean, and get the results as JSON
clean-dev-dirs --json --yes ~/Projects

# Show the projects with more than 1 GB of build files
clean-dev-dirs --json --dry-run | jq '.projects[] | select(.build_artifacts_size > 1000000000)'

# Show the reclaimable space of the Rust projects
clean-dev-dirs --json --dry-run -p rust | jq '.summary.total_size_formatted'
```

## Projects: dry run

```json
{
  "mode": "dry_run",
  "projects": [
    {
      "name": "my-rust-app",
      "type": "rust",
      "root_path": "/home/user/projects/rust-app",
      "build_artifacts_paths": ["/home/user/projects/rust-app/target"],
      "build_artifacts_size": 2300000000,
      "build_artifacts_size_formatted": "2.30 GB"
    },
    {
      "name": "web-frontend",
      "type": "node",
      "root_path": "/home/user/projects/web-app",
      "build_artifacts_paths": ["/home/user/projects/web-app/node_modules"],
      "build_artifacts_size": 856000000,
      "build_artifacts_size_formatted": "856.00 MB"
    }
  ],
  "summary": {
    "total_projects": 2,
    "total_size": 3156000000,
    "total_size_formatted": "3.16 GB",
    "by_type": {
      "node": { "count": 1, "size": 856000000, "size_formatted": "856.00 MB" },
      "rust": { "count": 1, "size": 2300000000, "size_formatted": "2.30 GB" }
    }
  }
}
```

The `name` field is `null` when the tool cannot find the name of the project.

## Projects: cleanup

After a cleanup, the `mode` is `"cleanup"` and the document has one more object, `cleanup`:

```json
{
  "mode": "cleanup",
  "projects": [ "..." ],
  "summary": { "..." },
  "cleanup": {
    "success_count": 2,
    "failure_count": 0,
    "total_freed": 3156000000,
    "total_freed_formatted": "3.16 GB",
    "errors": []
  }
}
```

## Global caches

The `caches` subcommand also accepts `--json`. Its document has a `caches` list in place of the `projects` list:

```json
{
  "mode": "dry_run",
  "caches": [
    {
      "kind": "cargo",
      "path": "/home/user/.cargo/registry",
      "size": 4884488192,
      "size_formatted": "4.88 GB"
    }
  ],
  "summary": {
    "total_caches": 1,
    "total_size": 4884488192,
    "total_size_formatted": "4.88 GB"
  }
}
```

After a cleanup, the document has the same `cleanup` object as above.
