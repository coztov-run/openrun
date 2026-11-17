//! Главное окно в стиле GNOME Files: компактная боковая панель слева, страницы справа.
//! Файлы .exe/.msi и .deb можно перетащить на «+» или в любое место страницы.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use gtk4 as gtk;
use libadwaita as adw;

use crate::core::config::{AppEntry, Config, Theme};
use crate::core::deb;
use crate::core::flatpak;
use crate::core::i18n::{self, t, Lang};
use crate::core::prefix::Prefix;
use crate::core::runner;
use crate::core::runtime;
use crate::core::shortcut;

const CSS: &str = "
/* Компактные кнопки боковой панели в стиле Nautilus (GNOME Files) */
navigation-sidebar row,
.navigation-sidebar > row {
    min-height: 38px;
    padding-top: 0;
    padding-bottom: 0;
    margin-top: 1px;
    margin-bottom: 1px;
    border-radius: 9px;
}

/* Одинаковая компактная высота у всех строк-«плашек». */
.boxed-list > row {
    min-height: 46px;
    padding-top: 4px;
    padding-bottom: 4px;
}
.boxed-list > row > box.header {
    min-height: 34px;
}
.boxed-list > row > box.header > box.title {
    margin-top: 0;
    margin-bottom: 0;
}

/* Компактные кнопки в строках и заголовках групп. */
.compact-button {
    min-height: 26px;
    min-width: 26px;
    padding: 0 8px;
}

/* Зона перетаскивания с плюсом в центре */
.drop-zone {
    border: 2px dashed alpha(currentColor, 0.24);
    border-radius: 12px;
    padding: 13px 16px;
    background-color: alpha(currentColor, 0.02);
    transition: all 150ms ease;
}
.drop-zone:hover {
    border-color: alpha(currentColor, 0.45);
    background-color: alpha(currentColor, 0.05);
}
.drop-zone.drag-over {
    border-color: @accent_bg_color;
    background-color: alpha(@accent_bg_color, 0.14);
}
";

struct State {
    window: adw::ApplicationWindow,
    toasts: adw::ToastOverlay,
    config: RefCell<Config>,
    prefix_group: adw::PreferencesGroup,
    prefix_rows: RefCell<Vec<adw::ActionRow>>,
    app_group: adw::PreferencesGroup,
    app_rows: RefCell<Vec<adw::ActionRow>>,
    stack: gtk::Stack,
    deb_group: adw::PreferencesGroup,
    deb_rows: RefCell<Vec<adw::ActionRow>>,
    flatpak_group: adw::PreferencesGroup,
    flatpak_rows: RefCell<Vec<adw::ActionRow>>,
    runtime_toast: RefCell<Option<adw::Toast>>,
    runtime_busy: Cell<bool>,
}

type Shared = Rc<State>;

// Текущее окно и его состояние. Нужны, чтобы второй запуск OpenRun
// (например, двойной клик по .exe при уже открытом приложении)
// передал файл в существующее окно, а не создавал новое.
thread_local! {
    static CURRENT: RefCell<Option<Shared>> = RefCell::new(None);
}

impl State {
    fn toast(&self, text: &str) {
        self.toasts.add_toast(adw::Toast::new(text));
    }

    fn save(&self) {
        if let Err(e) = self.config.borrow().save() {
            self.toast(&format!(
                "{}: {e}",
                t("Не удалось сохранить настройки", "Failed to save settings")
            ));
        }
    }
}

fn load_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(CSS);
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

/// Компактная строка боковой панели, как в GNOME Files.
fn sidebar_row(entry: (&str, &str, &str)) -> gtk::ListBoxRow {
    let icon = gtk::Image::builder()
        .icon_name(entry.2)
        .pixel_size(16)
        .build();

    let label = gtk::Label::builder()
        .label(entry.1)
        .halign(gtk::Align::Start)
        .hexpand(true)
        .build();

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(10)
        .margin_top(2)
        .margin_bottom(2)
        .margin_start(4)
        .margin_end(4)
        .build();
    content.append(&icon);
    content.append(&label);

    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&content));
    row.set_widget_name(entry.0);
    row
}

/// Кнопка «+» в центре: сюда можно перетащить файл или нажать для выбора.
fn drop_zone(title: Option<&str>) -> gtk::Button {
    let icon = gtk::Image::builder()
        .icon_name("list-add-symbolic")
        .pixel_size(28)
        .build();

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();
    content.append(&icon);
    if let Some(title) = title {
        let title_label = gtk::Label::builder().label(title).build();
        title_label.add_css_class("title-4");
        content.append(&title_label);
    }

    let button = gtk::Button::builder().child(&content).build();
    button.add_css_class("flat");
    button.add_css_class("drop-zone");
    button
}

/// Подключает приём перетаскиваемых файлов к виджету и подсвечивает визуальную drop-зону.
fn attach_drop<W, F>(widget: &W, highlight_target: &gtk::Button, on_files: F)
where
    W: IsA<gtk::Widget>,
    F: Fn(Vec<PathBuf>) + 'static,
{
    let target = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);

    let b = highlight_target.clone();
    target.connect_enter(move |_, _, _| {
        b.add_css_class("drag-over");
        gdk::DragAction::COPY
    });

    let b = highlight_target.clone();
    target.connect_leave(move |_| {
        b.remove_css_class("drag-over");
    });

    let b = highlight_target.clone();
    target.connect_drop(move |_, value, _, _| {
        b.remove_css_class("drag-over");
        match value.get::<gdk::FileList>() {
            Ok(list) => {
                let paths: Vec<PathBuf> = list.files().iter().filter_map(|f| f.path()).collect();
                if paths.is_empty() {
                    return false;
                }
                on_files(paths);
                true
            }
            Err(_) => false,
        }
    });

    widget.add_controller(target);
}

