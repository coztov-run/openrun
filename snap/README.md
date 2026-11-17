# Сборка и публикация Snap

## Локальная сборка

```bash
sudo snap install snapcraft --classic
# Snapcraft 9 ставит rustup как build-snap, но rustup есть только
# с classic-подключением. Поэтому ставим его заранее.
sudo snap install rustup --classic
snapcraft pack --destructive-mode
```

Готовый файл появится в корне проекта: `openrun_0.1.1_amd64.snap`.
Установка:

```bash
sudo snap install ./openrun_0.1.1_amd64.snap --classic --dangerous
openrun
```

Обычная установка из магазина (после публикации):

```bash
sudo snap install openrun --classic
```

## Почему classic

OpenRun — системная утилита: она создаёт ярлыки в
`~/.local/share/applications`, регистрирует обработчики `.exe`/`.msi`/`.deb`
в `~/.config/mimeapps.list`, запускает Wine из домашнего каталога и работает
с произвольными файлами пользователя. В строгой песочнице Snap эти пути
недоступны без отдельных разрешений `personal-files`, которые тоже требуют
ручного ревью.

Для `classic` Snap Store проводит ручную проверку. Заявку можно оставить в
категории «classic confinement» на forum.snapcraft.io, указав ссылку на
исходники и объяснив необходимость доступа к ярлыкам и Wine.

Важно: у classic-снапа не должно быть секции `plugs`/`slots`. Иначе Store
ответит `confinement 'classic' not allowed with plugs/slots`.

## Публикация

1. Зарегистрируйте имя `openrun` в Snap Store:
   `snapcraft register openrun`.
2. Войдите: `snapcraft login`.
3. Загрузите сборку:
   ```bash
   snapcraft upload --release=stable openrun_0.1.1_amd64.snap
   ```
4. Дождитесь ревью classic-разрешения. После одобрения установка
   `sudo snap install openrun --classic` будет доступна всем.

## Строгая песочница

Если нужен `confinement: strict`, замените в `snapcraft.yaml`:

```yaml
confinement: strict
```

и добавьте `personal-files` plugs для:

- `$HOME/.local/share/applications`
- `$HOME/.local/share/icons`
- `$HOME/.local/share/openrun`
- `$HOME/.config/openrun`
- `$HOME/.config/mimeapps.list`

Эти разрешения также проверяются вручную, а Wine в строгой песочнице может
потребовать дополнительных интерфейсов. Для первого релиза рекомендуется
`classic`.
