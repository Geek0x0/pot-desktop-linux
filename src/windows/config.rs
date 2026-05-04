use crate::config::AppConfig;
use gtk::prelude::*;
use relm4::prelude::*;
use std::sync::Arc;

use crate::i18n;
use crate::services::collection;
use crate::services::recognize;
use crate::services::translate;
use crate::services::tts;
use crate::windows::service_config::ServiceConfigModel;
use std::cell::RefCell;
use std::rc::Rc;

const SETTINGS_LABEL_WIDTH: i32 = 190;

pub struct ConfigModel {
    config: Arc<AppConfig>,
    #[allow(dead_code)]
    service_config_controller: Controller<ServiceConfigModel>,
}

#[derive(Debug)]
pub enum ConfigMsg {
    SetBool(String, bool),
    SetString(String, String),
    SetInt(String, i64),
}

#[derive(Debug)]
pub enum ConfigOutput {
    LanguageChanged(String),
}

#[relm4::component(pub)]
impl Component for ConfigModel {
    type Init = (
        Arc<AppConfig>,
        Arc<translate::TranslateRegistry>,
        Arc<recognize::RecognizeRegistry>,
        Arc<tts::TtsRegistry>,
        Arc<collection::CollectionRegistry>,
    );
    type Input = ConfigMsg;
    type Output = ConfigOutput;
    type CommandOutput = ();

    view! {
        gtk::Window {
            set_title: Some(&i18n::t("Pot - Settings")),
            set_default_width: 920,
            set_default_height: 640,
            set_hide_on_close: true,
            set_icon_name: Some("com.pot-app.pot-gtk"),

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                add_css_class: "settings-window",

                // Header
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_margin_start: 18,
                    set_margin_end: 18,
                    set_margin_top: 14,
                    set_margin_bottom: 10,
                    add_css_class: "settings-header",

                    gtk::Label {
                        set_label: &i18n::t("Settings"),
                        add_css_class: "title-2",
                        set_hexpand: true,
                        set_halign: gtk::Align::Start,
                    },

                    #[name = "search_entry"]
                    gtk::SearchEntry {
                        set_placeholder_text: Some(&i18n::t("Search settings")),
                        set_width_chars: 24,
                        add_css_class: "settings-search",
                    },
                },

                // Content
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 14,
                    set_margin_start: 18,
                    set_margin_end: 18,
                    set_margin_bottom: 18,
                    set_vexpand: true,

                    // Sidebar
                    #[name = "sidebar"]
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 4,
                        set_size_request: (170, -1),
                        add_css_class: "settings-sidebar",
                    },