/// Показывает окно и передаёт ему файлы, открытые двойным кликом.
///
/// Если OpenRun уже запущен, используется существующее окно: так работает
/// и второй процесс, который GApplication перенаправляет первому.
pub fn open(app: &adw::Application, files: Vec<PathBuf>) {
    if let Some(st) = CURRENT.with(|current| current.borrow().clone()) {
        st.window.present();
        if !files.is_empty() {
            handle_paths(&st, files);
        }
        return;
    }
    build_window(app, files);
}

/// Открывает окно и запускает запись из списка по имени.
/// Используется ярлыками, созданными внутри Flatpak.
pub fn open_app(app: &adw::Application, name: &str) {
    open(app, Vec::new());
    CURRENT.with(|current| {
        if let Some(st) = current.borrow().clone() {
            launch_by_name(&st, name);
        }
    });
}

fn launch_by_name(st: &Shared, name: &str) {
    let (app, prefix) = {
        let cfg = st.config.borrow();
        let Some(app) = cfg.apps.iter().find(|app| app.name == name).cloned() else {
            drop(cfg);
            st.toast(&format!(
                "{}: {name}",
                t("Программа не найдена", "Application not found")
            ));
            return;
        };
        let prefix = cfg
            .prefixes
            .iter()
            .find(|prefix| prefix.name == app.prefix_name)
            .cloned()
            .or_else(|| cfg.prefixes.first().cloned());
        (app, prefix)
    };

    match prefix {
        Some(prefix) => launch(st, prefix, vec![app.exe]),
        None => st.toast(t("Сначала создайте префикс", "Create a prefix first")),
    }
}

fn build_window(app: &adw::Application, files: Vec<PathBuf>) -> adw::ApplicationWindow {
    load_css();
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title(crate::core::APP_NAME)
        .default_width(840)
        .default_height(580)
        .build();
    fill_window(&window, true, "prefixes", files);
    window.present();
    window
}

/// Заголовок страницы в правой части окна.
fn page_title(id: &str) -> &'static str {
    match id {
        "deb" => t("Пакеты .deb", ".deb packages"),
        "flatpak" => t("Пакеты .flatpak", ".flatpak packages"),
        "settings" => t("Настройки", "Settings"),
        _ => t("Префиксы", "Prefixes"),
    }
}

/// Тема оформления из настроек.
fn apply_theme(theme: Theme) {
    let scheme = match theme {
        Theme::System => adw::ColorScheme::Default,
        Theme::Light => adw::ColorScheme::PreferLight,
        Theme::Dark => adw::ColorScheme::PreferDark,
    };
    adw::StyleManager::default().set_color_scheme(scheme);
}

