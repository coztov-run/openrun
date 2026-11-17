OpenRun 0.1.1 — portable Linux build
====================================

OpenRun is a GNOME-style manager for Windows applications. It downloads its
own portable Wine runtime, creates isolated prefixes, remembers programs, and
creates desktop shortcuts. Double-click an .exe, .msi, or .deb file to open it
in OpenRun.

Requirements
------------
- Linux x86_64
- GTK 4.12 or newer
- libadwaita 1.5 or newer
- glibc 2.39 or newer (Arch, CachyOS, Fedora 41+, Ubuntu 24.10+)
- Network access on the first run to download the Wine runtime (~100 MB)

If your distribution is older, use the Flatpak build instead: Flatpak brings
its own runtime and works on Ubuntu 22.04+, Debian 12+, Fedora, Arch, etc.

Install
-------
    tar -xzf openrun-0.1.1-x86_64.tar.gz
    cd openrun-0.1.1-x86_64
    ./install.sh

The default installation prefix is ~/.local. To install elsewhere:

    PREFIX=/opt/openrun ./install.sh

Then start OpenRun once:

    ~/.local/bin/openrun

The first launch downloads the Wine runtime. OpenRun registers itself as the
handler for .exe, .msi, and .deb files and creates a menu entry. Running it
once also creates a desktop shortcut if a Desktop folder exists.

Uninstall
---------
    ./uninstall.sh

This removes the binary, desktop entry, and icons. Settings, prefixes, and the
downloaded Wine runtime stay in:

    ~/.config/openrun
    ~/.local/share/openrun

Remove those directories manually only if you no longer need your Windows
applications and prefixes.

Files
-----
openrun                    main binary
openrun.desktop            desktop entry
openrun.svg                icon
openrun.metainfo.xml       AppStream metadata
install.sh                 installer
uninstall.sh               uninstaller