                    // Page stack
                    #[name = "stack"]
                    gtk::Stack {
                        set_hexpand: true,
                        set_vexpand: true,
                        set_transition_type: gtk::StackTransitionType::Crossfade,
                        add_css_class: "settings-stack",
                    },
                },
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let (config, translate_reg, recognize_reg, tts_reg, collection_reg) = init;

        let service_config_controller = ServiceConfigModel::builder()
            .launch((
                config.clone(),
                translate_reg,
                recognize_reg,
                tts_reg,
                collection_reg,
            ))
            .detach();

        let model = ConfigModel {
            config,
            service_config_controller,
        };

        let widgets = view_output!();

        // Build pages and add to stack
        let general_page = build_general_page(&model.config, &sender);
        let translate_page = build_translate_page(&model.config, &sender);
        let hotkey_page = build_hotkey_page(&model.config, &sender);

        // Services page — embed the service config component
        let services_container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        services_container.set_margin_top(18);
        services_container.set_margin_bottom(18);
        services_container.set_margin_start(18);
        services_container.set_margin_end(18);
        let svc_widget = model.service_config_controller.widget();
        services_container.append(svc_widget);

        // About page
        let about_page = settings_page_box();
        let about_section = settings_section(&i18n::t("About"));
        let about_name = gtk::Label::new(Some(&i18n::t("Pot Desktop — GTK4 Edition")));
        about_name.add_css_class("title-2");
        about_name.set_halign(gtk::Align::Start);
        about_name.set_margin_top(4);
        about_name.set_margin_bottom(6);
        about_section.append(&about_name);
        append_readonly_row(
            &about_section,
            &i18n::t("Version"),
            env!("CARGO_PKG_VERSION"),
        );
        append_readonly_row(&about_section, &i18n::t("Author"), "Kody");
        append_readonly_row(
            &about_section,
            &i18n::t("Repository"),
            "https://github.com/Geek0x0/pot-desktop-linux",
        );
        about_page.append(&about_section);

        widgets
            .stack
            .add_titled(&general_page, Some("general"), &i18n::t("General"));
        widgets
            .stack
            .add_titled(&translate_page, Some("translate"), &i18n::t("Translate"));
        widgets
            .stack
            .add_titled(&services_container, Some("services"), &i18n::t("Services"));
        widgets
            .stack
            .add_titled(&hotkey_page, Some("hotkey"), &i18n::t("Hotkey"));
        widgets
            .stack
            .add_titled(&about_page, Some("about"), &i18n::t("About"));

        // Build sidebar buttons
        let pages = [
            ("general", i18n::t("General"), "preferences-system-symbolic"),
            (
                "translate",
                i18n::t("Translate"),
                "accessories-dictionary-symbolic",
            ),
            (
                "services",
                i18n::t("Services"),
                "applications-system-symbolic",
            ),
            ("hotkey", i18n::t("Hotkey"), "input-keyboard-symbolic"),
            ("about", i18n::t("About"), "help-about-symbolic"),
        ];
        let stack = widgets.stack.clone();
        for (i, (page, label, icon)) in pages.iter().enumerate() {
            let btn = nav_button(label, icon, i == 0);
            let stack_c = stack.clone();
            let page_s = page.to_string();
            btn.connect_clicked(move |_| {
                stack_c.set_visible_child_name(&page_s);
            });
            widgets.sidebar.append(&btn);
        }

        // Sync sidebar active state with stack
        let sidebar = widgets.sidebar.clone();
        widgets.stack.connect_visible_child_name_notify(move |st| {
            if let Some(name) = st.visible_child_name() {
                let name_str = name.to_string();
                let pages = ["general", "translate", "services", "hotkey", "about"];
                if let Some(idx) = pages.iter().position(|p| *p == name_str) {
                    let children = sidebar.observe_children();
                    for i in 0..children.n_items() {
                        if let Some(child) = children.item(i) {
                            if let Ok(btn) = child.downcast::<gtk::Button>() {
                                if i == idx as u32 {
                                    btn.add_css_class("settings-nav-active");
                                } else {
                                    btn.remove_css_class("settings-nav-active");
                                }
                            }
                        }
                    }
                }
            }
        });

        let search_stack = widgets.stack.clone();
        widgets.search_entry.connect_search_changed(move |entry| {
            filter_settings_tree(&search_stack, &entry.text());
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            ConfigMsg::SetBool(key, value) => {
                self.config.set(&key, serde_json::Value::Bool(value));
            }
            ConfigMsg::SetString(key, value) => {
                self.config
                    .set(&key, serde_json::Value::String(value.clone()));
                if key == "app_theme" {
                    i18n::apply_theme(&value);
                }
                if key == "app_language" {
                    i18n::set_language(&value);
                    let _ = sender.output(ConfigOutput::LanguageChanged(value));
                }
            }
            ConfigMsg::SetInt(key, value) => {
                self.config.set(&key, serde_json::json!(value));
            }
        }
    }
}

fn nav_button(label: &str, icon: &str, active: bool) -> gtk::Button {
    let btn = gtk::Button::new();
    btn.add_css_class("flat");
    btn.add_css_class("settings-nav-button");
    if active {
        btn.add_css_class("settings-nav-active");
    }

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    content.set_halign(gtk::Align::Fill);
    content.append(&gtk::Image::from_icon_name(icon));

    let text = gtk::Label::new(Some(label));
    text.set_xalign(0.0);
    text.set_hexpand(true);
    content.append(&text);

    btn.set_child(Some(&content));
    btn
}

fn settings_scrolled_window() -> gtk::ScrolledWindow {
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_hscrollbar_policy(gtk::PolicyType::Never);
    scrolled.add_css_class("settings-page-scroll");
    scrolled
}

fn settings_page_box() -> gtk::Box {
    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 16);
    vbox.set_margin_start(6);
    vbox.set_margin_end(6);
    vbox.set_margin_top(2);
    vbox.set_margin_bottom(2);
    vbox.add_css_class("settings-page");
    vbox
}

fn settings_section(title: &str) -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 0);
    section.add_css_class("settings-section");

    let title_label = gtk::Label::new(Some(title));
    title_label.add_css_class("settings-section-title");
    title_label.set_halign(gtk::Align::Start);
    title_label.set_margin_bottom(8);
    section.append(&title_label);
    section
}