/// Собирает содержимое окна. Вызывается заново при смене языка.
fn fill_window(
    window: &adw::ApplicationWindow,
    first_run: bool,
    start_page: &str,
    initial_files: Vec<PathBuf>,
) {
    let cfg = Config::load();
    i18n::set(cfg.language);
    apply_theme(cfg.theme);

    // ---- Страница: префиксы ----
    let exe_zone = drop_zone(Some(t(
        "Перетащите .exe или .msi",
        "Drop .exe or .msi here",
    )));
    let exe_zone_group = adw::PreferencesGroup::new();
    exe_zone_group.add(&exe_zone);

    // ---- Установленные программы (страница настроек) ----
    let app_group = adw::PreferencesGroup::builder()
        .title(t("Приложения", "Applications"))
        .build();

    let new_prefix_btn = gtk::Button::with_label(t("Новый префикс", "New prefix"));
    new_prefix_btn.add_css_class("flat");
    new_prefix_btn.add_css_class("compact-button");
    let prefix_group = adw::PreferencesGroup::builder()
        .title(t("Префиксы OpenRun", "OpenRun prefixes"))
        .build();
    prefix_group.set_header_suffix(Some(&new_prefix_btn));

    let prefix_page = adw::PreferencesPage::new();
    prefix_page.add(&exe_zone_group);
    prefix_page.add(&prefix_group);

    // ---- Страница: .deb ----
    let deb_zone = drop_zone(None);
    let deb_zone_group = adw::PreferencesGroup::new();
    deb_zone_group.add(&deb_zone);

    let deb_group = adw::PreferencesGroup::builder()
        .title(t("Установленные пакеты", "Installed packages"))
        .build();

    let deb_page = adw::PreferencesPage::new();
    deb_page.add(&deb_zone_group);
    deb_page.add(&deb_group);

    // ---- Страница: .flatpak ----
    let flatpak_zone = drop_zone(Some(t("Перетащите .flatpak", "Drop .flatpak here")));
    let flatpak_zone_group = adw::PreferencesGroup::new();
    flatpak_zone_group.add(&flatpak_zone);

    let flatpak_group = adw::PreferencesGroup::builder()
        .title(t("Установленные Flatpak", "Installed Flatpaks"))
        .build();

    let flatpak_page = adw::PreferencesPage::new();
    flatpak_page.add(&flatpak_zone_group);
    flatpak_page.add(&flatpak_group);

    // ---- Страница: настройки ----
    let theme_row = adw::ActionRow::builder().title(t("Тема", "Theme")).build();
    let theme_dd = gtk::DropDown::from_strings(&[
        t("Системная", "System"),
        t("Светлая", "Light"),
        t("Тёмная", "Dark"),
    ]);
    theme_dd.set_selected(match cfg.theme {
        Theme::System => 0,
        Theme::Light => 1,
        Theme::Dark => 2,
    });
    theme_dd.set_valign(gtk::Align::Center);
    theme_row.add_suffix(&theme_dd);

    let appearance_group = adw::PreferencesGroup::builder()
        .title(t("Внешний вид", "Appearance"))
        .build();
    appearance_group.add(&theme_row);

    let lang_row = adw::ActionRow::builder()
        .title(t("Язык интерфейса", "Interface language"))
        .build();
    let lang_dd = gtk::DropDown::from_strings(&["Русский", "English"]);
    lang_dd.set_selected(match cfg.language {
        Lang::Ru => 0,
        Lang::En => 1,
    });
    lang_dd.set_valign(gtk::Align::Center);
    lang_row.add_suffix(&lang_dd);

    let lang_group = adw::PreferencesGroup::builder()
        .title(t("Язык", "Language"))
        .build();
    lang_group.add(&lang_row);

    let refresh_btn = gtk::Button::with_label(t("Обновить", "Refresh"));
    refresh_btn.add_css_class("flat");
    refresh_btn.add_css_class("compact-button");

    let shortcuts_row = adw::ActionRow::builder()
        .title(t("Ярлыки и кеш значков", "Shortcuts and icon cache"))
        .subtitle(t(
            "Пересоздать ярлыки OpenRun и обновить кеш значков",
            "Recreate OpenRun shortcuts and refresh the icon cache",
        ))
        .build();
    shortcuts_row.add_suffix(&refresh_btn);

    let maintenance_group = adw::PreferencesGroup::builder()
        .title(t("Обслуживание", "Maintenance"))
        .build();
    maintenance_group.add(&shortcuts_row);

    let settings_page = adw::PreferencesPage::new();
    settings_page.add(&appearance_group);
    settings_page.add(&lang_group);
    settings_page.add(&maintenance_group);
    settings_page.add(&app_group);

    // ---- Стек страниц ----
    let stack = gtk::Stack::new();
    stack.add_titled(&prefix_page, Some("prefixes"), t("Префиксы", "Prefixes"));
    stack.add_titled(&deb_page, Some("deb"), t("Пакеты .deb", ".deb packages"));
    stack.add_titled(
        &flatpak_page,
        Some("flatpak"),
        t("Пакеты .flatpak", ".flatpak packages"),
    );
    stack.add_titled(&settings_page, Some("settings"), t("Настройки", "Settings"));

    // ---- Боковая панель (компактная, как в Files) ----
    let list = gtk::ListBox::new();
    list.add_css_class("navigation-sidebar");
    list.set_selection_mode(gtk::SelectionMode::Single);
    let rows = [
        sidebar_row((
            "prefixes",
            t("Префиксы", "Prefixes"),
            "drive-harddisk-symbolic",
        )),
        sidebar_row((
            "deb",
            t("Пакеты .deb", ".deb packages"),
            "folder-download-symbolic",
        )),
        sidebar_row((
            "flatpak",
            t("Пакеты .flatpak", ".flatpak packages"),
            "package-x-generic-symbolic",
        )),
        sidebar_row((
            "settings",
            t("Настройки", "Settings"),
            "emblem-system-symbolic",
        )),
    ];
    for row in &rows {
        list.append(row);
    }

    let stack_for_selection = stack.clone();
    list.connect_row_selected(move |_, row| {
        if let Some(row) = row {
            let id = row.widget_name();
            stack_for_selection.set_visible_child_name(&id);
        }
    });
    let selected = rows
        .iter()
        .find(|row| row.widget_name() == start_page)
        .unwrap_or(&rows[0]);
    list.select_row(Some(selected));

    // В заголовке — только кнопка закрытия, без «свернуть» и «развернуть».
    let sidebar_header = adw::HeaderBar::new();
    sidebar_header.set_decoration_layout(Some(":close"));
    let sidebar_toolbar = adw::ToolbarView::new();
    sidebar_toolbar.add_top_bar(&sidebar_header);
    sidebar_toolbar.set_content(Some(&list));
    let sidebar_page = adw::NavigationPage::builder()
        .title(crate::core::APP_NAME)
        .child(&sidebar_toolbar)
        .build();

    // ---- Контентная часть ----
    let content_header = adw::HeaderBar::new();
    content_header.set_decoration_layout(Some(":close"));
    let content_toolbar = adw::ToolbarView::new();
    content_toolbar.add_top_bar(&content_header);
    content_toolbar.set_content(Some(&stack));
    let content_page = adw::NavigationPage::builder()
        .title(page_title(start_page))
        .child(&content_toolbar)
        .build();

    let content_page_for_title = content_page.clone();
    list.connect_row_selected(move |_, row| {
        if let Some(row) = row {
            content_page_for_title.set_title(page_title(row.widget_name().as_str()));
        }
    });

    let split = adw::NavigationSplitView::builder()
        .sidebar(&sidebar_page)
        .content(&content_page)
        .min_sidebar_width(200.0)
        .max_sidebar_width(240.0)
        .build();

    // Название приложения — чуть ниже центра шапки.
    let app_title = gtk::Label::builder()
        .label(crate::core::APP_NAME)
        .margin_top(8)
        .build();
    app_title.add_css_class("title");
    sidebar_header.set_title_widget(Some(&app_title));

    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&split));
    window.set_content(Some(&toasts));

    let state: Shared = Rc::new(State {
        window: window.clone(),
        toasts,
        config: RefCell::new(cfg),
        prefix_group,
        prefix_rows: RefCell::new(Vec::new()),
        app_group,
        app_rows: RefCell::new(Vec::new()),
        stack: stack.clone(),
        deb_group,
        deb_rows: RefCell::new(Vec::new()),
        flatpak_group,
        flatpak_rows: RefCell::new(Vec::new()),
        runtime_toast: RefCell::new(None),
        runtime_busy: Cell::new(false),
    });

    // .exe/.msi: клик по «+» и перетаскивание как на саму зону, так и на всю страницу префиксов
    let st = state.clone();
    exe_zone.connect_clicked(move |_| {
        choose_file(
            &st,
            t("Выберите программу", "Choose a program"),
            t(
                "Windows-программы (.exe, .msi)",
                "Windows programs (.exe, .msi)",
            ),
            &["exe", "msi"],
            |st, path| handle_exe_paths(st, vec![path]),
        );
    });
    let st = state.clone();
    attach_drop(&prefix_page, &exe_zone, move |paths| {
        handle_exe_paths(&st, paths)
    });

    // .deb: клик по «+» и перетаскивание как на саму зону, так и на всю страницу .deb
    let st = state.clone();
    deb_zone.connect_clicked(move |_| {
        choose_file(
            &st,
            t("Выберите пакет", "Choose a package"),
            t("Пакеты Debian (.deb)", "Debian packages (.deb)"),
            &["deb"],
            |st, path| handle_deb_paths(st, vec![path]),
        );
    });
    let st = state.clone();
    attach_drop(&deb_page, &deb_zone, move |paths| {
        handle_deb_paths(&st, paths)
    });

    // .flatpak: установка бандла через системный flatpak.
    let st = state.clone();
    flatpak_zone.connect_clicked(move |_| {
        choose_file(
            &st,
            t("Выберите Flatpak-пакет", "Choose a Flatpak bundle"),
            t("Пакеты Flatpak (.flatpak)", "Flatpak bundles (.flatpak)"),
            &["flatpak"],
            |st, path| handle_flatpak_paths(st, vec![path]),
        );
    });
    let st = state.clone();
    attach_drop(&flatpak_page, &flatpak_zone, move |paths| {
        handle_flatpak_paths(&st, paths)
    });

    let st = state.clone();
    new_prefix_btn.connect_clicked(move |_| add_prefix_dialog(&st));

    // Пересоздаёт ярлыки и обновляет кеш значков — полезно, если рабочий стол
    // или меню приложений продолжают показывать старую иконку.
    let st = state.clone();
    refresh_btn.connect_clicked(move |_| {
        let apps = st.config.borrow().apps.clone();
        st.toast(t("Обновляю ярлыки…", "Refreshing shortcuts…"));
        run_background(
            &st,
            move || shortcut::refresh_all(&apps),
            |st, result| match result {
                Ok(()) => st.toast(t(
                    "Ярлыки и кеш значков обновлены",
                    "Shortcuts and icon cache refreshed",
                )),
                Err(e) => st.toast(&format!(
                    "{}: {e}",
                    t("Не удалось обновить ярлыки", "Failed to refresh shortcuts")
                )),
            },
        );
    });

    // Тема меняется сразу.
    let st = state.clone();
    theme_dd.connect_selected_notify(move |dd| {
        let theme = match dd.selected() {
            1 => Theme::Light,
            2 => Theme::Dark,
            _ => Theme::System,
        };
        apply_theme(theme);
        st.config.borrow_mut().theme = theme;
        st.save();
    });

    // Смена языка пересобирает интерфейс.
    let st = state.clone();
    lang_dd.connect_selected_notify(move |dd| {
        let lang = if dd.selected() == 1 {
            Lang::En
        } else {
            Lang::Ru
        };
        if i18n::current() == lang {
            return;
        }
        i18n::set(lang);
        st.config.borrow_mut().language = lang;
        st.save();
        let window = st.window.clone();
        let page = st
            .stack
            .visible_child_name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "prefixes".to_string());
        glib::idle_add_local_once(move || {
            window.set_content(None::<&gtk::Widget>);
            fill_window(&window, false, &page, Vec::new());
        });
    });

    refresh_prefixes(&state);
    refresh_apps(&state);
    refresh_deb_list(&state);
    refresh_flatpak_list(&state);

    // При первом запуске OpenRun скачивается сам — ничего не нужно ставить в систему.
    if first_run && !runtime::is_bundled_ready() {
        start_runtime_install(&state);
    }

    if let Err(e) = shortcut::create_self_shortcut() {
        state.toast(&format!(
            "{}: {e}",
            t(
                "Не удалось создать ярлык приложения",
                "Failed to create app shortcut"
            )
        ));
    }

    CURRENT.with(|current| {
        *current.borrow_mut() = Some(state.clone());
    });

    if !initial_files.is_empty() {
        let st = state.clone();
        glib::idle_add_local_once(move || handle_paths(&st, initial_files));
    }
}

