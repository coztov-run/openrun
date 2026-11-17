#!/bin/sh
# Удаляет установленные файлы OpenRun. Настройки и префиксы не трогает.
set -eu

PREFIX=${PREFIX:-"$HOME/.local"}
BIN_DIR="$PREFIX/bin"
APPS_DIR="$PREFIX/share/applications"
ICON_DIR="$PREFIX/share/icons/hicolor/scalable/apps"

rm -f "$BIN_DIR/openrun"
rm -f "$APPS_DIR/openrun.desktop"
rm -f "$ICON_DIR/openrun.svg" "$ICON_DIR/io.github.coztov_run.OpenRun.svg"
rm -f "$PREFIX/share/metainfo/io.github.coztov_run.OpenRun.metainfo.xml"

for desktop in "$HOME/Desktop" "$HOME/Рабочий стол"; do
    rm -f "$desktop/openrun.desktop"
done

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APPS_DIR" >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" >/dev/null 2>&1 || true
fi

echo "Файлы OpenRun удалены из $PREFIX."
echo "Настройки, префиксы и скачанный Wine оставлены в ~/.config/openrun и ~/.local/share/openrun."
