#!/bin/sh
# Собирает архив для ручной отправки в Flathub.
# Запускать из корня репозитория: sh packaging/flathub/prepare.sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
out="$root/dist/flathub-submission"

rm -rf "$out"
mkdir -p "$out"

cp "$root/packaging/flathub/io.github.coztov_run.OpenRun.yml" "$out/"
cp "$root/cargo-sources.json" "$out/"
cp "$root/packaging/flathub/flathub.json" "$out/"
cp "$root/packaging/flathub/README.md" "$out/README-ru.md"
cp "$root/README.en.md" "$out/APP-README-en.md"

cat > "$out/HOWTO.txt" <<'EOF'
Flathub submission bundle for OpenRun
=====================================

Files to put in the root of your flathub/flathub fork:

  io.github.coztov_run.OpenRun.yml
  cargo-sources.json
  flathub.json

Steps:

1. Fork https://github.com/flathub/flathub with all branches.
2. Clone the new-pr branch:
     git clone --branch=new-pr https://github.com/YOUR_NAME/flathub.git
     cd flathub
     git checkout -b io.github.coztov_run.OpenRun
3. Copy the three files above into the repository root.
4. Commit and push the branch.
5. Open a pull request against the new-pr branch, not master.
   Title: Add io.github.coztov_run.OpenRun
6. Wait for the Flathub build and review.

Do not submit this automatically through a bot or AI agent: Flathub rules
require the app author to open and write the submission manually.

The manifest points at tag v0.1.1. Check the commit with:

  git rev-parse v0.1.1
EOF

tar -czf "$root/dist/flathub-submission.tar.gz" -C "$root/dist" flathub-submission
echo "Готово: dist/flathub-submission.tar.gz"
sha256sum "$root/dist/flathub-submission.tar.gz"