fn icon_button(icon: &str, tip: &str) -> gtk::Button {
    let button = gtk::Button::from_icon_name(icon);
    button.set_tooltip_text(Some(tip));
    button.set_valign(gtk::Align::Center);
    button.add_css_class("flat");
    button.add_css_class("compact-button");
    button
}

fn has_ext(path: &Path, exts: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| exts.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

/// Обрабатывает файлы, открытые двойным кликом в файловом менеджере.
fn handle_paths(st: &Shared, paths: Vec<PathBuf>) {
    let mut debs = Vec::new();
    let mut flatpaks = Vec::new();
    let mut windows = Vec::new();
    for path in paths {
        if has_ext(&path, &["deb"]) {
            debs.push(path);
        } else if has_ext(&path, &["flatpak"]) {
            flatpaks.push(path);
        } else if has_ext(&path, &["exe", "msi"]) {
            windows.push(path);
        }
    }

    if debs.is_empty() && flatpaks.is_empty() && windows.is_empty() {
        st.toast(t(
            "Откройте файл .exe, .msi, .deb или .flatpak",
            "Open an .exe, .msi, .deb, or .flatpak file",
        ));
        return;
    }
    if !debs.is_empty() {
        handle_deb_paths(st, debs);
    }
    if !flatpaks.is_empty() {
        handle_flatpak_paths(st, flatpaks);
    }
    if !windows.is_empty() {
        handle_exe_paths(st, windows);
    }
}

fn prefix_at(st: &Shared, idx: usize) -> Option<Prefix> {
    st.config.borrow().prefixes.get(idx).cloned()
}

fn app_at(st: &Shared, idx: usize) -> Option<AppEntry> {
    st.config.borrow().apps.get(idx).cloned()
}

/// Название программы для списка: из имени файла, без служебных символов.
fn human_name(raw: &str) -> String {
    let cleaned = raw.replace('_', " ").replace('-', " ");
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        t("Программа", "Program").to_string()
    } else {
        cleaned.to_string()
    }
}

