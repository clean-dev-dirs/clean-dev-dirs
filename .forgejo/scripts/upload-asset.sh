#!/usr/bin/env bash
# Upload one file to a release of the Forgejo repository.
#
# Usage: upload-asset.sh <file>
#
# Environment:
#   API         Base URL of the Forgejo API, for example https://git.tomplanche.com/api/v1
#   REPO        Repository, for example clean-dev-dirs/clean-dev-dirs
#   RELEASE_ID  ID of the release
#   TOKEN       Forgejo access token with write access to the repository
#
# The script uses only bash and curl, so it also runs on the macOS runner.
set -euo pipefail

FILE="$1"
NAME="$(basename "${FILE}")"

: "${API:?API is not set}"
: "${REPO:?REPO is not set}"
: "${RELEASE_ID:?RELEASE_ID is not set}"
: "${TOKEN:?TOKEN is not set}"

echo "Uploading ${NAME} to release ${RELEASE_ID}"
curl --silent --show-error --fail \
  --request POST \
  --header "Authorization: token ${TOKEN}" \
  --form "attachment=@${FILE}" \
  "${API}/repos/${REPO}/releases/${RELEASE_ID}/assets?name=${NAME}" \
  > /dev/null