fn append_setting_row<W: IsA<gtk::Widget>>(
    section: &gtk::Box,
    label: &str,
    widget: &W,
    expand_control: bool,
) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    row.add_css_class("settings-row");

    let lbl = gtk::Label::new(Some(label));
    lbl.set_size_request(SETTINGS_LABEL_WIDTH, -1);
    lbl.set_xalign(0.0);
    lbl.set_wrap(true);
    lbl.add_css_class("setting-label");
    row.append(&lbl);

    if expand_control {
        widget.set_hexpand(true);
        widget.set_halign(gtk::Align::Fill);
    } else {
        widget.set_halign(gtk::Align::End);
    }
    row.append(widget);
    section.append(&row);
}

fn append_toggle_setting_row(section: &gtk::Box, label: &str, switch: &gtk::Switch) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    row.add_css_class("settings-row");
    row.add_css_class("settings-toggle-row");

    let lbl = gtk::Label::new(Some(label));
    lbl.set_size_request(SETTINGS_LABEL_WIDTH, -1);
    lbl.set_xalign(0.0);
    lbl.set_wrap(true);
    lbl.set_hexpand(true);
    lbl.set_halign(gtk::Align::Fill);
    lbl.set_valign(gtk::Align::Center);
    lbl.add_css_class("setting-label");
    lbl.add_css_class("setting-toggle-label");
    row.append(&lbl);

    switch.set_valign(gtk::Align::Center);
    switch.set_halign(gtk::Align::End);
    row.append(switch);

    section.append(&row);
}

fn append_readonly_row(section: &gtk::Box, label: &str, value: &str) {
    let value_label = gtk::Label::new(Some(value));
    value_label.set_selectable(true);
    value_label.set_xalign(0.0);
    value_label.set_wrap(true);
    value_label.add_css_class("dim-label");
    append_setting_row(section, label, &value_label, true);
}

fn hotkey_status_widget(config: &Arc<AppConfig>) -> gtk::Box {
    #[cfg(not(feature = "hotkey"))]
    let _ = config;

    let card = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    card.add_css_class("settings-status-card");

    #[cfg(feature = "hotkey")]
    let (ok, session_type, backend, detail) = {
        let status = crate::core::hotkey::status(config);
        (
            status.ok,
            status.session_type,
            status.backend,
            status.detail,
        )
    };
    #[cfg(not(feature = "hotkey"))]
    let (ok, session_type, backend, detail) = (
        false,
        "disabled".to_string(),
        i18n::t("Hotkey feature disabled"),
        i18n::t("Build the app with the hotkey feature to enable global shortcuts."),
    );

    let icon_name = if ok {
        "emblem-ok-symbolic"
    } else {
        "dialog-warning-symbolic"
    };
    let icon = gtk::Image::from_icon_name(icon_name);
    icon.add_css_class(if ok { "status-ok" } else { "status-warning" });
    card.append(&icon);

    let text_box = gtk::Box::new(gtk::Orientation::Vertical, 3);
    text_box.set_hexpand(true);

    let title = gtk::Label::new(Some(&format!("{} · {}", session_type, backend)));
    title.set_halign(gtk::Align::Start);
    title.set_xalign(0.0);
    title.add_css_class("heading");
    text_box.append(&title);

    let detail = gtk::Label::new(Some(&detail));
    detail.set_halign(gtk::Align::Start);
    detail.set_xalign(0.0);
    detail.set_wrap(true);
    detail.add_css_class("dim-label");
    text_box.append(&detail);

    card.append(&text_box);
    card
}

fn filter_settings_tree(stack: &gtk::Stack, query: &str) {
    let query = query.trim().to_lowercase();
    let children = stack.observe_children();
    for i in 0..children.n_items() {
        if let Some(child) = children.item(i).and_downcast::<gtk::Widget>() {
            filter_settings_widget(&child, &query, query.is_empty());
        }
    }
}

fn filter_settings_widget(widget: &gtk::Widget, query: &str, reset: bool) -> bool {
    if reset {
        widget.set_visible(true);
        let children = widget.observe_children();
        for i in 0..children.n_items() {
            if let Some(child) = children.item(i).and_downcast::<gtk::Widget>() {
                filter_settings_widget(&child, query, true);
            }
        }
        return true;
    }

    let text_matches = widget_text(widget).to_lowercase().contains(query);
    let children = widget.observe_children();
    let mut child_matches = false;
    for i in 0..children.n_items() {
        if let Some(child) = children.item(i).and_downcast::<gtk::Widget>() {
            child_matches |= filter_settings_widget(&child, query, false);
        }
    }

    let is_filter_target = widget.has_css_class("settings-row")
        || widget.has_css_class("settings-section")
        || widget.has_css_class("service-card")
        || widget.has_css_class("service-field-row");
    let visible = text_matches || child_matches;
    if is_filter_target {
        widget.set_visible(visible);
    }

    visible
}

