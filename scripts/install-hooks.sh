#!/usr/bin/env bash
# Install git hooks from the repo into .git/hooks
# Run this once after cloning: ./scripts/install-hooks.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
HOOKS_SOURCE="${REPO_DIR}/hooks"
HOOKS_DEST="${REPO_DIR}/.git/hooks"

if [ ! -d "${HOOKS_SOURCE}" ]; then
    echo "ERROR: hooks/ directory not found at ${HOOKS_SOURCE}"
    exit 1
fi

for hook in "${HOOKS_SOURCE}"/*; do
    [ -f "${hook}" ] || continue
    name="$(basename "${hook}")"
    cp "${hook}" "${HOOKS_DEST}/${name}"
    chmod +x "${HOOKS_DEST}/${name}"
    echo "Installed: ${name}"
done

echo ""
echo "All hooks installed. pre-push will now enforce the verification sequence."
echo "To bypass (emergency only): git push --no-verify"
