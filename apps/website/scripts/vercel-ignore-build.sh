#!/bin/sh

[ -n "${VERCEL_GIT_PREVIOUS_SHA:-}" ] || exit 1

if ! git cat-file -e "$VERCEL_GIT_PREVIOUS_SHA^{commit}" 2>/dev/null; then
  echo 'Previous deployment commit unavailable; building website.'
  exit 1
fi

git diff --quiet "$VERCEL_GIT_PREVIOUS_SHA" HEAD -- . ../../CHANGELOG.md && exit 0
exit 1