fn widget_text(widget: &gtk::Widget) -> String {
    let mut parts = Vec::new();

    if let Ok(label) = widget.clone().downcast::<gtk::Label>() {
        parts.push(label.text().to_string());
    }
    if let Ok(button) = widget.clone().downcast::<gtk::Button>() {
        if let Some(label) = button.label() {
            parts.push(label.to_string());
        }
        if let Some(tooltip) = button.tooltip_text() {
            parts.push(tooltip.to_string());
        }
    }
    if let Ok(entry) = widget.clone().downcast::<gtk::Entry>() {
        parts.push(entry.text().to_string());
        if let Some(placeholder) = entry.placeholder_text() {
            parts.push(placeholder.to_string());
        }
    }

    let children = widget.observe_children();
    for i in 0..children.n_items() {
        if let Some(child) = children.item(i).and_downcast::<gtk::Widget>() {
            parts.push(widget_text(&child));
        }
    }

    parts.join(" ")
}

fn build_general_page(
    config: &Arc<AppConfig>,
    sender: &ComponentSender<ConfigModel>,
) -> gtk::ScrolledWindow {
    let scrolled = settings_scrolled_window();
    let vbox = settings_page_box();
    let general_section = settings_section(&i18n::t("General"));
    let proxy_section = settings_section(&i18n::t("Proxy"));

    // Server Port
    let spin = gtk::SpinButton::with_range(0.0, 65535.0, 1.0);
    spin.set_width_chars(8);
    spin.set_halign(gtk::Align::End);
    let port_val = config
        .get("server_port")
        .and_then(|v| v.as_i64())
        .unwrap_or(60828);
    spin.set_value(port_val as f64);
    let sender_c = sender.input_sender().clone();
    spin.connect_value_notify(move |s| {
        let _ = sender_c.send(ConfigMsg::SetInt("server_port".into(), s.value() as i64));
    });
    append_setting_row(&general_section, &i18n::t("Server Port:"), &spin, false);

    // Language
    let lang_codes = ["en", "zh_cn", "zh_tw", "ja", "ko", "fr", "de", "es", "ru"];
    let lang_labels = [
        "English",
        "简体中文",
        "繁體中文",
        "日本語",
        "한국어",
        "Français",
        "Deutsch",
        "Español",
        "Русский",
    ];
    let lang_list = gtk::StringList::new(&lang_labels);
    let lang_combo = gtk::DropDown::new(
        Some(lang_list.upcast::<gtk::gio::ListModel>()),
        gtk::Expression::NONE,
    );
    let current_lang = config
        .get("app_language")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "en".into());
    if let Some(idx) = lang_codes.iter().position(|c| *c == current_lang) {
        lang_combo.set_selected(idx as u32);
    }
    let sender_c = sender.input_sender().clone();
    lang_combo.connect_selected_notify(move |dd| {
        let idx = dd.selected() as usize;
        if idx < lang_codes.len() {
            let _ = sender_c.send(ConfigMsg::SetString(
                "app_language".into(),
                lang_codes[idx].to_string(),
            ));
        }
    });
    append_setting_row(&general_section, &i18n::t("Language:"), &lang_combo, true);

    // Theme
    let theme_values = ["system", "light", "dark"];
    let theme_labels_raw = ["System", "Light", "Dark"];
    let theme_labels: Vec<String> = theme_labels_raw.iter().map(|l| i18n::t(l)).collect();
    let theme_str_refs: Vec<&str> = theme_labels.iter().map(|s| s.as_str()).collect();
    let theme_list = gtk::StringList::new(&theme_str_refs);
    let theme_combo = gtk::DropDown::new(
        Some(theme_list.upcast::<gtk::gio::ListModel>()),
        gtk::Expression::NONE,
    );
    let current_theme = config
        .get("app_theme")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "system".into());
    if let Some(idx) = theme_values.iter().position(|c| *c == current_theme) {
        theme_combo.set_selected(idx as u32);
    }
    let sender_c = sender.input_sender().clone();
    theme_combo.connect_selected_notify(move |dd| {
        let idx = dd.selected() as usize;
        if idx < theme_values.len() {
            let _ = sender_c.send(ConfigMsg::SetString(
                "app_theme".into(),
                theme_values[idx].to_string(),
            ));
        }
    });
    append_setting_row(&general_section, &i18n::t("Theme:"), &theme_combo, true);

    // Autostart
    let autostart_sw = gtk::Switch::new();
    autostart_sw.set_halign(gtk::Align::End);
    autostart_sw.set_active(
        config
            .get("autostart_enabled")
            .map(|v| v.as_bool().unwrap_or(false))
            .unwrap_or_else(crate::core::autostart::is_enabled),
    );
    let sender_c = sender.input_sender().clone();
    autostart_sw.connect_state_set(move |_sw, state| {
        if let Err(e) = crate::core::autostart::set_enabled(state) {
            log::warn!("Failed to set autostart: {}", e);
        }
        let _ = sender_c.send(ConfigMsg::SetBool("autostart_enabled".into(), state));
        gtk::glib::Propagation::Proceed
    });
    append_toggle_setting_row(&general_section, &i18n::t("Auto Start:"), &autostart_sw);

    vbox.append(&general_section);

    // Proxy enable
    let proxy_sw = gtk::Switch::new();
    proxy_sw.set_halign(gtk::Align::End);
    proxy_sw.set_active(
        config
            .get("proxy_enable")
            .map(|v| v.as_bool().unwrap_or(false))
            .unwrap_or(false),
    );
    let sender_c = sender.input_sender().clone();
    proxy_sw.connect_state_set(move |_sw, state| {
        let _ = sender_c.send(ConfigMsg::SetBool("proxy_enable".into(), state));
        gtk::glib::Propagation::Proceed
    });
    append_toggle_setting_row(&proxy_section, &i18n::t("Enable Proxy:"), &proxy_sw);

    // Proxy host
    let host_entry = gtk::Entry::new();
    host_entry.set_placeholder_text(Some("http://127.0.0.1"));
    if let Some(v) = config.get("proxy_host") {
        if let Some(host) = v.as_str() {
            host_entry.set_text(host);
        }
    }
    let sender_c = sender.input_sender().clone();
    host_entry.connect_changed(move |e| {
        let _ = sender_c.send(ConfigMsg::SetString(
            "proxy_host".into(),
            e.text().to_string(),
        ));
    });
    append_setting_row(&proxy_section, &i18n::t("Proxy Host:"), &host_entry, true);

    // Proxy port
    let port_entry = gtk::Entry::new();
    port_entry.set_placeholder_text(Some("7890"));
    if let Some(port) = config.get("proxy_port") {
        let text = match &port {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            _ => String::new(),
        };
        port_entry.set_text(&text);
    }
    let sender_c = sender.input_sender().clone();
    port_entry.connect_changed(move |e| {
        let _ = sender_c.send(ConfigMsg::SetString(
            "proxy_port".into(),
            e.text().to_string(),
        ));
    });
    append_setting_row(&proxy_section, &i18n::t("Proxy Port:"), &port_entry, true);

    // Proxy no_proxy
    let noproxy_entry = gtk::Entry::new();
    noproxy_entry.set_placeholder_text(Some("localhost,127.0.0.1"));
    if let Some(v) = config.get("no_proxy") {
        if let Some(s) = v.as_str() {
            noproxy_entry.set_text(s);
        }
    }
    let sender_c = sender.input_sender().clone();
    noproxy_entry.connect_changed(move |e| {
        let _ = sender_c.send(ConfigMsg::SetString(
            "no_proxy".into(),
            e.text().to_string(),
        ));
    });
    append_setting_row(&proxy_section, &i18n::t("No Proxy:"), &noproxy_entry, true);

    vbox.append(&proxy_section);
    scrolled.set_child(Some(&vbox));
    scrolled
}

