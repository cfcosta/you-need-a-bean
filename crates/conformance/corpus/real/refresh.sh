#!/usr/bin/env bash
# Re-copy the example ledgers, then re-bless the goldens.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$here/../../../.."
for f in basic starter vesting; do
  cp "$root/examples/$f.beancount" "$here/$f.beancount"
done
head -400 "$root/examples/example.beancount" > "$here/example-head.beancount"
BLESS=1 cargo test -p you-need-a-bean-conformance --test corpus