fn looks_like_installer(exe: &Path) -> bool {
    let name = exe
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    ["setup", "install", "unins", "update", "patch"]
        .iter()
        .any(|key| name.contains(key))
}

/// Запоминает запущенную .exe. Возвращает новую запись, если её ещё не было.
fn record_app(st: &Shared, prefix: &Prefix, exe: &Path) -> Option<AppEntry> {
    let name = human_name(
        exe.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_else(|| t("Программа", "Program")),
    );

    let mut cfg = st.config.borrow_mut();
    if cfg
        .apps
        .iter()
        .any(|a| a.exe.as_path() == exe && a.prefix == prefix.path)
    {
        return None;
    }

    let entry = AppEntry {
        name,
        exe: exe.to_path_buf(),
        prefix_name: prefix.name.clone(),
        prefix: prefix.path.clone(),
        runner: prefix.runner.clone(),
    };
    cfg.apps.push(entry.clone());
    drop(cfg);
    st.save();
    Some(entry)
}

fn refresh_prefixes(st: &Shared) {
    let old: Vec<adw::ActionRow> = st.prefix_rows.borrow_mut().drain(..).collect();
    for row in &old {
        st.prefix_group.remove(row);
    }

    let prefixes: Vec<Prefix> = st.config.borrow().prefixes.clone();
    if prefixes.is_empty() {
        let row = adw::ActionRow::builder()
            .title(t("Префиксов пока нет", "No prefixes yet"))
            .build();
        st.prefix_group.add(&row);
        st.prefix_rows.borrow_mut().push(row);
        return;
    }

    for (idx, prefix) in prefixes.iter().enumerate() {
        let row = adw::ActionRow::builder()
            .title(prefix.name.as_str())
            .build();

        let run_btn = icon_button(
            "media-playback-start-symbolic",
            t(
                "Запустить .exe в этом префиксе",
                "Run an .exe in this prefix",
            ),
        );
        let cfg_btn = icon_button(
            "emblem-system-symbolic",
            t("Открыть настройки OpenRun", "Open OpenRun settings"),
        );
        let del_btn = icon_button(
            "user-trash-symbolic",
            t(
                "Убрать из списка (файлы останутся)",
                "Remove from list (files are kept)",
            ),
        );

        let (s1, s2, s3) = (st.clone(), st.clone(), st.clone());
        run_btn.connect_clicked(move |_| pick_exe_for_prefix(&s1, idx));
        cfg_btn.connect_clicked(move |_| {
            if let Some(p) = prefix_at(&s2, idx) {
                if let Err(e) = runner::run_winecfg(&p) {
                    s2.toast(&format!("{e}"));
                }
            }
        });
        del_btn.connect_clicked(move |_| remove_prefix(&s3, idx));

        row.add_suffix(&run_btn);
        row.add_suffix(&cfg_btn);
        row.add_suffix(&del_btn);
        st.prefix_group.add(&row);
        st.prefix_rows.borrow_mut().push(row);
    }
}

fn refresh_apps(st: &Shared) {
    let old: Vec<adw::ActionRow> = st.app_rows.borrow_mut().drain(..).collect();
    for row in &old {
        st.app_group.remove(row);
    }

    let apps: Vec<AppEntry> = st.config.borrow().apps.clone();
    if apps.is_empty() {
        let row = adw::ActionRow::builder()
            .title(t("Приложений пока нет", "No applications yet"))
            .build();
        st.app_group.add(&row);
        st.app_rows.borrow_mut().push(row);
        return;
    }

    for (idx, app) in apps.iter().enumerate() {
        let row = adw::ActionRow::builder()
            .title(app.name.as_str())
            .activatable(true)
            .build();
        let icon = gtk::Image::from_icon_name("application-x-executable-symbolic");
        icon.set_pixel_size(24);
        row.add_prefix(&icon);

        let s0 = st.clone();
        row.connect_activated(move |_| {
            if let Some(app) = app_at(&s0, idx) {
                launch(&s0, app.prefix(), vec![app.exe.clone()]);
            }
        });

        let link_btn = icon_button(
            "bookmark-new-symbolic",
            t("Создать ярлык", "Create shortcut"),
        );
        let del_btn = icon_button(
            "user-trash-symbolic",
            t(
                "Убрать из списка и удалить ярлык",
                "Remove from list and delete shortcut",
            ),
        );

        let s2 = st.clone();
        link_btn.connect_clicked(move |_| {
            if let Some(app) = app_at(&s2, idx) {
                match shortcut::create_app_shortcut(&app) {
                    Ok(path) => s2.toast(&format!(
                        "{}: {}",
                        t("Ярлык создан", "Shortcut created"),
                        path.display()
                    )),
                    Err(e) => s2.toast(&format!(
                        "{}: {e}",
                        t("Не удалось создать ярлык", "Failed to create shortcut")
                    )),
                }
            }
        });

        let s3 = st.clone();
        del_btn.connect_clicked(move |_| remove_app(&s3, idx));

        row.add_suffix(&link_btn);
        row.add_suffix(&del_btn);
        st.app_group.add(&row);
        st.app_rows.borrow_mut().push(row);
    }
}