fn build_translate_page(
    config: &Arc<AppConfig>,
    sender: &ComponentSender<ConfigModel>,
) -> gtk::ScrolledWindow {
    let scrolled = settings_scrolled_window();
    let vbox = settings_page_box();
    let section = settings_section(&i18n::t("Translate"));

    // Source language
    let languages = crate::lang::Language::all();
    let mut src_codes: Vec<String> = vec!["auto".into()];
    let mut src_labels: Vec<String> = vec![i18n::t("Auto Detect")];
    for lang in languages.iter() {
        src_codes.push(lang.code().to_string());
        src_labels.push(lang.display_name().to_string());
    }
    let src_str_refs: Vec<&str> = src_labels.iter().map(|s| s.as_str()).collect();
    let src_list = gtk::StringList::new(&src_str_refs);
    let src_combo = gtk::DropDown::new(
        Some(src_list.upcast::<gtk::gio::ListModel>()),
        gtk::Expression::NONE,
    );
    let src_lang = config
        .get("translate_source_language")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "auto".into());
    if let Some(idx) = src_codes.iter().position(|c| *c == src_lang) {
        src_combo.set_selected(idx as u32);
    }
    let sender_c = sender.input_sender().clone();
    src_combo.connect_selected_notify(move |dd| {
        let idx = dd.selected() as usize;
        if idx < src_codes.len() {
            let _ = sender_c.send(ConfigMsg::SetString(
                "translate_source_language".into(),
                src_codes[idx].clone(),
            ));
        }
    });
    append_setting_row(&section, &i18n::t("Source Language:"), &src_combo, true);

    // Target language
    let mut tgt_codes: Vec<String> = Vec::new();
    let mut tgt_labels: Vec<String> = Vec::new();
    for lang in languages.iter() {
        tgt_codes.push(lang.code().to_string());
        tgt_labels.push(lang.display_name().to_string());
    }
    let tgt_str_refs: Vec<&str> = tgt_labels.iter().map(|s| s.as_str()).collect();
    let tgt_list = gtk::StringList::new(&tgt_str_refs);
    let tgt_combo = gtk::DropDown::new(
        Some(tgt_list.upcast::<gtk::gio::ListModel>()),
        gtk::Expression::NONE,
    );
    let tgt_lang = config
        .get("translate_target_language")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "zh_cn".into());
    if let Some(idx) = tgt_codes.iter().position(|c| *c == tgt_lang) {
        tgt_combo.set_selected(idx as u32);
    }
    let sender_c = sender.input_sender().clone();
    tgt_combo.connect_selected_notify(move |dd| {
        let idx = dd.selected() as usize;
        if idx < tgt_codes.len() {
            let _ = sender_c.send(ConfigMsg::SetString(
                "translate_target_language".into(),
                tgt_codes[idx].clone(),
            ));
        }
    });
    append_setting_row(&section, &i18n::t("Target Language:"), &tgt_combo, true);

    // Delete newlines
    let sw = gtk::Switch::new();
    sw.set_active(
        config
            .get("translate_delete_newline")
            .map(|v| v.as_bool().unwrap_or(false))
            .unwrap_or(false),
    );
    let sender_c = sender.input_sender().clone();
    sw.connect_state_set(move |_sw, state| {
        let _ = sender_c.send(ConfigMsg::SetBool("translate_delete_newline".into(), state));
        gtk::glib::Propagation::Proceed
    });
    append_toggle_setting_row(&section, &i18n::t("Delete Newlines:"), &sw);

    // Close on blur
    let sw = gtk::Switch::new();
    sw.set_active(
        config
            .get("translate_close_on_blur")
            .map(|v| v.as_bool().unwrap_or(true))
            .unwrap_or(true),
    );
    let sender_c = sender.input_sender().clone();
    sw.connect_state_set(move |_sw, state| {
        let _ = sender_c.send(ConfigMsg::SetBool("translate_close_on_blur".into(), state));
        gtk::glib::Propagation::Proceed
    });
    append_toggle_setting_row(&section, &i18n::t("Close on Blur:"), &sw);

    vbox.append(&section);
    scrolled.set_child(Some(&vbox));
    scrolled
}

