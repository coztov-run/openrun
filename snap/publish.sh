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

snap_file="openrun_0.1.0_amd64.snap"
if [ ! -f "$snap_file" ]; then
    echo "Скачиваю $snap_file из GitHub Releases..."
    curl -L -o "$snap_file" \
        "https://github.com/coztov-run/openrun/releases/download/v0.1.0/$snap_file"
fi

if ! snapcraft register openrun; then
    echo
    echo "Имя openrun уже зарегистрировано. Если оно твоё — продолжаем."
    echo "Если нет — имя придётся сменить в snap/snapcraft.yaml."
fi

snapcraft upload --release=stable "$snap_file"

echo
echo "Готово. Если Store ответил, что нужен classic confinement,"
echo "оставь заявку здесь: https://forum.snapcraft.io/c/store-requests"
echo "После одобрения повтори: snapcraft upload --release=stable $snap_file"
