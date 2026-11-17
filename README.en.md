# OpenRun

A GNOME-style manager for Windows applications and Wine prefixes, written in
Rust with GTK 4 and libadwaita. OpenRun uses its own portable runtime, so a
system Wine installation is not required.

[Русская версия](README.md)

## Features

- **Portable OpenRun Runtime**: downloads a portable Wine build into the
  application data directory. A system Wine is used only as a temporary
  fallback when the runtime is not available yet.
- **Double-click `.exe` / `.msi` / `.deb`**: the file manager starts OpenRun,
  which prepares a prefix if needed, launches or installs the program, and
  creates a desktop shortcut. No prior OpenRun launch is required.
- **Drag and drop**: drop `.exe` / `.msi` / `.deb` anywhere on the page or on
  the central “+” area.
- **Prefixes**: create, run `.exe`/`.msi`, open settings, and remove entries
  from the list without deleting files from disk.
- **Applications**: launched `.exe` files are remembered and can be started
  again, pinned with a shortcut, or removed from the list.
- **`.deb` packages**: install local packages on Arch, CachyOS, Debian,
  Ubuntu, and other distributions. The ar archive is parsed by OpenRun itself;
  only `tar`/`xz` is needed. `.desktop` files are rewritten to the extracted
  paths, icons are copied into the icon theme, and entries appear in the menu.
- **Settings**: system/light/dark theme, Russian/English language, and a
  “Refresh” button that recreates shortcuts and icon caches.
- **Shortcuts**: OpenRun creates its own launcher, application shortcuts in
  the menu and on the desktop, and registers itself for `.exe`, `.msi`, and
  `.deb`. Previous handlers are kept as fallbacks.
- **Window chrome**: a close button only, without minimize or maximize.

## Installation

### Flatpak (recommended on any distribution)

After the Flathub submission is accepted:

```bash
flatpak install flathub io.github.coztov_run.OpenRun
flatpak run io.github.coztov_run.OpenRun
```

Local build or a `.flatpak` from GitHub Releases:

```bash
flatpak install --user openrun-0.1.0-x86_64.flatpak
```

Flatpak ships its own runtime and works on Ubuntu 22.04+, Debian 12+, Fedora,
Arch, openSUSE, and other distributions.

### Snap

After publication in the Snap Store:

```bash
sudo snap install openrun --classic
openrun
```

Local build:

```bash
sudo snap install snapcraft --classic
snapcraft pack --destructive-mode
sudo snap install ./openrun_0.1.0_amd64.snap --classic --dangerous
```

The snap uses `confinement: classic` because it creates shortcuts in the home
directory, registers MIME handlers, and launches Wine. Classic snaps require
manual Snap Store review; see [`snap/README.md`](snap/README.md).

### Portable tar.gz

A ready archive is built to `dist/openrun-0.1.0-x86_64.tar.gz` and attached to
GitHub Releases:

```bash
tar -xzf openrun-0.1.0-x86_64.tar.gz
cd openrun-0.1.0-x86_64
./install.sh
~/.local/bin/openrun
```

Requirements: Linux x86_64, GTK 4.12+, libadwaita 1.5+, glibc 2.39+
(Arch, CachyOS, Fedora 41+, Ubuntu 24.10+). Use Flatpak on older systems.
The installer writes to `~/.local`, creates a launcher, registers MIME
handlers, and refreshes caches when the tools are available.

Uninstall:

```bash
./uninstall.sh
```

### From source

```bash
cargo build --release
cargo run
```

On first launch OpenRun downloads its runtime (about 100 MB) to
`~/.local/share/openrun/runtime` and then works offline. Settings and prefixes
are stored in `~/.config/openrun` and `~/.local/share/openrun/prefixes`.
Data from older versions is migrated automatically.

## Double-click and shortcuts

OpenRun registers these MIME types:

- `application/x-msdownload`
- `application/x-ms-dos-executable`
- `application/vnd.microsoft.portable-executable`
- `application/x-msi`
- `application/vnd.debian.binary-package`

For `.exe`/`.msi`, a shortcut is created at
`~/.local/share/applications/openrun-app-*.desktop` with a copy on the desktop.
For `.deb`, launchers for installed packages are recreated
(`openrun-<package>-*.desktop`). If shortcuts or icons are missing, open
Settings → “Shortcuts and icon cache” → “Refresh”.

Inside Flatpak, Windows application shortcuts use
`flatpak run io.github.coztov_run.OpenRun --launch-app "Name"` because the Wine
path inside the sandbox is not accessible from the host.

## Packaging

```bash
# Portable archive
bash packaging/portable/build.sh

# Flatpak (local)
flatpak-builder --user --install-deps-from=flathub \
  --repo=repo --force-clean build-dir \
  flatpak/io.github.coztov_run.OpenRun.yml
flatpak build-bundle repo openrun.flatpak io.github.coztov_run.OpenRun

# Snap
snapcraft pack --destructive-mode
```

The GitHub Actions workflow `.github/workflows/release.yml` builds the tar.gz,
Flatpak bundle, and Snap for every `v*` tag and attaches them to the release.

## Store publication

The Flathub and Snap Store files are prepared, but publication requires your
account and manual review:

- **Flathub**: see [`packaging/flathub/README.md`](packaging/flathub/README.md).
  The repository and tag are ready; set the commit in the manifest and open a
  pull request against `flathub/flathub` (branch `new-pr`) manually. Flathub
  rules prohibit automated/AI-submitted pull requests.
- **Snap Store**: see [`snap/README.md`](snap/README.md). It requires
  `snapcraft login`, a store upload, and approval of classic confinement.

After publication, `flatpak install flathub io.github.coztov_run.OpenRun` and
`sudo snap install openrun --classic` will work for all users.

## License

GPL-3.0-or-later. See [`LICENSE`](LICENSE).
