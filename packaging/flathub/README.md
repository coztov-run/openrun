# Публикация OpenRun в Flathub

Flathub — это витрина Flatpak-приложений, из которой OpenRun можно установить
на любом дистрибутиве:

```bash
flatpak install flathub io.github.coztov_run.OpenRun
flatpak run io.github.coztov_run.OpenRun
```

Публикация выполняется вручную через Pull Request в репозиторий Flathub.
Автоматически открывать заявку нельзя: правила Flathub запрещают
AI-агентам создавать submission PR и писать за него сообщения. Всё
необходимое для ручной отправки уже подготовлено.

## Что уже готово

- `packaging/flathub/io.github.coztov_run.OpenRun.yml` — манифест для Flathub.
- `cargo-sources.json` — архивы crates для офлайн-сборки.
- `flatpak/io.github.coztov_run.OpenRun.yml` — манифест для локальной сборки.
- `data/io.github.coztov_run.OpenRun.metainfo.xml` — AppStream-метаданные.
- `data/io.github.coztov_run.OpenRun.desktop` — ярлык и MIME-ассоциации.
- `data/icons/hicolor/scalable/apps/io.github.coztov_run.OpenRun.svg` — иконка.
- `packaging/flathub/prepare.sh` — собирает `dist/flathub-submission.tar.gz`
  с тремя файлами для форка Flathub и инструкцией.
- Готовая ветка в форке:
  `https://github.com/coztov-run/flathub/tree/io.github.coztov_run.OpenRun`
- Создать PR из неё:
  `https://github.com/flathub/flathub/compare/new-pr...coztov-run:io.github.coztov_run.OpenRun?expand=1`

## Шаги

1. Убедитесь, что репозиторий `https://github.com/coztov-run/openrun`
   опубликован, а тег `v0.1.1` указывает на нужный коммит. Идентификатор
   приложения `io.github.coztov_run.OpenRun` подразумевает владельца
   `coztov-run` (в app-id — `coztov_run`) и репозиторий `openrun`.
2. Проверьте commit тега:

   ```bash
   git rev-parse v0.1.1
   ```

3. В `packaging/flathub/io.github.coztov_run.OpenRun.yml` уже указан commit
   `b99bfe5162f32e7cb963f72a881158503862a132` для тега `v0.1.1`. Если тег
   пересобран, замените его на новый `git rev-parse v0.1.1`.
4. `cargo-sources.json` уже лежит в корне репозитория. Он содержит архивы
   всех crates и нужен для сборки без сети. При изменении `Cargo.lock`
   пересоберите его:

   ```bash
   python3 flatpak-cargo-generator.py Cargo.lock -o cargo-sources.json
   ```

   Генератор: `https://github.com/flatpak/flatpak-builder-tools/tree/master/cargo`.

5. Проверьте сборку локально (рекомендуется `org.flatpak.Builder`):

   ```bash
   flatpak install -y flathub org.flatpak.Builder
   flatpak run --command=flathub-build org.flatpak.Builder --install \
     packaging/flathub/io.github.coztov_run.OpenRun.yml
   ```

6. Форкните `https://github.com/flathub/flathub` **со всеми ветками**
   (не только `master`). Создайте ветку от `new-pr`:

   ```bash
   git clone --branch=new-pr https://github.com/ВАШ_ЛОГИН/flathub.git
   cd flathub
   git checkout -b io.github.coztov_run.OpenRun
   ```

7. Положите в корень форка три файла:
   - `io.github.coztov_run.OpenRun.yml` — манифест из
     `packaging/flathub/io.github.coztov_run.OpenRun.yml`;
   - `cargo-sources.json` — из корня этого репозитория;
   - `flathub.json` — из `packaging/flathub/flathub.json` (сборка только
     для x86_64, потому что Wine-рантайм OpenRun сейчас x86_64).

   Не добавляйте исходники приложения или готовые бинарники: Flathub
   собирает их из Git-тега.

8. Закоммитьте файлы и отправьте ветку. Откройте Pull Request **в ветку
   `new-pr`** (не в `master`) с заголовком
   `Add io.github.coztov_run.OpenRun`. Опишите, зачем нужны разрешения
   (`home`, `/media`, `/run/media`, `xdg-download`,
   `xdg-data/applications`, `xdg-data/icons`, `xdg-desktop`,
   `xdg-config/mimeapps.list`, `org.freedesktop.Flatpak`), и дождитесь ревью.

9. После одобрения бот Flathub создаст репозиторий приложения и запустит
   официальную сборку. Тогда команда
   `flatpak install flathub io.github.coztov_run.OpenRun` начнёт работать
   у всех пользователей.

## Ограничения

- Flathub принимает приложения с историей разработки и реальными
  пользователями. Совсем новый репозиторий могут попросить подождать.
- Приложение использует Wine, поэтому ревьюеры проверяют, что заявка идёт
  от upstream-разработчика и что проект будет поддерживаться.
- Все зависимости должны собираться без сети. Поэтому в манифесте есть
  `cargo-sources.json` и `CARGO_NET_OFFLINE=true`; `--share=network` в
  `build-args` запрещён.
- PR в Flathub нужно открывать вручную. Не поручайте это AI-агенту:
  правила Flathub это прямо запрещают.

## Важно про доступ к файлам

Flatpak-песочница не даёт приложению свободно писать в домашний каталог.
В манифесте уже запрошены:

- `home` — произвольные `.exe`, `.msi`, `.deb` и `.flatpak`, которые
  пользователь открывает двойным кликом из любой папки;
- `/media` и `/run/media` — те же файлы с флешек и внешних дисков;
- `xdg-download` — файлы из «Загрузок»;
- `xdg-data/applications` и `xdg-data/icons` — ярлыки и значки в меню;
- `xdg-desktop` — ярлыки на рабочем столе;
- `xdg-config/mimeapps.list` — OpenRun как обработчик по умолчанию;
- `--talk-name=org.freedesktop.Flatpak` — установка `.flatpak` через
  `flatpak-spawn --host`, потому что внутри песочницы нет системного
  `flatpak`.

`home` выглядит широко, но без него Flatpak не откроет `.exe` из случайной
папки: файловый менеджер передаёт путь, а песочница его не видит. Если
ревьюер попросит сузить права, можно оставить `xdg-download` и
`xdg-data/applications` — тогда двойной клик будет работать только для
«Загрузок», а ярлыки Windows-программ останутся внутри песочницы.
