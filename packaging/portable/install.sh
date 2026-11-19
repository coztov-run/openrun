#!/bin/sh
# Устанавливает OpenRun в ~/.local (или в PREFIX) без пакетного менеджера.
set -eu

HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
PREFIX=${PREFIX:-"$HOME/.local"}

BIN_DIR="$PREFIX/bin"
APPS_DIR="$PREFIX/share/applications"
ICON_DIR="$PREFIX/share/icons/hicolor/scalable/apps"

install -d "$BIN_DIR" "$APPS_DIR" "$ICON_DIR"
install -m 755 "$HERE/openrun" "$BIN_DIR/openrun"
install -m 644 "$HERE/openrun.svg" "$ICON_DIR/openrun.svg"
install -m 644 "$HERE/openrun.svg" "$ICON_DIR/io.github.coztov_run.OpenRun.svg"
install -Dm644 "$HERE/openrun.metainfo.xml" \
    "$PREFIX/share/metainfo/io.github.coztov_run.OpenRun.metainfo.xml" 2>/dev/null || true

# Абсолютный путь нужен, если ~/.local/bin отсутствует в PATH.
sed "s|^Exec=openrun|Exec=\"$BIN_DIR/openrun\"|" \
    "$HERE/openrun.desktop" > "$APPS_DIR/openrun.desktop"
chmod 644 "$APPS_DIR/openrun.desktop"

# Необязательная копия на рабочий стол.
for desktop in "$HOME/Desktop" "$HOME/Рабочий стол"; do
    if [ -d "$desktop" ]; then
        cp "$APPS_DIR/openrun.desktop" "$desktop/openrun.desktop"
        chmod 755 "$desktop/openrun.desktop"
    fi
done

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APPS_DIR" >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" >/dev/null 2>&1 || true
fi

# Регистрируем обработчики .exe/.msi/.deb. Если xdg-mime нет, OpenRun
# сделает это сам при первом запуске.
MIMES="application/x-msdownload application/x-ms-dos-executable application/vnd.microsoft.portable-executable application/x-msi application/vnd.debian.binary-package application/vnd.flatpak application/x-flatpak application/vnd.android.package-archive"
if command -v xdg-mime >/dev/null 2>&1; then
    for mime in $MIMES; do
        XDG_DATA_DIRS="$PREFIX/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}" \
            xdg-mime default openrun.desktop "$mime" >/dev/null 2>&1 || true
    done
elif command -v gio >/dev/null 2>&1; then
    for mime in $MIMES; do
        gio mime "$mime" openrun.desktop >/dev/null 2>&1 || true
    done
fi

echo "OpenRun установлен: $BIN_DIR/openrun"
echo "Запустите его один раз, чтобы создать ярлыки и завершить настройку:"
echo "  $BIN_DIR/openrun"
