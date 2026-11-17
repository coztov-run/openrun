# OpenRun

[English version](README.en.md)

OpenRun — это менеджер Windows-программ для Linux. Он держит Wine-префиксы
в порядке, запускает `.exe` и `.msi`, ставит локальные `.deb` и делает ярлыки,
чтобы к программам можно было возвращаться из меню, а не искать файлы по
папкам.

Написан на Rust с GTK4 и libadwaita, поэтому выглядит как обычное приложение
GNOME. Собственный Wine-рантайм скачивается при первом запуске — отдельно
ставить Wine в систему не нужно.

## Что умеет

Двойной клик по `.exe`, `.msi` или `.deb` — и OpenRun сам открывает файл,
создаёт префикс, если его ещё нет, запускает или устанавливает программу и
добавляет ярлык. Предварительно запускать OpenRun не надо.

Файлы можно просто перетащить в окно. Если перетащить на центральную зону с
плюсом, OpenRun тоже поймёт.

Префиксы можно создавать, запускать, настраивать и убирать из списка. Файлы
на диске при этом не удаляются — список это только список.

Запущенные `.exe` запоминаются. Через них можно снова открыть программу,
сделать ярлык или убрать запись, если она больше не нужна.

Локальные `.deb` ставятся без `dpkg` и без root: OpenRun сам разбирает ar-архив
(нужен только `tar`/`xz`), распаковывает файлы в `~/.local/share/openrun/deb`,
переписывает `.desktop` на новые пути и кладёт иконки в тему. Работает на
Arch, CachyOS, Debian, Ubuntu, Pop!_OS и других дистрибутивах.

В настройках есть тема (системная, светлая, тёмная), язык (русский или
английский) и кнопка «Обновить» — она пересоздаёт ярлыки и кеш значков, если
что-то потерялось после обновления или переустановки.

Заголовок окна без кнопок «свернуть» и «развернуть»: только закрыть. Так
задумано.

## Установка

### Готовые сборки

Самый простой способ — взять файл со страницы релизов:

https://github.com/coztov-run/openrun/releases/latest

Там лежат переносимый `tar.gz`, Flatpak-бандл и Snap. Контрольные суммы —
в `SHA256SUMS`.

### Flatpak

Flatpak лучше всего подходит для Pop!_OS, Ubuntu, Debian, Fedora и других
дистрибутивов: он приносит свои библиотеки и не зависит от версии GTK в
системе.

```bash
flatpak install --user openrun-0.1.0-x86_64.flatpak
flatpak run io.github.coztov_run.OpenRun
```

В Flathub приложения пока нет. Когда заявку примут, установка будет такой:

```bash
flatpak install flathub io.github.coztov_run.OpenRun
```

### Snap

```bash
sudo snap install ./openrun_0.1.0_amd64.snap --classic --dangerous
openrun
```

В Snap Store приложения тоже пока нет: classic-снапы проходят ручное ревью.
После публикации будет достаточно `sudo snap install openrun --classic`.

Snap нужен классический режим, потому что OpenRun пишет ярлыки в домашний
каталог, регистрирует MIME-обработчики и запускает Wine. Подробности — в
[`snap/README.md`](snap/README.md).

### Переносимый архив

```bash
tar -xzf openrun-0.1.0-x86_64.tar.gz
cd openrun-0.1.0-x86_64
./install.sh
~/.local/bin/openrun
```

Нужны Linux x86_64, GTK 4.12+, libadwaita 1.5+ и glibc 2.39+. Это Arch,
CachyOS, Fedora 41+, Ubuntu 24.10+ и новее. На старых системах лучше Flatpak.

Установщик кладёт бинарник в `~/.local/bin`, ярлык — в
`~/.local/share/applications`, иконку — в тему, а также прописывает OpenRun
обработчиком `.exe`, `.msi` и `.deb`. Старые ассоциации остаются как запасной
вариант.

Удалить:

```bash
./uninstall.sh
```

### Из исходников

```bash
cargo build --release
cargo run
```

Первый запуск скачает Wine-рантайм (около 100 МБ) в
`~/.local/share/openrun/runtime`. Дальше интернет не нужен.

Настройки лежат в `~/.config/openrun`, префиксы и установленные `.deb` — в
`~/.local/share/openrun`. Данные старых версий переносятся автоматически.

## Двойной клик и ярлыки

OpenRun регистрирует такие MIME-типы:

- `application/x-msdownload`
- `application/x-ms-dos-executable`
- `application/vnd.microsoft.portable-executable`
- `application/x-msi`
- `application/vnd.debian.binary-package`

Для `.exe` и `.msi` появляется ярлык
`~/.local/share/applications/openrun-app-*.desktop` и копия на рабочем столе.
Для `.deb` — ярлыки вида `openrun-<пакет>-*.desktop`.

Если ярлык или иконка не появились, откройте «Настройки» → «Ярлыки и кеш
значков» → «Обновить». Это быстрее, чем искать причину в логах.

В Flatpak ярлыки Windows-программ запускают
`flatpak run io.github.coztov_run.OpenRun --launch-app "Название"`: путь к
Wine внутри песочницы снаружи недоступен.

## Сборка

```bash
# переносимый архив
bash packaging/portable/build.sh

# Flatpak
flatpak-builder --user --install-deps-from=flathub \
  --repo=repo --force-clean build-dir \
  flatpak/io.github.coztov_run.OpenRun.yml
flatpak build-bundle repo openrun.flatpak io.github.coztov_run.OpenRun

# Snap
snapcraft pack --destructive-mode
```

По тегу `v*` GitHub Actions собирает `tar.gz`, Flatpak и Snap, считает
`SHA256SUMS` и прикладывает всё к релизу.

## Про магазины

Flathub и Snap Store — отдельные витрины. Файл на GitHub Releases не
появляется в них автоматически: туда нужно отправить заявку и дождаться
проверки.

Сейчас заявки ещё не отправлены. Для Flathub всё подготовлено в
[`packaging/flathub`](packaging/flathub/README.md): манифест, `cargo-sources.json`
для сборки без сети и `flathub.json`. Заявку нужно открыть вручную в
`flathub/flathub`, ветка `new-pr`, — правила Flathub не разрешают делать это
автоматически. Для Snap Store нужен `snapcraft login`, загрузка `.snap` и
одобрение classic confinement.

Поэтому поиск в Pop!_Shop и Snap Store пока ничего не находит. Это не ошибка
установки: приложения там просто ещё нет.

## Лицензия

GPL-3.0-or-later. См. [`LICENSE`](LICENSE).
