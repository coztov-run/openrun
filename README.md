# OpenRun

[English version](README.en.md)

Менеджер Windows-программ и префиксов с интерфейсом в стиле GNOME, на Rust,
GTK4 и libadwaita. OpenRun работает на собственном встроенном рантайме —
устанавливать Wine в систему не нужно.

## Возможности

- **Встроенный OpenRun Runtime**: приложение само скачивает портативную сборку
  Wine в свой каталог данных. Если сборки ещё нет, временно используется
  системный Wine (если он есть).
- **Двойной клик по `.exe` / `.msi` / `.deb`**: файловый менеджер открывает
  OpenRun, приложение само создаёт префикс (если его нет), запускает или
  устанавливает программу и добавляет ярлык. Открывать OpenRun заранее не нужно.
- **Перетаскивание (Drag & Drop)**: перетащите `.exe` / `.msi` или `.deb` в
  любое место страницы или на зону с «+» по центру.
- **Префиксы**: создание, запуск `.exe`/`.msi`, настройки, удаление из списка
  (файлы на диске не трогаются).
- **Программы**: запущенные `.exe` запоминаются; их можно снова запустить,
  создать ярлык или убрать из списка.
- **Пакеты `.deb`**: установка локального файла на Arch, CachyOS, Debian,
  Ubuntu и других дистрибутивах. Пакет распаковывается (ar разбирается самим
  приложением, нужен только `tar`/`xz`), `.desktop`-файлы переписываются на
  новые пути и появляются в меню приложений, иконки копируются в тему.
- **Настройки**: тема (системная, светлая, тёмная), язык (русский, английский),
  кнопка «Обновить» для пересоздания ярлыков и кешей значков.
- **Ярлыки**: при запуске создаётся ярлык OpenRun, для Windows-программ —
  ярлыки в меню и на рабочем столе. OpenRun назначается обработчиком `.exe`,
  `.msi` и `.deb`; прежние ассоциации сохраняются как запасной вариант.
- **Заголовок окна**: только кнопка закрытия, без «свернуть» и «развернуть».

## Установка

### Flatpak (рекомендуется для любого дистрибутива)

После публикации в Flathub:

```bash
flatpak install flathub io.github.coztov_run.OpenRun
flatpak run io.github.coztov_run.OpenRun
```

Локальная сборка или готовый `.flatpak` из GitHub Releases:

```bash
flatpak install --user openrun-0.1.0-x86_64.flatpak
```

Flatpak приносит собственный рантайм, поэтому работает на Ubuntu 22.04+,
Debian 12+, Fedora, Arch, openSUSE и других дистрибутивах.

### Snap

После публикации в Snap Store:

```bash
sudo snap install openrun --classic
openrun
```

Локальная сборка:

```bash
sudo snap install snapcraft --classic
snapcraft pack --destructive-mode
sudo snap install ./openrun_0.1.0_amd64.snap --classic --dangerous
```

Snap использует `confinement: classic`, потому что создаёт ярлыки в домашнем
каталоге, регистрирует MIME-обработчики и запускает Wine. Публикация classic
snap требует ручного ревью Snap Store; подробности —
[`snap/README.md`](snap/README.md).

### Переносимый tar.gz

Готовый архив лежит в `dist/openrun-0.1.0-x86_64.tar.gz` после сборки, а также
прикладывается к GitHub Release:

```bash
tar -xzf openrun-0.1.0-x86_64.tar.gz
cd openrun-0.1.0-x86_64
./install.sh
~/.local/bin/openrun
```

Требования: Linux x86_64, GTK 4.12+, libadwaita 1.5+, glibc 2.39+
(Arch, CachyOS, Fedora 41+, Ubuntu 24.10+). Для старых дистрибутивов
используйте Flatpak. Установщик кладёт файлы в `~/.local`, создаёт ярлык,
регистрирует MIME-обработчики и обновляет кеши, если утилиты установлены.

Удаление:

```bash
./uninstall.sh
```

### Из исходников

```bash
cargo build --release
cargo run
```

При первом запуске приложение скачает рантайм (~100 МБ) в
`~/.local/share/openrun/runtime` и дальше будет работать офлайн.
Настройки и префиксы хранятся в `~/.config/openrun` и
`~/.local/share/openrun/prefixes`. Данные прежних версий переносятся
автоматически.

## Двойной клик и ярлыки

OpenRun регистрирует MIME-типы:

- `application/x-msdownload`
- `application/x-ms-dos-executable`
- `application/vnd.microsoft.portable-executable`
- `application/x-msi`
- `application/vnd.debian.binary-package`

Для `.exe`/`.msi` создаётся ярлык
`~/.local/share/applications/openrun-app-*.desktop` и копия на рабочем столе.
Для `.deb` пересоздаются ярлыки установленных программ
(`openrun-<пакет>-*.desktop`). Если ярлыки или значки не появились, откройте «Настройки» →
«Ярлыки и кеш значков» → «Обновить».

Внутри Flatpak ярлыки Windows-программ используют
`flatpak run io.github.coztov_run.OpenRun --launch-app "Название"`, потому что
прямой путь к Wine внутри песочницы снаружи недоступен.

## Сборка пакетов

```bash
# Переносимый архив
bash packaging/portable/build.sh

# Flatpak (локально)
flatpak-builder --user --install-deps-from=flathub \
  --repo=repo --force-clean build-dir \
  flatpak/io.github.coztov_run.OpenRun.yml
flatpak build-bundle repo openrun.flatpak io.github.coztov_run.OpenRun

# Snap
snapcraft pack --destructive-mode
```

GitHub Actions `.github/workflows/release.yml` по тегу `v*` собирает tar.gz,
Flatpak-бандл и Snap и прикладывает их к релизу.

## Публикация в магазинах

Файлы для Flathub и Snap Store подготовлены, но публикация требует вашего
аккаунта и ручного ревью:

- **Flathub**: [`packaging/flathub/README.md`](packaging/flathub/README.md).
  Репозиторий и тег уже подготовлены; осталось указать commit в манифесте
  и вручную открыть Pull Request в `flathub/flathub` (ветка `new-pr`).
  Автоматически отправлять заявку нельзя по правилам Flathub.
- **Snap Store**: [`snap/README.md`](snap/README.md). Нужны
  `snapcraft login`, загрузка `.snap` и одобрение classic confinement.

После публикации команды `flatpak install flathub io.github.coztov_run.OpenRun`
и `sudo snap install openrun --classic` начнут работать у всех пользователей.

## Лицензия

GPL-3.0-or-later. См. [`LICENSE`](LICENSE).