fn refresh_deb_list(st: &Shared) {
    let old: Vec<adw::ActionRow> = st.deb_rows.borrow_mut().drain(..).collect();
    for row in &old {
        st.deb_group.remove(row);
    }

    let names = deb::list_installed();
    if names.is_empty() {
        let row = adw::ActionRow::builder()
            .title(t("Пакетов пока нет", "No packages yet"))
            .build();
        st.deb_group.add(&row);
        st.deb_rows.borrow_mut().push(row);
        return;
    }

    for name in names {
        let row = adw::ActionRow::builder().title(name.as_str()).build();

        let del_btn = icon_button(
            "user-trash-symbolic",
            t("Удалить пакет и ярлыки", "Delete package and shortcuts"),
        );
        let st2 = st.clone();
        let pkg = name.clone();
        del_btn.connect_clicked(move |_| match deb::uninstall(&pkg) {
            Ok(()) => {
                st2.toast(&format!("{}: {pkg}", t("Удалено", "Deleted")));
                refresh_deb_list(&st2);
            }
            Err(e) => st2.toast(&format!("{}: {e}", t("Ошибка удаления", "Delete failed"))),
        });
        if deb::has_launcher(&name) {
            let run_btn = icon_button(
                "media-playback-start-symbolic",
                t("Запустить программу", "Run program"),
            );
            let st3 = st.clone();
            let pkg = name.clone();
            run_btn.connect_clicked(move |_| {
                if let Err(e) = deb::launch(&pkg) {
                    st3.toast(&format!(
                        "{}: {e}",
                        t("Не удалось запустить", "Failed to run")
                    ));
                }
            });
            row.add_suffix(&run_btn);
        }
        row.add_suffix(&del_btn);
        st.deb_group.add(&row);
        st.deb_rows.borrow_mut().push(row);
    }
}

fn refresh_flatpak_list(st: &Shared) {
    let old: Vec<adw::ActionRow> = st.flatpak_rows.borrow_mut().drain(..).collect();
    for row in &old {
        st.flatpak_group.remove(row);
    }

    if !flatpak::available() {
        let row = adw::ActionRow::builder()
            .title(t("Flatpak не найден", "Flatpak is not installed"))
            .subtitle(t(
                "Установите flatpak, чтобы открывать .flatpak-бандлы",
                "Install flatpak to open .flatpak bundles",
            ))
            .build();
        st.flatpak_group.add(&row);
        st.flatpak_rows.borrow_mut().push(row);
        return;
    }

    let apps = flatpak::list_installed();
    if apps.is_empty() {
        let row = adw::ActionRow::builder()
            .title(t(
                "Установленных Flatpak пока нет",
                "No installed Flatpaks yet",
            ))
            .build();
        st.flatpak_group.add(&row);
        st.flatpak_rows.borrow_mut().push(row);
        return;
    }

    for app in apps {
        let row = adw::ActionRow::builder()
            .title(app.name.as_str())
            .subtitle(app.id.as_str())
            .build();

        let run_btn = icon_button(
            "media-playback-start-symbolic",
            t("Запустить Flatpak", "Run Flatpak"),
        );
        let st2 = st.clone();
        let id = app.id.clone();
        run_btn.connect_clicked(move |_| {
            if let Err(e) = flatpak::launch(&id) {
                st2.toast(&format!(
                    "{}: {e}",
                    t("Не удалось запустить", "Failed to run")
                ));
            }
        });

        let del_btn = icon_button(
            "user-trash-symbolic",
            t("Удалить Flatpak и ярлык", "Remove Flatpak and shortcut"),
        );
        let st3 = st.clone();
        let id = app.id.clone();
        del_btn.connect_clicked(move |_| match flatpak::uninstall(&id) {
            Ok(()) => {
                st3.toast(&format!("{}: {id}", t("Удалено", "Removed")));
                refresh_flatpak_list(&st3);
            }
            Err(e) => st3.toast(&format!("{}: {e}", t("Ошибка удаления", "Removal failed"))),
        });

        row.add_suffix(&run_btn);
        row.add_suffix(&del_btn);
        st.flatpak_group.add(&row);
        st.flatpak_rows.borrow_mut().push(row);
    }
}

enum RuntimeMsg {
    Progress(runtime::Progress),
    Finished(Option<String>),
}

fn set_runtime_toast(st: &Shared, text: &str) {
    if let Some(toast) = st.runtime_toast.borrow().as_ref() {
        toast.set_title(text);
    }
}

