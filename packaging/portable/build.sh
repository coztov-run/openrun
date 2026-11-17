#!/bin/sh
# Собирает переносимый tar.gz: бинарник, ярлык, иконка, установщик.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
cd "$ROOT"

VERSION=${VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)}
ARCH=${ARCH:-$(uname -m)}
NAME="openrun-$VERSION-$ARCH"
STAGE="dist/$NAME"

cargo build --release --locked

rm -rf "$STAGE"
install -d "$STAGE"

install -m 755 target/release/openrun "$STAGE/openrun"
install -m 644 data/io.github.coztov_run.OpenRun.desktop "$STAGE/openrun.desktop"
install -m 644 data/io.github.coztov_run.OpenRun.metainfo.xml "$STAGE/openrun.metainfo.xml"
install -m 644 data/icons/hicolor/scalable/apps/io.github.coztov_run.OpenRun.svg "$STAGE/openrun.svg"
install -m 755 packaging/portable/install.sh "$STAGE/install.sh"
install -m 755 packaging/portable/uninstall.sh "$STAGE/uninstall.sh"
install -m 644 packaging/portable/README.txt "$STAGE/README.txt"
if [ -f LICENSE ]; then
    install -m 644 LICENSE "$STAGE/LICENSE"
fi

tar -czf "dist/$NAME.tar.gz" -C "$STAGE" .
(
    cd dist
    sha256sum "$NAME.tar.gz" > SHA256SUMS
)

echo "Готово: dist/$NAME.tar.gz"
cat "dist/SHA256SUMS"
