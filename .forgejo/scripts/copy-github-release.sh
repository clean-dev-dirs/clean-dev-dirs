#!/usr/bin/env bash
# Copy one release (notes and assets) from the GitHub mirror to Forgejo.
#
# Use this script one time per release that exists only on GitHub. The tag
# must already exist on Forgejo.
#
# Usage: FORGEJO_TOKEN=<token> copy-github-release.sh <tag>
# Example: FORGEJO_TOKEN=... .forgejo/scripts/copy-github-release.sh v2.9.1
#
# Requirements: gh (logged in to GitHub), jq, and curl.
set -euo pipefail

TAG="$1"
GITHUB_REPO="clean-dev-dirs/clean-dev-dirs"

export API="https://git.tomplanche.com/api/v1"
export REPO="clean-dev-dirs/clean-dev-dirs"
export TOKEN="${FORGEJO_TOKEN:?FORGEJO_TOKEN is not set}"

WORK="$(mktemp -d)"
trap 'rm -rf "${WORK}"' EXIT

echo "Downloading ${TAG} from GitHub"
gh release view "${TAG}" --repo "${GITHUB_REPO}" --json body --jq .body > "${WORK}/body.md"
gh release download "${TAG}" --repo "${GITHUB_REPO}" --dir "${WORK}/assets"

echo "Creating ${TAG} on Forgejo"
RELEASE_ID=$(jq -n --arg tag "${TAG}" --rawfile body "${WORK}/body.md" \
               '{tag_name: $tag, name: $tag, body: $body}' \
             | curl --silent --show-error --fail --request POST \
                 --header "Authorization: token ${TOKEN}" \
                 --header "Content-Type: application/json" \
                 --data @- "${API}/repos/${REPO}/releases" \
             | jq -r .id)
export RELEASE_ID

for f in "${WORK}"/assets/*; do
  "$(dirname "$0")/upload-asset.sh" "$f"
done

echo "Done: https://git.tomplanche.com/${REPO}/releases/tag/${TAG}"