fn build_hotkey_page(
    config: &Arc<AppConfig>,
    sender: &ComponentSender<ConfigModel>,
) -> gtk::ScrolledWindow {
    let scrolled = settings_scrolled_window();
    let vbox = settings_page_box();
    let status_section = settings_section(&i18n::t("Hotkey Status"));
    status_section.append(&hotkey_status_widget(config));
    vbox.append(&status_section);

    let section = settings_section(&i18n::t("Hotkeys"));

    let hint = gtk::Label::new(Some(&i18n::t(
        "Click a shortcut, press the new keys, then restart the app to apply the change. Press Esc to cancel.",
    )));
    hint.add_css_class("dim-label");
    hint.set_halign(gtk::Align::Start);
    hint.set_wrap(true);
    hint.set_margin_bottom(6);
    section.append(&hint);

    let hotkeys: [(&str, &str, &str); 4] = [
        (
            "hotkey_selection_translate",
            "Selection Translate:",
            "Ctrl+Shift+S",
        ),
        ("hotkey_input_translate", "Input Translate:", "Ctrl+Shift+I"),
        ("hotkey_ocr_recognize", "OCR Recognize:", "Ctrl+Shift+O"),
        ("hotkey_ocr_translate", "OCR Translate:", "Ctrl+Shift+T"),
    ];

    for (key, label, default_shortcut) in &hotkeys {
        let default_shortcut =
            normalize_shortcut_text(default_shortcut).unwrap_or_else(|| (*default_shortcut).into());
        let initial_shortcut = config
            .get(key)
            .and_then(|v| v.as_str().map(String::from))
            .filter(|value| !value.trim().is_empty())
            .and_then(|value| normalize_shortcut_text(&value))
            .unwrap_or_else(|| default_shortcut.clone());
        let current_shortcut = Rc::new(RefCell::new(initial_shortcut.clone()));

        let capture_button = gtk::ToggleButton::with_label(&initial_shortcut);
        capture_button.set_hexpand(true);
        capture_button.set_halign(gtk::Align::Fill);
        capture_button.add_css_class("shortcut-capture-button");
        capture_button.connect_toggled({
            let current_shortcut = current_shortcut.clone();
            move |button| {
                if button.is_active() {
                    button.set_label("...");
                    button.grab_focus();
                } else {
                    let label = current_shortcut.borrow().clone();
                    button.set_label(&label);
                }
            }
        });

        let key_controller = gtk::EventControllerKey::new();
        key_controller.connect_key_pressed({
            let capture_button = capture_button.clone();
            let current_shortcut = current_shortcut.clone();
            let sender_c = sender.input_sender().clone();
            let key_s = key.to_string();
            move |_controller, keyval, _keycode, state| {
                if !capture_button.is_active() {
                    return gtk::glib::Propagation::Proceed;
                }

                if keyval == gtk::gdk::Key::Escape {
                    capture_button.set_active(false);
                    return gtk::glib::Propagation::Stop;
                }

                if let Some(shortcut) = shortcut_from_key_event(keyval, state) {
                    *current_shortcut.borrow_mut() = shortcut.clone();
                    let _ = sender_c.send(ConfigMsg::SetString(key_s.clone(), shortcut));
                    capture_button.set_active(false);
                }

                gtk::glib::Propagation::Stop
            }
        });
        capture_button.add_controller(key_controller);

        let reset_button = gtk::Button::from_icon_name("edit-undo-symbolic");
        reset_button.add_css_class("flat");
        reset_button.set_tooltip_text(Some(&i18n::t("Reset")));
        reset_button.connect_clicked({
            let capture_button = capture_button.clone();
            let current_shortcut = current_shortcut.clone();
            let sender_c = sender.input_sender().clone();
            let key_s = key.to_string();
            let default_shortcut = default_shortcut.clone();
            move |_| {
                *current_shortcut.borrow_mut() = default_shortcut.clone();
                let _ = sender_c.send(ConfigMsg::SetString(
                    key_s.clone(),
                    default_shortcut.clone(),
                ));
                capture_button.set_active(false);
                capture_button.set_label(&default_shortcut);
            }
        });

        let control = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        control.set_hexpand(true);
        control.append(&capture_button);
        control.append(&reset_button);
        append_setting_row(&section, &i18n::t(label), &control, true);
    }

    vbox.append(&section);
    scrolled.set_child(Some(&vbox));
    scrolled
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct ShortcutModifiers {
    ctrl: bool,
    alt: bool,
    shift: bool,
    super_key: bool,
}

fn normalize_shortcut_text(shortcut: &str) -> Option<String> {
    let parts: Vec<&str> = shortcut
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    let key = parts.last()?;

    let mut modifiers = ShortcutModifiers::default();
    for part in &parts[..parts.len().saturating_sub(1)] {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" | "cmdorctrl" | "commandorcontrol" => modifiers.ctrl = true,
            "alt" | "option" => modifiers.alt = true,
            "shift" => modifiers.shift = true,
            "super" | "win" | "meta" => modifiers.super_key = true,
            _ => return None,
        }
    }

    format_shortcut(modifiers, key)
}

