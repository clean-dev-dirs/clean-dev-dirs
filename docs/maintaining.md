# Maintaining

This page is for the maintainers. It explains where the project lives, how a release works, and how to set up the infrastructure one time.

## Where the project lives

| Place | Role |
|-------|------|
| [git.tomplanche.com](https://git.tomplanche.com/clean-dev-dirs), the self-hosted Forgejo instance of [Tom Planche](https://tomplanche.com) | Primary forge: code, issues, pull requests, CI, releases |
| [GitHub](https://github.com/clean-dev-dirs) | Push mirror of Forgejo. It runs only one workflow: the macOS build of a release, because Forgejo has no macOS runner. Homebrew reads the tap from it. Its issues stay open as a backup for the people who cannot use Forgejo. |
| [crates.io](https://crates.io/crates/clean-dev-dirs) | The published crate |

Forgejo pushes each change to GitHub. Do not push to GitHub directly: the next sync of the push mirror replaces the GitHub branches and tags with the Forgejo ones.

## Release a new version

1. Change `version` in `Cargo.toml`, and run `cargo build` to update `Cargo.lock`.
2. Commit the change, open a pull request on Forgejo, and merge it into `main`.
3. Wait for the `Release` workflow on Forgejo. It does these steps:
   1. It creates the tag `v<version>` and the release, with the source archive.
   2. The Linux runner builds the Linux binary and its Homebrew bottle.
   3. It publishes the crate on crates.io.
   4. It waits for the macOS bottles. Then it opens a pull request on the Homebrew tap.
4. Merge the pull request on the Homebrew tap.

Forgejo has no macOS runner, so GitHub builds the macOS binaries:

1. The push mirror copies the new tag to GitHub.
2. On GitHub, the `macOS release build` workflow starts. It builds the Apple Silicon and Intel binaries and their bottles.
3. The GitHub workflow uploads these files to the release on Forgejo.

The last job on Forgejo waits 45 minutes at most for the macOS bottles. Then it fails. The limit is below 1 hour because the Linux runner stops a job after 1 hour.

The push mirror also copies the tap to GitHub. The `brew` command reads the tap from GitHub.

If a job on Forgejo fails after the release workflow created the tag, fix the cause. Then run the failed job again from the Actions page of Forgejo. Do not run the full workflow again: the tag exists, so the workflow detects no new version.

If the GitHub workflow fails, fix the cause. Then start the GitHub workflow again with the tag. The workflow replaces the macOS files of the release:

```bash
gh workflow run macos.yml --repo clean-dev-dirs/clean-dev-dirs -f tag=v<version>
```

The workflows are `.forgejo/workflows/release.yml` and `.github/workflows/macos.yml`. Forgejo does not run the files in `.github/workflows`, because the `.forgejo/workflows` directory exists.

## One-time setup

Do these steps in this order. The push mirror (step 5) replaces the content of GitHub with the content of Forgejo, so Forgejo must be complete first.

### 1. Copy the missing content from GitHub to Forgejo

1. In the Homebrew tap on GitHub, merge the open pull request for the latest version.

2. In a clone of each repository (`clean-dev-dirs` and `homebrew-clean-dev-dirs`), add a remote for each forge if it does not exist:

   ```bash
   git remote add forgejo https://git.tomplanche.com/clean-dev-dirs/<repository>.git
   ```

3. Get all the branches and tags from GitHub:

   ```bash
   git fetch origin --tags --prune
   ```

4. Push the tags to Forgejo:

   ```bash
   git push forgejo --tags
   ```

5. Push all the GitHub branches to Forgejo:

   ```bash
   git for-each-ref --format='%(refname:lstrip=3)' refs/remotes/origin | grep -v '^HEAD$' | while read -r b; do git push forgejo "refs/remotes/origin/$b:refs/heads/$b"; done
   ```

6. For each release that exists only on GitHub, copy the release and its assets to Forgejo. Run this command in the `clean-dev-dirs` repository:

   ```bash
   FORGEJO_TOKEN=<token> .forgejo/scripts/copy-github-release.sh v2.9.1
   ```

   The token is the Forgejo token of step 2 below.

### 2. Create the tokens and the secrets

1. In Forgejo, open **Settings > Applications** and create an access token with the `write:repository` scope. The token must have access to `clean-dev-dirs` and `homebrew-clean-dev-dirs`.
2. In crates.io, open **Account Settings > API Tokens** and create a token with the `publish-update` scope for `clean-dev-dirs`.
3. In the `clean-dev-dirs` repository on Forgejo, open **Settings > Actions > Secrets** and add 2 secrets:

   | Secret | Value |
   |--------|-------|
   | `RELEASE_TOKEN` | The Forgejo token |
   | `CRATES_TOKEN` | The crates.io token |

4. In GitHub, create a fine-grained personal access token for the push mirror:
   - Resource owner: the `clean-dev-dirs` organization.
   - Repositories: `clean-dev-dirs` and `homebrew-clean-dev-dirs`.
   - Permissions: **Contents** (read and write) and **Workflows** (read and write).

   GitHub refuses a push that changes `.github/workflows` without the **Workflows** permission. The first sync deletes the old GitHub workflows, so this permission is necessary.

### 3. Give GitHub access to the Forgejo releases

The GitHub workflow uploads the macOS files to the release on Forgejo. It needs a Forgejo token for this upload.

1. In Forgejo, open **Settings > Applications** and create a second access token with the `write:repository` scope. Use a token that is different from `RELEASE_TOKEN`: then you can revoke one token and keep the other.
2. Add the token to the secrets of the GitHub repository. The command asks for the value, so the token does not go into your shell history:

   ```bash
   gh secret set FORGEJO_TOKEN --repo clean-dev-dirs/clean-dev-dirs
   ```

### 4. Merge the Forgejo configuration

Open the pull requests that move the CI and the releases to Forgejo, on Forgejo, and merge them:

- In `clean-dev-dirs`: the branch `ci/forgejo-primary`.
- In `homebrew-clean-dev-dirs`: the branch `ci/forgejo-primary`.

### 5. Create the push mirrors

Do these steps for `clean-dev-dirs` and for `homebrew-clean-dev-dirs`:

1. On Forgejo, open **Settings > Repository > Mirror settings**.
2. In **Push mirrors**, set these values:
   - Git remote repository URL: `https://github.com/clean-dev-dirs/<repository>.git`
   - Username: your GitHub username.
   - Password: the GitHub token of step 2.
   - **Sync when commits are pushed**: on.
3. Click **Add push mirror**, then **Synchronize now**.
4. On GitHub, make sure that the branches and the tags are the same as on Forgejo.

### 6. Make GitHub a read-only mirror

1. Turn off the wiki and the projects. Keep the issues of `clean-dev-dirs`: the documentation gives them as a backup.

   ```bash
   for r in clean-dev-dirs homebrew-clean-dev-dirs; do gh repo edit "clean-dev-dirs/$r" --enable-wiki=false --enable-projects=false --homepage "https://git.tomplanche.com/clean-dev-dirs/$r"; done
   ```

   Turn off GitHub Actions on the tap only. The `clean-dev-dirs` repository needs GitHub Actions for the macOS build:

   ```bash
   gh api --method PUT repos/clean-dev-dirs/homebrew-clean-dev-dirs/actions/permissions -F enabled=false
   ```

   ```bash
   gh repo edit clean-dev-dirs/homebrew-clean-dev-dirs --enable-issues=false
   ```

   A pull request on GitHub cannot be merged on GitHub: the next sync replaces the GitHub branches. To accept it, fetch its branch and merge it into `main` on Forgejo without a rebase. When the mirror pushes the same commits to GitHub, GitHub marks the pull request as merged. If you change the commits (rebase, squash, or cherry-pick), close the GitHub pull request by hand.

2. Tell the visitors where the project lives:

   ```bash
   gh repo edit clean-dev-dirs/clean-dev-dirs --description "Mirror of https://git.tomplanche.com/clean-dev-dirs/clean-dev-dirs. Open issues and pull requests there."
   ```

   ```bash
   gh repo edit clean-dev-dirs/homebrew-clean-dev-dirs --description "Homebrew tap of clean-dev-dirs. Mirror of https://git.tomplanche.com/clean-dev-dirs/homebrew-clean-dev-dirs."
   ```

3. Delete the GitHub secrets that the old workflows used: `CRATES_TOKEN`, `PAT_TOKEN`, `CLEAN_DEV_DIRS_PRS_TOKEN`, and `SONAR_TOKEN`. Then revoke the tokens themselves on crates.io and GitHub.

   ```bash
   for s in CRATES_TOKEN PAT_TOKEN CLEAN_DEV_DIRS_PRS_TOKEN SONAR_TOKEN; do gh secret delete "$s" --repo clean-dev-dirs/clean-dev-dirs; done
   ```

   ```bash
   for s in PAT_TOKEN CLEAN_DEV_DIRS_PRS_TOKEN; do gh secret delete "$s" --repo clean-dev-dirs/homebrew-clean-dev-dirs; done
   ```

### 7. Download the bottles from Forgejo

The `root_url` of the formula must point to the release on Forgejo:

```ruby
bottle do
  root_url "https://git.tomplanche.com/clean-dev-dirs/clean-dev-dirs/releases/download/v<version>"
```

Homebrew adds the name of the bottle to this URL. The `update-tap` job changes the version in `root_url` at each release.

Forgejo counts the downloads of each file of a release. To see the counts, run this command:

```bash
curl -s https://git.tomplanche.com/api/v1/repos/clean-dev-dirs/clean-dev-dirs/releases/tags/v<version> | jq -r '.assets[] | "\(.name) \(.download_count)"'
```

### 8. Update your local clones

In each local clone, make Forgejo the `origin` remote:

```bash
git remote rename origin github
```

```bash
git remote rename forgejo origin
```

```bash
git branch --set-upstream-to=origin/main main
```

Then push only to `origin`. The push mirror updates GitHub.
