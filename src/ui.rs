use crate::state::{SharedState, TargetProcess};
use crate::backend;
use gtk::prelude::*;
use gtk::gdk_pixbuf::PixbufLoader;
use gtk::{
    Align, Application, ApplicationWindow, Box as GBox, Button, CssProvider,
    Label, ListBox, ListBoxRow, Orientation, ScrolledWindow, Scale, Adjustment,
    SearchEntry, SpinButton, Image, Expander, AlertDialog, CheckButton,
    Stack, Switch,
};
use std::process::Command;
use std::path::Path;
use std::collections::HashMap;
use std::fs;
lazy_static::lazy_static! {
    static ref ICON_CACHE: std::sync::Mutex<HashMap<String, String>> = std::sync::Mutex::new(HashMap::new());
}
fn find_icon_name(proc_name: &str) -> String {
    let proc_name = proc_name.trim();
    if proc_name.is_empty() { return "application-x-executable-symbolic".to_string(); }
    if let Ok(cache) = ICON_CACHE.lock() {
        if let Some(cached) = cache.get(proc_name) {
            return cached.clone();
        }
    }
    let theme = gtk::IconTheme::for_display(&gtk::gdk::Display::default().unwrap());
    let clean_name = proc_name.to_lowercase()
        .replace("-bin", "")
        .replace("-wrapper", "")
        .replace(".real", "");
    let variations = [
        proc_name.to_string(),
        proc_name.to_lowercase(),
        clean_name.clone(),
    ];
    for v in &variations {
        if theme.has_icon(v) {
            if let Ok(mut cache) = ICON_CACHE.lock() {
                cache.insert(proc_name.to_string(), v.clone());
            }
            return v.clone();
        }
    }
    let desktop_paths = [
        "/usr/share/applications",
        "/usr/local/share/applications",
        "/var/lib/flatpak/exports/share/applications",
    ];
    let home = std::env::var("HOME").unwrap_or_default();
    let user_desktop = format!("{}/.local/share/applications", home);
    let mut all_paths = desktop_paths.to_vec();
    all_paths.push(&user_desktop);
    for path in all_paths {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let file_name = entry.file_name().to_string_lossy().to_lowercase();
                if file_name.contains(&clean_name) && file_name.ends_with(".desktop") {
                    if let Ok(content) = fs::read_to_string(entry.path()) {
                        for line in content.lines() {
                            if line.starts_with("Icon=") {
                                let icon = line.replace("Icon=", "").trim().to_string();
                                if theme.has_icon(&icon) || Path::new(&icon).exists() {
                                    if let Ok(mut cache) = ICON_CACHE.lock() {
                                        cache.insert(proc_name.to_string(), icon.clone());
                                    }
                                    return icon;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    "application-x-executable-symbolic".to_string()
}
pub const CURRENT_VERSION: &str = "0.1.0";
const CSS: &str = "
    .main-window { background-color: @window_bg_color; }
    .main-box { padding: 20px; }
    .header-box { margin-bottom: 20px; }
    .app-title { font-size: 24px; font-weight: 800; letter-spacing: -0.5px; }
    .version-btn {
        font-size: 9px;
        font-weight: bold;
        padding: 1px 5px;
        border-radius: 6px;
        background-color: alpha(@window_fg_color, 0.1);
        color: alpha(@window_fg_color, 0.6);
        margin-left: 8px;
        border: none;
        min-height: 18px;
    }
    .version-btn:hover {
        background-color: alpha(@window_fg_color, 0.2);
        color: @window_fg_color;
    }
    .app-subtitle { font-size: 12px; margin-top: -2px; opacity: 0.6; }
    .app-subtitle a, .badge-link a { color: inherit; text-decoration: none; font-weight: bold; }
    .app-subtitle a:hover { opacity: 1.0; text-decoration: underline; }
    .status-badge { padding: 2px 8px; border-radius: 8px; font-size: 9px; font-weight: 800; text-align: center; }
    .status-badge.active { background-color: alpha(@accent_bg_color, 0.2); color: @accent_bg_color; }
    .status-badge.inactive { background-color: alpha(@window_fg_color, 0.1); color: alpha(@window_fg_color, 0.6); }
    .badge-link { padding: 2px 6px; border-radius: 6px; font-size: 9px; font-weight: 800; background-color: alpha(@window_fg_color, 0.06); color: alpha(@window_fg_color, 0.5); }
    .badge-link:hover { background-color: alpha(@window_fg_color, 0.12); color: @window_fg_color; }
    .card { background-color: alpha(@window_fg_color, 0.03); border: 1px solid alpha(@window_fg_color, 0.08); border-radius: 12px; margin-bottom: 10px; transition: all 200ms; }
    .card:hover { border-color: alpha(@accent_bg_color, 0.3); background-color: alpha(@window_fg_color, 0.05); }
    .target-row { padding: 12px; }
    .target-name { font-weight: bold; font-size: 14px; }
    .target-pid { font-size: 10px; opacity: 0.5; font-family: monospace; }
    .limit-label { font-size: 16px; font-weight: 800; color: @accent_color; }
    .cpu-label-active { font-size: 10px; font-weight: bold; color: @accent_color; opacity: 0.8; }
    .targets-list-container { 
        border: 1px solid alpha(@window_fg_color, 0.1); 
        border-radius: 16px; 
        background-color: alpha(@window_fg_color, 0.02);
        padding: 10px;
    }
    .targets-list-container list.transparent-list { background: transparent; border-radius: 12px; }
    .targets-list-container list.transparent-list > row { background: transparent; border: none; padding: 0; box-shadow: none; }
    .targets-list-container list.transparent-list > row:hover { background: transparent; }
    .no-targets-card {
        padding: 40px 20px;
        background-color: transparent;
        border: 1px dashed alpha(@window_fg_color, 0.1);
        border-radius: 12px;
        margin: 10px;
        box-shadow: none;
    }
    .process-dialog-scrolled {
        border: 1px solid alpha(@window_fg_color, 0.2);
        border-radius: 12px;
        background-color: alpha(@window_fg_color, 0.02);
    }
    .process-dialog-scrolled list {
        background: transparent;
        border-radius: 12px;
    }
    .refresh-btn {
        min-height: 38px;
        min-width: 38px;
        padding: 0;
        border-radius: 10px;
        background-color: alpha(@window_fg_color, 0.05);
        border: 1px solid alpha(@window_fg_color, 0.1);
    }
    .refresh-btn:hover {
        background-color: alpha(@window_fg_color, 0.1);
    }
    .accent-button { border-radius: 10px; padding: 10px; font-weight: bold; background-color: @accent_bg_color; color: @accent_fg_color; border: none; }
    .accent-button:hover { background-color: alpha(@accent_bg_color, 0.8); }
    .start-button, .stop-button { border-radius: 12px; padding: 12px; font-weight: 800; font-size: 14px; border: none; transition: all 300ms cubic-bezier(0.25, 1, 0.5, 1); margin-bottom: 10px; }
    .start-button { background-color: @accent_bg_color; color: @accent_fg_color; box-shadow: 0 4px 0px alpha(@accent_bg_color, 0.5); }
    .start-button:hover { background-color: alpha(@accent_bg_color, 0.9); transform: translateY(-2px); box-shadow: 0 6px 0px alpha(@accent_bg_color, 0.4); }
    .start-button:active { transform: translateY(2px); box-shadow: 0 2px 0px alpha(@accent_bg_color, 0.6); }
    .stop-button { background-color: #ff5555; color: white; box-shadow: 0 4px 0px #cc0000; }
    .stop-button:hover { background-color: #ff6666; transform: translateY(-2px); box-shadow: 0 6px 0px #cc0000; }
    .stop-button:active { transform: translateY(2px); box-shadow: 0 2px 0px #cc0000; }
    .process-card { 
        margin: 4px;
        border-radius: 10px;
        border: 1px solid alpha(@window_fg_color, 0.08);
        background-color: alpha(@window_fg_color, 0.02);
    }
    .process-card:hover {
        background-color: alpha(@window_fg_color, 0.04);
        border-color: alpha(@accent_bg_color, 0.3);
    }
    .process-row-content { 
        padding: 0 12px 0 15px;
        min-height: 54px;
    }
    .process-spacer {
        min-width: 0px; 
    }
    .process-cpu { 
        font-weight: 800; 
        font-size: 12px; 
        color: @accent_color; 
        margin-right: 12px; 
        min-width: 55px; 
        text-align: right; 
    }
    .target-btn-small { 
        font-size: 11px; 
        font-weight: bold; 
        padding: 0;
        border-radius: 8px; 
        min-width: 75px; 
        min-height: 34px; 
    }
    expander { 
        margin: 4px; 
        border-radius: 10px; 
        border: 1px solid alpha(@window_fg_color, 0.08); 
        background: alpha(@window_fg_color, 0.02); 
    }
    expander:hover {
        background: alpha(@window_fg_color, 0.04);
        border-color: alpha(@accent_bg_color, 0.3);
    }
    expander > title { 
        padding: 0;
        min-height: 54px;
    }
    expander > title > arrow { 
        margin: 0 25px 0 4px; 
        padding: 0;
    }
    expander > box { padding: 0; }
    expander > list { 
        background: alpha(@window_fg_color, 0.03); 
        padding: 0; 
        margin: 0; 
        border-top: 1px solid alpha(@window_fg_color, 0.05);
    }
    .expander-header-content { 
        padding-right: 12px;
        min-height: 54px;
    }
    .sub-process-row {
        padding: 0 12px 0 45px; 
        min-height: 48px;
        border-bottom: 1px solid alpha(@window_fg_color, 0.03);
    }
    .sub-process-row:last-child {
        border-bottom: none;
    }
    .limit-all-btn { margin-left: 8px; }
    .section-title { font-size: 10px; font-weight: bold; text-transform: uppercase; letter-spacing: 0.5px; opacity: 0.5; margin-bottom: 8px; margin-top: 4px; }
    .settings-row { padding: 8px 14px; border-bottom: 1px solid alpha(@window_fg_color, 0.04); transition: background-color 200ms ease; }
    .settings-row:hover { background-color: alpha(@window_fg_color, 0.02); }
    .row-title { font-weight: 500; font-size: 14px; }
    .row-subtitle { font-size: 11px; opacity: 0.5; }
    switch { transform: scale(0.85); }
    .compat-title { font-size: 16px; font-weight: 800; margin-bottom: 0px; opacity: 0.9; }
    .compat-item { padding: 12px; border-radius: 12px; background: alpha(@window_fg_color, 0.03); border: 1px solid alpha(@window_fg_color, 0.06); margin-bottom: 8px; transition: all 300ms ease; }
    .compat-item:hover { background-color: alpha(@window_fg_color, 0.05); }
    .compat-item.error { border-left: 4px solid #ff5555; }
    .compat-item.ok { border-left: 4px solid #8fde58; }
    .compat-item.warning-item { border-left: 4px solid #f5c71a; }
    .compat-item.info-item { border-left: 4px solid #3584e4; }
    .tutorial-text { font-size: 11px; opacity: 0.6; margin-top: 4px; }
    .compat-name { font-size: 13px; font-weight: bold; }
    .beta-badge { font-size: 9px; font-weight: 800; padding: 1px 5px; border-radius: 5px; background-color: #f5c71a; color: #000; margin-left: 0px; }
    .info-icon { opacity: 0.4; transition: opacity 200ms; }
    .info-icon:hover { opacity: 0.9; }
";
fn create_row(
    title: &str,
    subtitle: Option<&str>,
    widget: &impl IsA<gtk::Widget>,
    info_text: Option<&str>,
    is_beta: bool,
) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.add_css_class("settings-row");
    let main_hbox = GBox::new(Orientation::Horizontal, 12);
    main_hbox.set_valign(Align::Center);
    main_hbox.set_hexpand(true);
    let text_vbox = GBox::new(Orientation::Vertical, 0);
    text_vbox.set_valign(Align::Center);
    let title_hbox = GBox::new(Orientation::Horizontal, 6);
    title_hbox.set_valign(Align::Center);
    if let Some(txt) = info_text {
        let info_img = Image::from_icon_name("info-symbolic");
        info_img.add_css_class("info-icon");
        info_img.set_tooltip_text(Some(txt));
        title_hbox.append(&info_img);
    }
    title_hbox.append(
        &Label::builder()
            .label(title)
            .halign(Align::Start)
            .css_classes(["row-title"])
            .wrap(true)
            .xalign(0.0)
            .build(),
    );
    if is_beta {
        title_hbox.append(
            &Label::builder()
                .label("BETA")
                .css_classes(["beta-badge"])
                .valign(Align::Center)
                .build(),
        );
    }
    text_vbox.append(&title_hbox);
    if let Some(sub) = subtitle {
        let sub_label = Label::builder()
            .label(sub)
            .halign(Align::Start)
            .css_classes(["row-subtitle"])
            .wrap(true)
            .xalign(0.0)
            .build();
        text_vbox.append(&sub_label);
    }
    main_hbox.append(&text_vbox);
    let filler = GBox::new(Orientation::Horizontal, 0);
    filler.set_hexpand(true);
    main_hbox.append(&filler);
    widget.set_valign(Align::Center);
    main_hbox.append(widget);
    row.set_child(Some(&main_hbox));
    row.set_activatable(false);
    row.set_selectable(false);
    row
}
fn check_latest_version() -> Result<(bool, String), String> {
    let remote_v = Command::new("curl")
        .args([
            "-s",
            "--fail",
            "--connect-timeout",
            "3",
            "https://raw.githubusercontent.com/Agzes/LBS/main/version",
        ])
        .output();
    if let Ok(output) = remote_v
        && output.status.success()
    {
        let latest_v_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !latest_v_str.is_empty() && !latest_v_str.contains("Not Found") {
            return Ok((CURRENT_VERSION == latest_v_str, latest_v_str));
        }
    }
    Err("Failed to check for updates".to_string())
}
pub fn build_ui(app: &Application, state: SharedState) -> ApplicationWindow {
    let initial_state = { state.lock().unwrap().clone() };
    let provider = CssProvider::new();
    provider.load_from_data(CSS);
    gtk::style_context_add_provider_for_display(
        &gtk::gdk::Display::default().expect("Display error"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    let window = ApplicationWindow::builder()
        .application(app)
        .title("LBS - Linux Battle Shaper")
        .default_width(380)
        .default_height(600)
        .build();

    let state_close = state.clone();
    let app_c = app.clone();
    window.connect_close_request(move |w| {
        let close_to_tray = {
            state_close.lock().unwrap().close_to_tray
        };
        if close_to_tray {
            w.set_visible(false);
            glib::Propagation::Stop
        } else {
            app_c.quit();
            glib::Propagation::Proceed
        }
    });
    let root_vbox = GBox::new(Orientation::Vertical, 0);
    root_vbox.add_css_class("main-box");
    window.set_child(Some(&root_vbox));
    let header_box = GBox::new(Orientation::Horizontal, 8);
    header_box.add_css_class("header-box");
    let pb_loader = PixbufLoader::new();
    pb_loader.write(include_bytes!("../assets/logo.png")).ok();
    pb_loader.close().ok();
    let logo_img = Image::builder().pixel_size(44).build();
    if let Some(pb) = pb_loader.pixbuf() {
        logo_img.set_from_pixbuf(Some(&pb));
    }
    header_box.append(&logo_img);
    let title_vbox = GBox::new(Orientation::Vertical, 0);
    let title_hbox = GBox::new(Orientation::Horizontal, 0);
    title_hbox.set_valign(Align::Center);
    title_hbox.append(&Label::builder().label("LBS").css_classes(["app-title"]).build());
    let version_btn = Button::builder()
        .css_classes(["version-btn"])
        .valign(Align::Center)
        .has_frame(false)
        .build();
    let btn_content = GBox::new(Orientation::Horizontal, 4);
    btn_content.append(
        &Label::builder()
            .label(format!("v{CURRENT_VERSION}"))
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build(),
    );
    let settings_icon = Image::from_icon_name("preferences-system-symbolic");
    settings_icon.set_pixel_size(15);
    btn_content.append(&settings_icon);
    version_btn.set_child(Some(&btn_content));
    title_hbox.append(&version_btn);
    title_vbox.append(&title_hbox);
    title_vbox.append(&Label::builder()
        .use_markup(true)
        .label("<b>Linux Battle Shaper</b> • by <a href='https://github.com/agzes'>agzes</a>")
        .halign(Align::Start)
        .css_classes(["app-subtitle"])
        .build());
    header_box.append(&title_vbox);
    let filler = GBox::new(Orientation::Horizontal, 0);
    filler.set_hexpand(true);
    header_box.append(&filler);
    let right_vbox = GBox::new(Orientation::Vertical, 4);
    right_vbox.set_valign(Align::Center);
    right_vbox.set_halign(Align::End);
    let is_active = !initial_state.targets.is_empty();
    let status_badge = Label::builder()
        .label(if is_active { "ACTIVE" } else { "IDLE" })
        .css_classes(["status-badge", if is_active { "active" } else { "inactive" }])
        .halign(Align::End)
        .build();
    right_vbox.append(&status_badge);
    right_vbox.append(&Label::builder()
        .use_markup(true)
        .label("<a href='https://github.com/agzes/LBS'>GITHUB</a>")
        .css_classes(["badge-link"])
        .halign(Align::End)
        .build());
    header_box.append(&right_vbox);
    root_vbox.append(&header_box);
    let stack = Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .transition_duration(400)
        .vexpand(true)
        .hexpand(true)
        .build();
    root_vbox.append(&stack);
    let main_vbox = GBox::new(Orientation::Vertical, 0);
    let scrolled = ScrolledWindow::builder().vexpand(true).hscrollbar_policy(gtk::PolicyType::Never).build();
    let list_vbox = GBox::builder().orientation(Orientation::Vertical).css_classes(["targets-list-container"]).build();
    let targets_list = ListBox::new();
    targets_list.add_css_class("transparent-list");
    targets_list.set_selection_mode(gtk::SelectionMode::None);
    list_vbox.append(&targets_list);
    scrolled.set_child(Some(&list_vbox));
    main_vbox.append(&scrolled);
    let state_c = state.clone();
    let list_c = targets_list.clone();
    let status_badge_c = status_badge.clone();
    refresh_targets(&list_c, state_c.clone(), status_badge_c.clone());
    let footer_vbox = GBox::new(Orientation::Vertical, 10);
    footer_vbox.set_margin_top(20);
    let btn_hbox = GBox::new(Orientation::Horizontal, 10);
    let target_btn = Button::builder().label("Target...").css_classes(["start-button"]).hexpand(true).build();
    let unlimit_btn = Button::builder().label("Unlimit all").css_classes(["stop-button"]).hexpand(true).build();
    btn_hbox.append(&target_btn);
    btn_hbox.append(&unlimit_btn);
    footer_vbox.append(&btn_hbox);
    main_vbox.append(&footer_vbox);
    stack.add_named(&main_vbox, Some("main"));
    let settings_scrolled = ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).build();
    let settings_vbox = GBox::new(Orientation::Vertical, 0);
    settings_scrolled.set_child(Some(&settings_vbox));
    stack.add_named(&settings_scrolled, Some("settings"));
    let state_settings = state.clone();
    let stack_settings = stack.clone();
    let badge_settings = status_badge.clone();
    let window_settings = window.clone();
    let list_settings = targets_list.clone();
    version_btn.connect_clicked(move |_| {
        if stack_settings.visible_child_name().as_deref() == Some("settings") {
            stack_settings.set_visible_child_name("main");
        } else {
            build_settings_ui(
                window_settings.clone(),
                &settings_vbox,
                state_settings.clone(),
                stack_settings.clone(),
                badge_settings.clone(),
                list_settings.clone(),
            );
            stack_settings.set_visible_child_name("settings");
        }
    });
    let state_dialog = state.clone();
    let main_list_dialog = targets_list.clone();
    let badge_dialog = status_badge.clone();
    let window_dialog = window.clone();
    target_btn.connect_clicked(move |_| {
        show_process_dialog(window_dialog.upcast_ref(), state_dialog.clone(), main_list_dialog.clone(), badge_dialog.clone());
    });
    let state_unlimit = state.clone();
    let list_unlimit = targets_list.clone();
    let badge_unlimit = status_badge.clone();
    unlimit_btn.connect_clicked(move |_| {
        if let Ok(mut s) = state_unlimit.lock() {
            s.targets.clear();
            s.save();
        }
        refresh_targets(&list_unlimit, state_unlimit.clone(), badge_unlimit.clone());
    });
    let state_timer = state.clone();
    let list_timer = targets_list.clone();
    glib::timeout_add_local(std::time::Duration::from_secs(1), move || {
        let (available,) = {
            let s = state_timer.lock().unwrap();
            (s.available_processes.clone(),)
        };
        let mut child = list_timer.first_child();
        while let Some(row) = child {
            if let Some(lb_row) = row.downcast_ref::<ListBoxRow>() {
                if let Some(card) = lb_row.child().and_then(|c| c.downcast::<GBox>().ok()) {
                    if let Some(header) = card.first_child().and_then(|h| h.downcast::<GBox>().ok()) {
                        if let Some(vbox) = header.first_child().and_then(|v| v.downcast::<GBox>().ok()) {
                            let mut inner = vbox.first_child(); 
                            let name_label = inner.as_ref().and_then(|i| i.downcast_ref::<Label>());
                            let name_text = name_label.map(|l| l.label().to_string()).unwrap_or_default();
                            inner = inner.and_then(|i| i.next_sibling()); 
                            if let Some(pid_label) = inner.and_then(|i| i.downcast::<Label>().ok()) {
                                let pid_text = pid_label.label();
                                let pid_raw = pid_text.split(':').last().map(|s| s.trim()).unwrap_or("");
                                let mut cpu_val = 0.0;
                                if pid_raw == "ALL" {
                                    for (name, _, total_cpu) in &available {
                                        if name == &name_text {
                                            cpu_val = *total_cpu; break;
                                        }
                                    }
                                } else if let Ok(pid) = pid_raw.parse::<u32>() {
                                    for (_, pids, _) in &available {
                                        if let Some((_, c, _)) = pids.iter().find(|(p, _, _)| *p == pid) {
                                            cpu_val = *c; break;
                                        }
                                    }
                                }
                                if let Some(cpu_label) = pid_label.next_sibling().and_then(|i| i.downcast::<Label>().ok()) {
                                    cpu_label.set_label(&format!("{:.1}% CPU", cpu_val));
                                }
                            }
                        }
                    }
                }
            }
            child = row.next_sibling();
        }
        glib::ControlFlow::Continue
    });
    let desktop_scrolled = ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).build();
    let desktop_vbox = GBox::new(Orientation::Vertical, 0);
    desktop_scrolled.set_child(Some(&desktop_vbox));
    stack.add_named(&desktop_scrolled, Some("desktop"));

    let desktop_spacer_top = GBox::new(Orientation::Vertical, 0);
    desktop_spacer_top.set_vexpand(true);
    desktop_vbox.append(&desktop_spacer_top);

    let desktop_content = GBox::new(Orientation::Vertical, 0);
    desktop_content.set_halign(Align::Center);

    let d_icon = Image::from_icon_name("system-run-symbolic");
    d_icon.set_pixel_size(80);
    d_icon.set_margin_bottom(20);
    d_icon.set_halign(Align::Center);
    desktop_content.append(&d_icon);

    desktop_content.append(&Label::builder().label("Desktop Integration").css_classes(["compat-title"]).halign(Align::Center).build());
    desktop_content.append(&Label::builder().label("Add LBS to your application menu for easier access.").css_classes(["app-subtitle"]).halign(Align::Center).build());

    desktop_vbox.append(&desktop_content);

    let desktop_spacer_bottom = GBox::new(Orientation::Vertical, 0);
    desktop_spacer_bottom.set_vexpand(true);
    desktop_vbox.append(&desktop_spacer_bottom);

    let install_d_btn = Button::builder().label("Install Desktop File").css_classes(["start-button"]).build();
    let skip_d_btn = Button::builder().label("Skip Integration").css_classes(["version-btn"]).halign(Align::Center).margin_bottom(12).build();

    let s_c_d = stack.clone(); let st_c_d = state.clone();
    install_d_btn.connect_clicked(move |_| {
        if backend::setup_desktop_file() {
            if let Ok(mut s) = st_c_d.lock() { s.desktop_installed = true; s.save(); }
            s_c_d.set_visible_child_name("main");
        }
    });

    let s_c_d2 = stack.clone(); let st_c_d2 = state.clone();
    skip_d_btn.connect_clicked(move |_| {
        if let Ok(mut s) = st_c_d2.lock() { s.desktop_installed = true; s.save(); }
        s_c_d2.set_visible_child_name("main");
    });

    desktop_vbox.append(&skip_d_btn);
    desktop_vbox.append(&install_d_btn);

    let (shown_warning, desktop_installed, last_v) = {
        let s = state.lock().unwrap();
        (s.shown_warning, s.desktop_installed, s.last_run_version.clone())
    };

    let desktop_exists = backend::desktop_file_exists();
    let is_system = backend::is_system_install();

    if !shown_warning && !is_system {
        stack.set_visible_child_name("main"); 
    } else if !desktop_installed && !desktop_exists && !is_system {
        stack.set_visible_child_name("desktop");
    } else if last_v.is_none_or(|v| v != CURRENT_VERSION) {
        stack.set_visible_child_name("main");
    } else {
        stack.set_visible_child_name("main");
    }

    window.present();
    window
}
#[derive(Clone, Copy)]
enum ItemStatus {
    Ok,
    #[allow(dead_code)]
    Error,
    Warning,
    Info,
}
fn add_compat_item(
    name: &str,
    tutorial: &str,
    widget: Option<gtk::Widget>,
    status: ItemStatus,
) -> GBox {
    let item = GBox::new(Orientation::Vertical, 2);
    item.add_css_class("compat-item");
    let icon_name = match status {
        ItemStatus::Ok => {
            item.add_css_class("ok");
            "emblem-ok-symbolic"
        }
        ItemStatus::Error => {
            item.add_css_class("error");
            "dialog-error-symbolic"
        }
        ItemStatus::Warning => {
            item.add_css_class("warning-item");
            "dialog-warning-symbolic"
        }
        ItemStatus::Info => {
            item.add_css_class("info-item");
            "dialog-information-symbolic"
        }
    };
    let header = GBox::new(Orientation::Horizontal, 10);
    header.append(&Image::from_icon_name(icon_name));
    header.append(
        &Label::builder()
            .label(name)
            .css_classes(["compat-name"])
            .wrap(true)
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build(),
    );
    let filler = GBox::new(Orientation::Horizontal, 0);
    filler.set_hexpand(true);
    header.append(&filler);
    if let Some(w) = widget {
        header.append(&w);
    }
    item.append(&header);
    let tut = Label::builder()
        .label(tutorial)
        .css_classes(["tutorial-text"])
        .halign(Align::Start)
        .wrap(true)
        .xalign(0.0)
        .build();
    item.append(&tut);
    item
}
fn build_settings_ui(
    window: ApplicationWindow,
    container: &GBox,
    state: SharedState,
    stack: Stack,
    badge: Label,
    main_list: ListBox,
) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
    container.append(
        &Label::builder()
            .label("Settings")
            .css_classes(["compat-title"])
            .halign(Align::Start)
            .build(),
    );
    let (unlimit_val, close_to_tray_val, allow_99_val, check_up_val, awake_val) = {
        let s = state.lock().unwrap();
        (
            s.unlimit_at_focus,
            s.close_to_tray,
            s.allow_limit_99,
            s.check_updates,
            s.awake_cycle_ms,
        )
    };
    if check_up_val {
        container.append(
            &Label::builder()
                .label("Compatibility")
                .css_classes(["section-title"])
                .halign(Align::Start)
                .build(),
        );
        let compat_box = GBox::new(Orientation::Vertical, 0);
        container.append(&compat_box);

        compat_box.append(&add_compat_item(
            "Application Version",
            "Checking for updates...",
            None,
            ItemStatus::Ok,
        ));

        let desktop_ok = backend::desktop_file_exists();
        let is_system = backend::is_system_install();

        if !is_system {
            let d_status = if desktop_ok { ItemStatus::Ok } else { ItemStatus::Warning };
            let d_fix = if !desktop_ok {
                let fix_btn = Button::builder().label("FIX").css_classes(["version-btn"]).valign(Align::Center).build();
                let st_c = state.clone(); let c_c = container.clone(); let w_c = window.clone();
                let s_c = stack.clone(); let b_c = badge.clone(); let ml_c = main_list.clone();
                fix_btn.connect_clicked(move |_| {
                    if backend::setup_desktop_file() {
                        if let Ok(mut s) = st_c.lock() { s.desktop_installed = true; s.save(); }
                        build_settings_ui(w_c.clone(), &c_c, st_c.clone(), s_c.clone(), b_c.clone(), ml_c.clone());
                    }
                });
                Some(fix_btn.upcast::<gtk::Widget>())
            } else { None };

            compat_box.append(&add_compat_item(
                "Desktop Integration",
                "Application shortcut and terminal command status.",
                d_fix,
                d_status,
            ));
        }

        let (tx, rx) = glib::MainContext::channel::<Result<(bool, String), String>>(glib::Priority::DEFAULT);
        std::thread::spawn(move || {
            let res = check_latest_version();
            let _ = tx.send(res);
        });
        let cb_v = compat_box.clone();
        let st_v = state.clone();
        let stack_v = stack.clone();
        let badge_v = badge.clone();
        let container_v = container.clone();
        let window_v = window.clone();
        let ml_v = main_list.clone();

        rx.attach(None, move |result| {
            let render_version = |res: Result<(bool, String), String>,
                                  cb: GBox,
                                  s: Stack,
                                  st: SharedState,
                                  c: GBox,
                                  sb: Label,
                                  w: ApplicationWindow,
                                  ml: ListBox| {
                let mut version_item = None;
                let mut child = cb.first_child();
                while let Some(c) = child {
                    if let Some(lbl) = c.first_child()
                        .and_then(|h| h.first_child()) 
                        .and_then(|i| i.next_sibling()) 
                        .and_then(|n| n.downcast::<Label>().ok()) 
                    {
                        if lbl.label() == "Application Version" {
                            version_item = Some(c);
                            break;
                        }
                    }
                    child = c.next_sibling();
                }

                if let Some(item) = version_item {
                    cb.remove(&item);
                }

                match res {
                    Ok((version_ok, latest_v)) => {
                        let version_tutorial = if version_ok {
                            format!("Current: v{CURRENT_VERSION}. You have the latest version.")
                        } else {
                            format!("Update available: v{latest_v}. Visit GitHub to download.")
                        };
                        let check_btn = Button::builder()
                            .label(if version_ok { "CHECK" } else { "UPDATE" })
                            .css_classes(["version-btn"])
                            .valign(Align::Center)
                            .build();
                        let s_c = s.clone();
                        let st_c = st.clone();
                        let c_c = c.clone();
                        let sb_c = sb.clone();
                        let w_c = w.clone();
                        let ml_c = ml.clone();
                        let v_ok = version_ok;
                        check_btn.connect_clicked(move |_| {
                            if v_ok {
                                build_settings_ui(
                                    w_c.clone(),
                                    &c_c,
                                    st_c.clone(),
                                    s_c.clone(),
                                    sb_c.clone(),
                                    ml_c.clone(),
                                );
                            } else {
                                let _ = Command::new("xdg-open")
                                    .arg("https://github.com/Agzes/LBS/releases")
                                    .spawn();
                            }
                        });
                        let status = if version_ok {
                            ItemStatus::Ok
                        } else {
                            ItemStatus::Info
                        };
                        cb.prepend(&add_compat_item(
                            "Application Version",
                            &version_tutorial,
                            Some(check_btn.upcast()),
                            status,
                        ));
                    }
                    Err(e) => {
                        let retry_btn = Button::builder()
                            .label("RETRY")
                            .css_classes(["version-btn"])
                            .valign(Align::Center)
                            .build();
                        let s_c = s.clone();
                        let st_c = st.clone();
                        let c_c = c.clone();
                        let sb_c = sb.clone();
                        let w_c = w.clone();
                        let ml_c = ml.clone();
                        retry_btn.connect_clicked(move |_| {
                            build_settings_ui(
                                w_c.clone(),
                                &c_c,
                                st_c.clone(),
                                s_c.clone(),
                                sb_c.clone(),
                                ml_c.clone(),
                            );
                        });
                        cb.prepend(&add_compat_item(
                            "Application Version",
                            &e,
                            Some(retry_btn.upcast()),
                            ItemStatus::Warning,
                        ));
                    }
                }
            };

            render_version(
                result,
                cb_v.clone(),
                stack_v.clone(),
                st_v.clone(),
                container_v.clone(),
                badge_v.clone(),
                window_v.clone(),
                ml_v.clone(),
            );

            glib::ControlFlow::Break
        });
    }
    container.append(
        &Label::builder()
            .label("General")
            .css_classes(["section-title"])
            .halign(Align::Start)
            .build(),
    );
    let list = ListBox::new();
    list.add_css_class("card");
    container.append(&list);
    let unlimit_sw = Switch::new();
    unlimit_sw.set_active(unlimit_val);
    let st_unlimit = state.clone();
    unlimit_sw.connect_state_set(move |_, val| {
        let mut s = st_unlimit.lock().unwrap();
        s.unlimit_at_focus = val;
        s.save();
        glib::Propagation::Proceed
    });
    list.append(&create_row(
        "Unlimit at Focus",
        Some("Temporarily unlock app when focused"),
        &unlimit_sw,
        Some("This feature may not work on some Wayland compositors (like GNOME) due to security restrictions."),
        true,
    ));
    let close_tray_sw = Switch::new();
    close_tray_sw.set_active(close_to_tray_val);
    let st_close = state.clone();
    close_tray_sw.connect_state_set(move |_, val| {
        let mut s = st_close.lock().unwrap();
        s.close_to_tray = val;
        s.save();
        glib::Propagation::Proceed
    });
    list.append(&create_row(
        "Close to Tray",
        Some("Hide window instead of quitting"),
        &close_tray_sw,
        None,
        false,
    ));
    let allow_99_sw = Switch::new();
    allow_99_sw.set_active(allow_99_val);
    let st_99 = state.clone();
    let ml_99 = main_list.clone();
    let badge_99 = badge.clone();
    allow_99_sw.connect_state_set(move |_, val| {
        {
            let mut s = st_99.lock().unwrap();
            s.allow_limit_99 = val;
            s.save();
        }
        refresh_targets(&ml_99, st_99.clone(), badge_99.clone());
        glib::Propagation::Proceed
    });
    list.append(&create_row(
        "Allow limit to 99.9%",
        Some("Enable deeper CPU throttling"),
        &allow_99_sw,
        None,
        false,
    ));
    let check_up_sw = Switch::new();
    check_up_sw.set_active(check_up_val);
    let st_up = state.clone();
    check_up_sw.connect_state_set(move |_, val| {
        let mut s = st_up.lock().unwrap();
        s.check_updates = val;
        s.save();
        glib::Propagation::Proceed
    });
    list.append(&create_row(
        "Check Updates",
        Some("Automatically check for new versions"),
        &check_up_sw,
        None,
        false,
    ));
    let awake_hbox = GBox::new(Orientation::Horizontal, 8);
    let awake_adj = Adjustment::new(awake_val as f64, 2.0, 400.0, 1.0, 10.0, 0.0);
    let awake_spin = SpinButton::new(Some(&awake_adj), 1.0, 0);
    awake_spin.set_valign(Align::Center);
    awake_hbox.append(&awake_spin);
    awake_hbox.append(&Label::builder().label("ms").valign(Align::Center).build());
    let st_awake = state.clone();
    awake_adj.connect_value_changed(move |a| {
        let mut s = st_awake.lock().unwrap();
        s.awake_cycle_ms = a.value() as u32;
        s.save();
    });
    list.append(&create_row(
        "Awake Cycle",
        Some("Frequency of process limiting (2-400ms)"),
        &awake_hbox,
        None,
        false,
    ));
    let spacer = GBox::new(Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    container.append(&spacer);

    let uninstall_btn = Button::builder()
        .label("Uninstall (Remove all traces)")
        .css_classes(["stop-button"])
        .margin_bottom(2)
        .build();
    let state_un = state.clone();
    uninstall_btn.connect_clicked(move |btn| {
        if backend::uninstall() {
            btn.set_label("Uninstalled successfully!");
            btn.set_sensitive(false);
            if let Ok(mut s) = state_un.lock() {
                s.auto_start = false;
                s.save();
            }
        }
    });
    container.append(&uninstall_btn);

    let back_btn = Button::builder().label("Return to Dashboard").css_classes(["start-button"]).margin_top(8).build();
    let st_back = stack.clone();
    back_btn.connect_clicked(move |_| {
        st_back.set_visible_child_name("main");
    });
    container.append(&back_btn);
}
fn refresh_targets(list: &ListBox, state: SharedState, status_badge: Label) {
    while let Some(child) = list.first_child() { list.remove(&child); }
    let targets = { state.lock().unwrap().targets.clone() };
    if targets.is_empty() {
        status_badge.set_label("IDLE"); status_badge.add_css_class("inactive"); status_badge.remove_css_class("active");
        let box_empty = GBox::new(Orientation::Vertical, 0);
        box_empty.add_css_class("no-targets-card");
        box_empty.append(&Label::builder().label("No active targets.").css_classes(["app-subtitle"]).build());
        list.append(&box_empty);
    } else {
        status_badge.set_label("ACTIVE"); status_badge.add_css_class("active"); status_badge.remove_css_class("inactive");
        for target in targets {
            list.append(&create_target_row(target, state.clone(), list.clone(), status_badge.clone()));
        }
    }
}
fn create_target_row(target: TargetProcess, state: SharedState, list: ListBox, status_badge: Label) -> ListBoxRow {
    let row = ListBoxRow::new();
    let card = GBox::new(Orientation::Vertical, 8);
    card.add_css_class("card"); card.add_css_class("target-row");
    let header = GBox::new(Orientation::Horizontal, 10);
    let title_vbox = GBox::new(Orientation::Vertical, 0);
    title_vbox.append(&Label::builder().label(&target.name).halign(Align::Start).css_classes(["target-name"]).build());
    let pid_label_text = if target.pid == 0 { "PID: ALL".to_string() } else { format!("PID: {}", target.pid) };
    title_vbox.append(&Label::builder().label(&pid_label_text).halign(Align::Start).css_classes(["target-pid"]).build());
    title_vbox.append(&Label::builder().label("0.0% CPU").halign(Align::Start).css_classes(["cpu-label-active"]).build());
    header.append(&title_vbox);
    let filler = GBox::new(Orientation::Horizontal, 0); filler.set_hexpand(true); header.append(&filler);
    let limit_label = Label::builder().label(&format!("-{}%", target.limit_percent)).css_classes(["limit-label"]).build();
    header.append(&limit_label);
    let active_sw = Switch::new();
    active_sw.set_active(target.is_active);
    active_sw.set_valign(Align::Center);
    header.append(&active_sw);
    let del_btn = Button::builder().icon_name("user-trash-symbolic").has_frame(false).build();
    header.append(&del_btn);
    card.append(&header);
    let (max_val, step) = {
        let s = state.lock().unwrap();
        if s.allow_limit_99 { (99.9, 0.1) } else { (99.0, 1.0) }
    };
    let adj = Adjustment::new(target.limit_percent as f64, 0.0, max_val, step, 10.0, 0.0);
    let scale = Scale::new(Orientation::Horizontal, Some(&adj)); scale.set_draw_value(false);
    if step < 1.0 { scale.set_digits(1); }
    scale.set_sensitive(target.is_active);
    card.append(&scale);
    let state_sw = state.clone(); let pid_sw = target.pid; let name_sw = target.name.clone();
    let scale_sw = scale.clone();
    active_sw.connect_state_set(move |_, val| {
        if let Ok(mut s) = state_sw.lock() {
            if let Some(t) = s.targets.iter_mut().find(|t| t.pid == pid_sw && t.name == name_sw) {
                t.is_active = val;
            }
            s.save();
        }
        scale_sw.set_sensitive(val);
        glib::Propagation::Proceed
    });
    let state_c = state.clone(); let pid = target.pid; let name = target.name.clone(); let limit_c = limit_label.clone();
    adj.connect_value_changed(move |a| {
        let val = a.value();
        let display_val = if step < 1.0 { format!("-{:.1}%", val) } else { format!("-{}%", val.round() as i32) };
        limit_c.set_label(&display_val);
        if let Ok(mut s) = state_c.lock() {
            if let Some(t) = s.targets.iter_mut().find(|t| t.pid == pid && t.name == name) {
                t.limit_percent = val;
            }
            s.save();
        }
    });
    let state_del = state.clone(); let list_del = list.clone(); let badge_del = status_badge.clone();
    let name_del = target.name.clone();
    del_btn.connect_clicked(move |_| {
        if let Ok(mut s) = state_del.lock() { s.targets.retain(|t| !(t.pid == pid && t.name == name_del)); s.save(); }
        refresh_targets(&list_del, state_del.clone(), badge_del.clone());
    });
    row.set_child(Some(&card));
    row
}
fn show_process_dialog(parent: &gtk::Window, state: SharedState, main_list: ListBox, badge: Label) {
    let dialog = gtk::Window::builder().title("Select Process").default_width(640).default_height(540).modal(true).transient_for(parent).build();
    let vbox = GBox::new(Orientation::Vertical, 10);
    vbox.set_margin_start(12); vbox.set_margin_end(12); vbox.set_margin_top(12); vbox.set_margin_bottom(12);
    dialog.set_child(Some(&vbox));
    let search = SearchEntry::new(); 
    let sys_toggle = CheckButton::builder().label("Show system processes").active(false).build();
    let auto_toggle = CheckButton::builder().label("Auto-refresh").active(true).build();
    let update_btn = Button::builder()
        .icon_name("view-refresh-symbolic")
        .css_classes(["refresh-btn"])
        .valign(Align::Center)
        .build();
    let top_hbox = GBox::new(Orientation::Horizontal, 10);
    top_hbox.set_valign(Align::Center);
    top_hbox.append(&search); search.set_hexpand(true);
    top_hbox.append(&update_btn);
    vbox.append(&top_hbox);
    let scrolled = ScrolledWindow::builder().vexpand(true).build();
    scrolled.add_css_class("process-dialog-scrolled");
    let process_list = ListBox::new(); scrolled.set_child(Some(&process_list)); vbox.append(&scrolled);
    let bottom_hbox = GBox::new(Orientation::Horizontal, 20);
    bottom_hbox.set_halign(Align::Center);
    bottom_hbox.append(&sys_toggle);
    bottom_hbox.append(&auto_toggle);
    vbox.append(&bottom_hbox);
    let state_c = state.clone(); let main_list_c = main_list.clone(); let badge_c = badge.clone();
    let dialog_c = dialog.clone(); let search_c = search.clone(); let list_c = process_list.clone();
    let sys_toggle_c = sys_toggle.clone();
    let refresh = move || {
        while let Some(child) = list_c.first_child() { list_c.remove(&child); }
        let filter = search_c.text().to_lowercase();
        let show_system = sys_toggle_c.is_active();
        let grouped = { state_c.lock().unwrap().available_processes.clone() };
        for (name, pids, total_cpu) in grouped {
            let name_lower = name.to_lowercase();
            if name_lower.contains("lbs") { continue; }
            let is_system = pids.iter().any(|(p, _, k)| *p < 1000 || *k);
            if is_system && !show_system { continue; }
            if filter.is_empty() || name_lower.contains(&filter) {
                if pids.len() > 1 {
                    let expander = Expander::builder().expanded(false).build();
                    let header = GBox::new(Orientation::Horizontal, 8);
                    header.add_css_class("expander-header-content");
                    header.set_valign(Align::Center);
                    let icon = Image::builder().pixel_size(24).build();
                    let icon_name = find_icon_name(&name);
                    icon.set_from_icon_name(Some(&icon_name));
                    header.append(&icon);
                    header.append(&Label::builder()
                        .label(&name)
                        .halign(Align::Start)
                        .hexpand(true)
                        .valign(Align::Center)
                        .css_classes(["target-name"])
                        .build());
                    header.append(&Label::builder()
                        .label(&format!("{:.1}%", total_cpu))
                        .valign(Align::Center)
                        .css_classes(["process-cpu"])
                        .build());
                    let limit_all = Button::builder()
                        .label("Limit All")
                        .css_classes(["target-btn-small", "accent-button", "limit-all-btn"])
                        .valign(Align::Center)
                        .build();
                    let st_all = state_c.clone(); 
                    let name_all = name.clone();
                    let ml_all = main_list_c.clone(); 
                    let bg_all = badge_c.clone(); 
                    let dc_all = dialog_c.clone();
                    limit_all.connect_clicked(move |_| {
                        if let Ok(mut s) = st_all.lock() {
                            s.targets.retain(|t| t.name != name_all);
                            s.targets.push(TargetProcess { pid: 0, name: name_all.clone(), limit_percent: 33.0, is_active: true });
                            s.save();
                        }
                        refresh_targets(&ml_all, st_all.clone(), bg_all.clone()); 
                        dc_all.close();
                    });
                    header.append(&limit_all);
                    expander.set_label_widget(Some(&header));
                    let inner_list = ListBox::new();
                    inner_list.add_css_class("transparent-list");
                    for (pid, cpu, _) in pids {
                        inner_list.append(&create_process_row(pid, &name, cpu, state_c.clone(), main_list_c.clone(), badge_c.clone(), dialog_c.clone(), true));
                    }
                    expander.set_child(Some(&inner_list));
                    list_c.append(&expander);
                } else if let Some((pid, cpu, _)) = pids.first() {
                    list_c.append(&create_process_row(*pid, &name, *cpu, state_c.clone(), main_list_c.clone(), badge_c.clone(), dialog_c.clone(), false));
                }
            }
        }
    };
    refresh();
    let ref_c = refresh.clone();
    search.connect_search_changed(move |_| ref_c());
    let ref_t = refresh.clone();
    sys_toggle.connect_toggled(move |_| ref_t());
    let ref_u = refresh.clone();
    update_btn.connect_clicked(move |_| ref_u());
    let dialog_timer = dialog.clone();
    let auto_timer = auto_toggle.clone();
    glib::timeout_add_local(std::time::Duration::from_secs(2), move || {
        if !dialog_timer.is_visible() { return glib::ControlFlow::Break; }
        if auto_timer.is_active() {
            refresh();
        }
        glib::ControlFlow::Continue
    });
    dialog.present();
}
fn create_process_row(pid: u32, name: &str, cpu: f64, state: SharedState, main_list: ListBox, badge: Label, dialog: gtk::Window, is_sub_row: bool) -> ListBoxRow {
    let row = ListBoxRow::new();
    let hbox = GBox::new(Orientation::Horizontal, 10);
    hbox.set_valign(Align::Center);
    if is_sub_row {
        hbox.add_css_class("sub-process-row");
    } else {
        row.add_css_class("process-card");
        hbox.add_css_class("process-row-content");
    }
    let icon = Image::builder().pixel_size(24).build();
    let icon_name = find_icon_name(name);
    icon.set_from_icon_name(Some(&icon_name));
    hbox.append(&icon);
    let text_vbox = GBox::new(Orientation::Vertical, 0);
    text_vbox.set_valign(Align::Center);
    text_vbox.append(&Label::builder().label(name).halign(Align::Start).css_classes(["target-name"]).build());
    text_vbox.append(&Label::builder().label(&format!("PID: {}", pid)).halign(Align::Start).css_classes(["target-pid"]).build());
    hbox.append(&text_vbox);
    let filler = GBox::new(Orientation::Horizontal, 0); filler.set_hexpand(true); hbox.append(&filler);
    let cpu_label = Label::builder().label(&format!("{:.1}%", cpu)).css_classes(["process-cpu"]).build();
    cpu_label.set_valign(Align::Center);
    hbox.append(&cpu_label);
    let limit_btn = Button::builder()
        .label("Limit")
        .css_classes(["target-btn-small", "accent-button"])
        .valign(Align::Center)
        .build();
    let st_c = state.clone(); let ml_c = main_list.clone(); let bg_c = badge.clone(); let dc_c = dialog.clone();
    let pid_v = pid; let name_v = name.to_string();
    limit_btn.connect_clicked(move |_| {
        if name_v.to_lowercase().contains("lbs") {
            let alert = AlertDialog::builder()
                .message("Action Forbidden: Limiting LBS itself is not allowed to prevent system deadlocks.")
                .modal(true)
                .build();
            alert.choose(None::<&gtk::Window>, gtk::gio::Cancellable::NONE, |_| {});
        } else {
            backend::add_target(pid_v, &name_v, 33.0, st_c.clone());
            refresh_targets(&ml_c, st_c.clone(), bg_c.clone());
            dc_c.close();
        }
    });
    hbox.append(&limit_btn);
    row.set_child(Some(&hbox));
    row
}