/// Тихо скачивает встроенный рантайм OpenRun при первом запуске.
fn start_runtime_install(st: &Shared) {
    if st.runtime_busy.get() {
        return;
    }
    st.runtime_busy.set(true);

    let toast = adw::Toast::new(t("Скачиваю OpenRun…", "Downloading OpenRun…"));
    toast.set_timeout(0);
    st.toasts.add_toast(toast.clone());
    st.runtime_toast.replace(Some(toast));

    let (tx, rx) = async_channel::unbounded::<RuntimeMsg>();
    std::thread::spawn(move || {
        let sender = tx.clone();
        let mut progress = move |p| {
            let _ = sender.send_blocking(RuntimeMsg::Progress(p));
        };
        let result = runtime::install(&mut progress);
        drop(progress);
        let _ = tx.send_blocking(RuntimeMsg::Finished(result.err().map(|e| format!("{e}"))));
    });

    let st = st.clone();
    glib::spawn_future_local(async move {
        while let Ok(msg) = rx.recv().await {
            match msg {
                RuntimeMsg::Progress(runtime::Progress::Status(text)) => {
                    set_runtime_toast(&st, &text);
                }
                RuntimeMsg::Progress(runtime::Progress::Download { done, total }) => {
                    let text = if total > 0 {
                        format!(
                            "{} · {:.0}%",
                            t("Скачиваю OpenRun", "Downloading OpenRun"),
                            done as f64 / total as f64 * 100.0
                        )
                    } else {
                        t("Скачиваю OpenRun…", "Downloading OpenRun…").to_string()
                    };
                    set_runtime_toast(&st, &text);
                }
                RuntimeMsg::Progress(runtime::Progress::Extract) => {
                    set_runtime_toast(&st, t("Распаковываю OpenRun…", "Extracting OpenRun…"));
                }
                RuntimeMsg::Finished(Some(err)) => {
                    if let Some(toast) = st.runtime_toast.borrow().as_ref() {
                        toast.set_title(&format!(
                            "{}: {err}",
                            t(
                                "Не удалось подготовить OpenRun",
                                "Failed to prepare OpenRun"
                            )
                        ));
                        toast.set_timeout(5);
                    }
                }
                RuntimeMsg::Finished(None) => {
                    if let Some(toast) = st.runtime_toast.borrow().as_ref() {
                        toast.set_title(t("OpenRun готов к работе", "OpenRun is ready"));
                        toast.set_timeout(3);
                    }
                }
            }
        }

        st.runtime_busy.set(false);
        st.runtime_toast.replace(None);
    });
}

/// Выполняет блокирующую работу в отдельном потоке и возвращает результат в главный поток.
fn run_background<T, W, D>(st: &Shared, work: W, done: D)
where
    T: Send + 'static,
    W: FnOnce() -> anyhow::Result<T> + Send + 'static,
    D: FnOnce(&Shared, anyhow::Result<T>) + 'static,
{
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let _ = tx.send_blocking(work());
    });
    let st = st.clone();
    let _ = glib::spawn_future_local(async move {
        if let Ok(result) = rx.recv().await {
            done(&st, result);
        }
    });
}

/// Диалог выбора одного файла с фильтром по расширениям.
fn choose_file<F>(st: &Shared, title: &str, filter_name: &str, suffixes: &[&str], done: F)
where
    F: FnOnce(&Shared, PathBuf) + 'static,
{
    let filter = gtk::FileFilter::new();
    filter.set_name(Some(filter_name));
    for suffix in suffixes {
        filter.add_suffix(suffix);
    }
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);

    let dialog = gtk::FileDialog::new();
    dialog.set_title(title);
    dialog.set_modal(true);
    dialog.set_filters(Some(&filters));

    let window = st.window.clone();
    let st = st.clone();
    dialog.open(Some(&window), gio::Cancellable::NONE, move |result| {
        let Ok(file) = result else {
            return;
        };
        let Some(path) = file.path() else {
            st.toast(t(
                "Файл недоступен как локальный путь",
                "File is not available as a local path",
            ));
            return;
        };
        done(&st, path);
    });
}

// ---------- .exe / .msi ----------

/// Префикс по умолчанию: первый в списке; если списка нет, создаётся «Основной».
fn default_prefix(st: &Shared) -> Prefix {
    let first = st.config.borrow().prefixes.first().cloned();
    if let Some(prefix) = first {
        return prefix;
    }
    let prefix = Prefix::new(t("Основной", "Main"), &Prefix::default_root());
    st.config.borrow_mut().prefixes.push(prefix.clone());
    st.save();
    refresh_prefixes(st);
    prefix
}

fn handle_exe_paths(st: &Shared, paths: Vec<PathBuf>) {
    let exes: Vec<PathBuf> = paths
        .into_iter()
        .filter(|p| has_ext(p, &["exe", "msi"]))
        .collect();
    if exes.is_empty() {
        st.toast(t(
            "Нужен файл .exe или .msi",
            "An .exe or .msi file is required",
        ));
        return;
    }
    let prefix = default_prefix(st);
    launch(st, prefix, exes);
}

fn pick_exe_for_prefix(st: &Shared, idx: usize) {
    choose_file(
        st,
        t("Выберите программу", "Choose a program"),
        t(
            "Windows-программы (.exe, .msi)",
            "Windows programs (.exe, .msi)",
        ),
        &["exe", "msi"],
        move |st, path| {
            if let Some(prefix) = prefix_at(st, idx) {
                launch(st, prefix, vec![path]);
            }
        },
    );
}

/// Готовит префикс (если нужно) и запускает файлы, не блокируя окно.
fn launch(st: &Shared, prefix: Prefix, exes: Vec<PathBuf>) {
    // Запоминаем программу и сразу делаем ярлык, чтобы её можно было
    // запускать из приложения и с рабочего стола. Установщики .exe
    // (setup/unins/update) не записываем, а .msi записываем: для него
    // ярлык запускает msiexec с этим пакетом.
    let mut added = 0;
    for exe in &exes {
        let is_msi = has_ext(exe, &["msi"]);
        if !is_msi && (!has_ext(exe, &["exe"]) || looks_like_installer(exe)) {
            continue;
        }
        if let Some(app) = record_app(st, &prefix, exe) {
            let _ = shortcut::create_app_shortcut(&app);
            added += 1;
        }
    }
    if added > 0 {
        refresh_apps(st);
        st.toast(t(
            "Программа добавлена в список, ярлык создан",
            "Application added to the list, shortcut created",
        ));
    }

    st.toast(t(
        "Запускаю… при первом запуске префикс готовится дольше",
        "Launching… the first run takes longer while the prefix is prepared",
    ));
    run_background(
        st,
        move || {
            runner::ensure_prefix(&prefix)?;
            for exe in &exes {
                runner::run_exe(&prefix, exe)?;
            }
            Ok(exes.len())
        },
        |st, result| match result {
            Ok(n) => st.toast(&format!("{}: {n}", t("Запущено файлов", "Files launched"))),
            Err(e) => st.toast(&format!("{}: {e}", t("Ошибка запуска", "Launch failed"))),
        },
    );
}

