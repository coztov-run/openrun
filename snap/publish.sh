#!/usr/bin/env bash
# Публикация готового .snap в Snap Store.
# Запускать из любого места: bash snap/publish.sh
set -euo pipefail

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"

if ! command -v snapcraft >/dev/null 2>&1; then
    echo "Snapcraft не найден. Устанавливаю..."
    sudo snap install snapcraft --classic
    export PATH="$PATH:/snap/bin"
fi

if ! command -v snapcraft >/dev/null 2>&1; then
    echo "Snapcraft всё ещё не найден. Открой новый терминал или выполни:"
    echo "  set -gx PATH \$PATH /snap/bin"
    exit 1
fi

snapcraft --version
snapcraft login
snapcraft whoami

snap_file="openrun_0.1.1_amd64.snap"
echo "Скачиваю свежий $snap_file из GitHub Releases..."
curl -fL -o "$snap_file" \
    "https://github.com/coztov-run/openrun/releases/download/v0.1.1/$snap_file"

if ! snapcraft register openrun; then
    echo
    echo "Имя openrun уже зарегистрировано. Если оно твоё — продолжаем."
    echo "Если нет — имя придётся сменить в snap/snapcraft.yaml."
fi

if ! snapcraft upload --release=stable "$snap_file"; then
    echo
    echo "Store не принял snap. Для classic-снапов это обычно значит,"
    echo "что нужно ручное ревью. Оставь заявку здесь:"
    echo "https://forum.snapcraft.io/c/store-requests/classic-confinement/28"
    echo
    echo "После одобрения повтори:"
    echo "  snapcraft upload --release=stable $snap_file"
    exit 1
fi

echo
echo "Готово: snap загружен в Snap Store."