fn shortcut_from_key_event(keyval: gtk::gdk::Key, state: gtk::gdk::ModifierType) -> Option<String> {
    let modifiers = ShortcutModifiers {
        ctrl: state.contains(gtk::gdk::ModifierType::CONTROL_MASK),
        alt: state.contains(gtk::gdk::ModifierType::ALT_MASK),
        shift: state.contains(gtk::gdk::ModifierType::SHIFT_MASK),
        super_key: state.contains(gtk::gdk::ModifierType::SUPER_MASK)
            || state.contains(gtk::gdk::ModifierType::META_MASK),
    };

    let key = keyval_to_shortcut_key(keyval)?;
    format_shortcut(modifiers, &key)
}

fn format_shortcut(modifiers: ShortcutModifiers, key: &str) -> Option<String> {
    let key = normalize_shortcut_key(key)?;

    let mut parts = Vec::new();
    if modifiers.ctrl {
        parts.push("Ctrl".to_string());
    }
    if modifiers.shift {
        parts.push("Shift".to_string());
    }
    if modifiers.alt {
        parts.push("Alt".to_string());
    }
    if modifiers.super_key {
        parts.push("Super".to_string());
    }
    parts.push(key);

    Some(parts.join("+"))
}

fn normalize_shortcut_key(key: &str) -> Option<String> {
    let normalized = key.trim().to_lowercase();

    match normalized.as_str() {
        "esc" | "escape" => Some("Escape".into()),
        "return" | "enter" => Some("Enter".into()),
        "space" => Some("Space".into()),
        "tab" => Some("Tab".into()),
        "backspace" => Some("Backspace".into()),
        "delete" => Some("Delete".into()),
        "insert" => Some("Insert".into()),
        "home" => Some("Home".into()),
        "end" => Some("End".into()),
        "pageup" | "page_up" | "page up" => Some("PageUp".into()),
        "pagedown" | "page_down" | "page down" => Some("PageDown".into()),
        "left" => Some("Left".into()),
        "right" => Some("Right".into()),
        "up" => Some("Up".into()),
        "down" => Some("Down".into()),
        other if other.len() == 1 && other.chars().all(|ch| ch.is_ascii_alphanumeric()) => {
            Some(other.to_ascii_uppercase())
        }
        other if other.starts_with('f') => {
            let number = other.strip_prefix('f')?.parse::<u8>().ok()?;
            if (1..=12).contains(&number) {
                Some(format!("F{}", number))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn keyval_to_shortcut_key(keyval: gtk::gdk::Key) -> Option<String> {
    if let Some(ch) = keyval.to_unicode() {
        if ch == ' ' {
            return Some("Space".into());
        }
        if ch.is_ascii_alphanumeric() {
            return Some(ch.to_ascii_uppercase().to_string());
        }
    }

    match keyval {
        gtk::gdk::Key::Return => Some("Enter".into()),
        gtk::gdk::Key::Tab => Some("Tab".into()),
        gtk::gdk::Key::BackSpace => Some("Backspace".into()),
        gtk::gdk::Key::Delete => Some("Delete".into()),
        gtk::gdk::Key::Insert => Some("Insert".into()),
        gtk::gdk::Key::Home => Some("Home".into()),
        gtk::gdk::Key::End => Some("End".into()),
        gtk::gdk::Key::Page_Up => Some("PageUp".into()),
        gtk::gdk::Key::Page_Down => Some("PageDown".into()),
        gtk::gdk::Key::Left => Some("Left".into()),
        gtk::gdk::Key::Right => Some("Right".into()),
        gtk::gdk::Key::Up => Some("Up".into()),
        gtk::gdk::Key::Down => Some("Down".into()),
        gtk::gdk::Key::F1 => Some("F1".into()),
        gtk::gdk::Key::F2 => Some("F2".into()),
        gtk::gdk::Key::F3 => Some("F3".into()),
        gtk::gdk::Key::F4 => Some("F4".into()),
        gtk::gdk::Key::F5 => Some("F5".into()),
        gtk::gdk::Key::F6 => Some("F6".into()),
        gtk::gdk::Key::F7 => Some("F7".into()),
        gtk::gdk::Key::F8 => Some("F8".into()),
        gtk::gdk::Key::F9 => Some("F9".into()),
        gtk::gdk::Key::F10 => Some("F10".into()),
        gtk::gdk::Key::F11 => Some("F11".into()),
        gtk::gdk::Key::F12 => Some("F12".into()),
        gtk::gdk::Key::Shift_L
        | gtk::gdk::Key::Shift_R
        | gtk::gdk::Key::Control_L
        | gtk::gdk::Key::Control_R
        | gtk::gdk::Key::Alt_L
        | gtk::gdk::Key::Alt_R
        | gtk::gdk::Key::Meta_L
        | gtk::gdk::Key::Meta_R
        | gtk::gdk::Key::Super_L
        | gtk::gdk::Key::Super_R => None,
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_shortcut_text_aliases() {
        assert_eq!(
            normalize_shortcut_text("control + option + page_down"),
            Some("Ctrl+Alt+PageDown".into())
        );
    }

    #[test]
    fn rejects_modifier_only_shortcuts() {
        assert_eq!(normalize_shortcut_text("Ctrl+Shift"), None);
    }

    #[test]
    fn formats_key_events_to_backend_shortcut_strings() {
        assert_eq!(
            shortcut_from_key_event(
                gtk::gdk::Key::F8,
                gtk::gdk::ModifierType::CONTROL_MASK | gtk::gdk::ModifierType::SHIFT_MASK,
            ),
            Some("Ctrl+Shift+F8".into())
        );
    }
}