// ---------- .deb ----------

fn handle_deb_paths(st: &Shared, paths: Vec<PathBuf>) {
    let debs: Vec<PathBuf> = paths.into_iter().filter(|p| has_ext(p, &["deb"])).collect();
    if debs.is_empty() {
        st.toast(t("Нужен файл .deb", "A .deb file is required"));
        return;
    }
    st.toast(t("Устанавливаю…", "Installing…"));

    run_background(
        st,
        move || {
            let mut installed = Vec::new();
            for path in &debs {
                installed.push(deb::install_file(path)?);
            }
            Ok(installed)
        },
        |st, result| match result {
            Ok(list) => {
                for info in &list {
                    let text = if i18n::current() == Lang::Ru {
                        format!("Установлено: {} (ярлыков: {})", info.name, info.launchers)
                    } else {
                        format!("Installed: {} (shortcuts: {})", info.name, info.launchers)
                    };
                    st.toast(&text);
                }
                if let Some(warning) = list.first().and_then(|i| i.warnings.first()) {
                    st.toast(warning);
                }
                refresh_deb_list(st);
            }
            Err(e) => st.toast(&format!(
                "{}: {e}",
                t("Ошибка установки", "Installation failed")
            )),
        },
    );
}

// ---------- .flatpak ----------

fn handle_flatpak_paths(st: &Shared, paths: Vec<PathBuf>) {
    let bundles: Vec<PathBuf> = paths
        .into_iter()
        .filter(|path| has_ext(path, &["flatpak"]))
        .collect();
    if bundles.is_empty() {
        st.toast(t("Нужен файл .flatpak", "A .flatpak file is required"));
        return;
    }
    st.toast(t("Устанавливаю Flatpak…", "Installing Flatpak…"));

    run_background(
        st,
        move || {
            let mut installed = Vec::new();
            for path in &bundles {
                installed.push(flatpak::install_file(path)?);
            }
            Ok(installed)
        },
        |st, result| match result {
            Ok(list) => {
                for app in &list {
                    let _ = shortcut::create_flatpak_shortcut(&app.id, &app.name);
                    st.toast(&format!("{}: {}", t("Установлено", "Installed"), app.name));
                }
                refresh_flatpak_list(st);
            }
            Err(e) => st.toast(&format!(
                "{}: {e}",
                t("Ошибка установки Flatpak", "Flatpak installation failed")
            )),
        },
    );
}

// ---------- префиксы ----------

fn add_prefix_dialog(st: &Shared) {
    let entry = gtk::Entry::builder()
        .placeholder_text(t("Например: Office", "For example: Office"))
        .activates_default(true)
        .build();

    let dialog = adw::MessageDialog::new(
        Some(&st.window),
        Some(t("Новый префикс", "New prefix")),
        Some(t("Имя префикса", "Prefix name")),
    );
    dialog.set_extra_child(Some(&entry));
    dialog.add_response("cancel", t("Отмена", "Cancel"));
    dialog.add_response("create", t("Создать", "Create"));
    dialog.set_response_appearance("create", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("create"));
    dialog.set_close_response("cancel");

    let st = st.clone();
    dialog.connect_response(None, move |_, response| {
        if response != "create" {
            return;
        }
        let name = entry.text().trim().to_string();
        if name.is_empty() {
            st.toast(t("Имя не может быть пустым", "Name cannot be empty"));
            return;
        }
        let exists = st.config.borrow().prefixes.iter().any(|p| p.name == name);
        if exists {
            st.toast(t(
                "Префикс с таким именем уже есть",
                "A prefix with this name already exists",
            ));
            return;
        }

        let prefix = Prefix::new(&name, &Prefix::default_root());
        st.config.borrow_mut().prefixes.push(prefix.clone());
        st.save();
        refresh_prefixes(&st);

        run_background(
            &st,
            move || runner::create_prefix(&prefix),
            |st, result| match result {
                Ok(()) => st.toast(t("Префикс создан", "Prefix created")),
                Err(e) => st.toast(&format!(
                    "{}: {e}",
                    t("Ошибка инициализации", "Initialization failed")
                )),
            },
        );
    });

    gtk4::prelude::GtkWindowExt::present(&dialog);
}

fn remove_prefix(st: &Shared, idx: usize) {
    {
        let mut cfg = st.config.borrow_mut();
        if idx >= cfg.prefixes.len() {
            return;
        }
        cfg.prefixes.remove(idx);
    }
    st.save();
    refresh_prefixes(st);
    st.toast(t(
        "Префикс убран из списка, файлы сохранены",
        "Prefix removed from the list, files are kept",
    ));
}

fn remove_app(st: &Shared, idx: usize) {
    let removed = {
        let mut cfg = st.config.borrow_mut();
        if idx < cfg.apps.len() {
            Some(cfg.apps.remove(idx))
        } else {
            None
        }
    };
    let Some(app) = removed else {
        return;
    };
    shortcut::remove_app_shortcut(&app);
    st.save();
    refresh_apps(st);
    st.toast(t(
        "Программа убрана из списка",
        "Application removed from the list",
    ));
}
