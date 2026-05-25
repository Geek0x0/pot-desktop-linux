use crate::config::{AppConfig, ServiceCategory, DEFAULT_TRANSLATE_SERVICES};
use crate::i18n;
use gtk::prelude::*;
use relm4::prelude::*;
use std::collections::BTreeSet;
use std::sync::Arc;

pub struct ServiceConfigModel {
    config: Arc<AppConfig>,
    category: ServiceCategory,
    registries: ServiceRegistries,
    instance_keys: Vec<String>,
    list_box: std::cell::RefCell<gtk::ListBox>,
}

pub struct ServiceRegistries {
    translate: Arc<crate::services::translate::TranslateRegistry>,
    recognize: Arc<crate::services::recognize::RecognizeRegistry>,
    tts: Arc<crate::services::tts::TtsRegistry>,
    collection: Arc<crate::services::collection::CollectionRegistry>,
}

impl ServiceRegistries {
    fn list(&self, category: ServiceCategory) -> Vec<&str> {
        match category {
            ServiceCategory::Translate => self.translate.list(),
            ServiceCategory::Recognize => self.recognize.list(),
            ServiceCategory::Tts => self.tts.list(),
            ServiceCategory::Collection => self.collection.list(),
        }
    }

    fn default_config(&self, category: ServiceCategory, name: &str) -> serde_json::Value {
        match category {
            ServiceCategory::Translate => self
                .translate
                .get(name)
                .map(|s| s.default_config())
                .unwrap_or(serde_json::Value::Null),
            ServiceCategory::Recognize => self
                .recognize
                .get(name)
                .map(|s| s.default_config())
                .unwrap_or(serde_json::Value::Null),
            ServiceCategory::Tts => self
                .tts
                .get(name)
                .map(|s| s.default_config())
                .unwrap_or(serde_json::Value::Null),
            ServiceCategory::Collection => self
                .collection
                .get(name)
                .map(|s| s.default_config())
                .unwrap_or(serde_json::Value::Null),
        }
    }

    pub fn clone_registries(&self) -> Self {
        Self {
            translate: self.translate.clone(),
            recognize: self.recognize.clone(),
            tts: self.tts.clone(),
            collection: self.collection.clone(),
        }
    }
}

fn instance_service_name(instance_key: &str) -> &str {
    instance_key.split('@').next().unwrap_or(instance_key)
}

fn is_locked_default_service(category: ServiceCategory, service_name: &str) -> bool {
    matches!(category, ServiceCategory::Translate)
        && DEFAULT_TRANSLATE_SERVICES.contains(&service_name)
}

fn is_locked_default_instance(category: ServiceCategory, instance_key: &str) -> bool {
    is_locked_default_service(category, instance_service_name(instance_key))
}

fn addable_services(category: ServiceCategory, available: &[String]) -> Vec<String> {
    available
        .iter()
        .filter(|service_name| !is_locked_default_service(category, service_name.as_str()))
        .cloned()
        .collect()
}

fn service_page_keys(config: &AppConfig, category: ServiceCategory) -> Vec<String> {
    config.effective_service_list(category)
}

fn display_instance_key(instance_key: &str) -> String {
    match instance_key.split_once('@') {
        Some(("openai", suffix)) => format!("ai@{}", suffix),
        Some((service_name, suffix)) => format!("{}@{}", service_name, suffix),
        None if instance_key == "openai" => "ai".to_string(),
        None => instance_key.to_string(),
    }
}

fn service_display_name(service_name: &str) -> &str {
    match service_name {
        "openai" => "AI",
        "lingva_tts" => "Lingva TTS",
        other => other,
    }
}

#[derive(Debug)]
pub enum ServiceConfigMsg {
    SetCategory(ServiceCategory),
    AddInstance,
    DeleteInstance(String),
    ToggleEnable(String, bool),
    MoveUp(String),
    MoveDown(String),
    EditInstance(String),
    #[allow(dead_code)]
    ReloadList,
}

#[relm4::component(pub)]
impl Component for ServiceConfigModel {
    type Init = (
        Arc<AppConfig>,
        Arc<crate::services::translate::TranslateRegistry>,
        Arc<crate::services::recognize::RecognizeRegistry>,
        Arc<crate::services::tts::TtsRegistry>,
        Arc<crate::services::collection::CollectionRegistry>,
    );
    type Input = ServiceConfigMsg;
    type Output = ();
    type CommandOutput = ();

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 14,
            add_css_class: "service-config-page",

            // Category tabs
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 6,
                add_css_class: "service-tabs",

                #[name = "btn_translate"]
                gtk::ToggleButton {
                    set_label: &i18n::t("Translate"),
                    set_active: true,
                    add_css_class: "service-tab",
                    connect_clicked => ServiceConfigMsg::SetCategory(ServiceCategory::Translate),
                },
                #[name = "btn_recognize"]
                gtk::ToggleButton {
                    set_label: &i18n::t("Recognize"),
                    set_group: Some(&btn_translate),
                    add_css_class: "service-tab",
                    connect_clicked => ServiceConfigMsg::SetCategory(ServiceCategory::Recognize),
                },
                #[name = "btn_tts"]
                gtk::ToggleButton {
                    set_label: &i18n::t("TTS"),
                    set_group: Some(&btn_translate),
                    add_css_class: "service-tab",
                    connect_clicked => ServiceConfigMsg::SetCategory(ServiceCategory::Tts),
                },
                #[name = "btn_collection"]
                gtk::ToggleButton {
                    set_label: &i18n::t("Collection"),
                    set_group: Some(&btn_translate),
                    add_css_class: "service-tab",
                    connect_clicked => ServiceConfigMsg::SetCategory(ServiceCategory::Collection),
                },
            },

            // Instance list
            gtk::ScrolledWindow {
                set_vexpand: true,
                set_min_content_height: 200,
                add_css_class: "service-list-scroll",

                #[name = "list_box"]
                gtk::ListBox {
                    set_selection_mode: gtk::SelectionMode::None,
                    add_css_class: "service-list",
                },
            },

            // Action bar
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 8,
                set_margin_top: 4,

                gtk::Button {
                    set_label: &i18n::t("Add Service"),
                    add_css_class: "suggested-action",
                    add_css_class: "service-add-button",
                    connect_clicked => ServiceConfigMsg::AddInstance,
                },
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let (config, translate, recognize, tts, collection) = init;
        let registries = ServiceRegistries {
            translate,
            recognize,
            tts,
            collection,
        };

        let instance_keys = service_page_keys(&config, ServiceCategory::Translate);

        let model = ServiceConfigModel {
            config,
            category: ServiceCategory::Translate,
            registries,
            instance_keys,
            list_box: std::cell::RefCell::new(gtk::ListBox::new()),
        };

        let widgets = view_output!();

        // Store the actual list_box widget
        *model.list_box.borrow_mut() = widgets.list_box.clone();

        // Build initial list
        rebuild_list(
            &widgets.list_box,
            &model.config,
            model.category,
            &model.instance_keys,
            &sender,
        );

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
        let needs_rebuild = match msg {
            ServiceConfigMsg::SetCategory(cat) => {
                self.category = cat;
                self.instance_keys = service_page_keys(&self.config, cat);
                true
            }
            ServiceConfigMsg::ReloadList => {
                self.instance_keys = service_page_keys(&self.config, self.category);
                true
            }
            ServiceConfigMsg::AddInstance => {
                let available: Vec<String> = self
                    .registries
                    .list(self.category)
                    .into_iter()
                    .map(String::from)
                    .collect();
                let available = addable_services(self.category, &available);
                if !available.is_empty() {
                    show_add_dialog(
                        &self.config,
                        &self.registries,
                        self.category,
                        &available,
                        &sender,
                    );
                }
                false
            }
            ServiceConfigMsg::DeleteInstance(key) => {
                if is_locked_default_instance(self.category, &key) {
                    return;
                }
                let mut list = self.config.get_service_list(self.category);
                list.retain(|k| k != &key);
                self.config.set_service_list(self.category, &list);
                self.config.remove(&key);
                self.instance_keys = service_page_keys(&self.config, self.category);
                true
            }
            ServiceConfigMsg::ToggleEnable(key, enabled) => {
                let mut cfg = self
                    .config
                    .get(&key)
                    .and_then(|v| v.as_object().cloned())
                    .unwrap_or_default();
                cfg.insert("enable".into(), serde_json::Value::Bool(enabled));
                self.config.set(&key, cfg);
                false
            }
            ServiceConfigMsg::MoveUp(key) => {
                let mut list = self.config.get_service_list(self.category);
                if let Some(pos) = list.iter().position(|k| k == &key) {
                    if pos > 0 {
                        list.swap(pos - 1, pos);
                        self.config.set_service_list(self.category, &list);
                        self.instance_keys = service_page_keys(&self.config, self.category);
                    }
                }
                true
            }
            ServiceConfigMsg::MoveDown(key) => {
                let mut list = self.config.get_service_list(self.category);
                if let Some(pos) = list.iter().position(|k| k == &key) {
                    if pos + 1 < list.len() {
                        list.swap(pos, pos + 1);
                        self.config.set_service_list(self.category, &list);
                        self.instance_keys = service_page_keys(&self.config, self.category);
                    }
                }
                true
            }
            ServiceConfigMsg::EditInstance(key) => {
                if is_locked_default_instance(self.category, &key) {
                    return;
                }
                let service_name = key.split('@').next().unwrap_or(&key);
                show_edit_dialog(
                    &self.config,
                    &self.registries,
                    self.category,
                    &key,
                    service_name,
                );
                false
            }
        };

        if needs_rebuild {
            let lb = self.list_box.borrow().clone();
            rebuild_list(
                &lb,
                &self.config,
                self.category,
                &self.instance_keys,
                &sender,
            );
        }
    }
}

fn rebuild_list(
    list_box: &gtk::ListBox,
    config: &Arc<AppConfig>,
    category: ServiceCategory,
    keys: &[String],
    sender: &ComponentSender<ServiceConfigModel>,
) {
    while let Some(child) = list_box.last_child() {
        list_box.remove(&child);
    }

    for key in keys {
        let service_name = instance_service_name(key);
        let is_locked = is_locked_default_instance(category, key);
        let instance_cfg = config.get(key);
        let display_name = instance_cfg
            .as_ref()
            .and_then(|v| v.get("instanceName"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(service_name)
            .to_string();
        let enabled = instance_cfg
            .as_ref()
            .and_then(|v| v.get("enable"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row_box.add_css_class("service-card");

        let text_box = gtk::Box::new(gtk::Orientation::Vertical, 3);
        text_box.set_hexpand(true);

        let label = gtk::Label::new(Some(&display_name));
        label.set_halign(gtk::Align::Start);
        label.set_xalign(0.0);
        label.add_css_class("service-card-title");
        text_box.append(&label);

        let subtitle = gtk::Label::new(Some(&format!(
            "{} · {}",
            service_display_name(service_name),
            display_instance_key(key)
        )));
        subtitle.set_halign(gtk::Align::Start);
        subtitle.set_xalign(0.0);
        subtitle.add_css_class("caption");
        subtitle.add_css_class("dim-label");
        subtitle.set_ellipsize(gtk::pango::EllipsizeMode::End);
        text_box.append(&subtitle);
        row_box.append(&text_box);

        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        controls.add_css_class("service-card-controls");
        controls.set_valign(gtk::Align::Center);

        let switch = gtk::Switch::new();
        switch.set_active(enabled);
        switch.set_tooltip_text(Some(&i18n::t("Enable")));
        switch.set_valign(gtk::Align::Center);
        switch.set_halign(gtk::Align::End);
        let key_c = key.to_string();
        let sender_c = sender.input_sender().clone();
        switch.connect_state_set(move |_sw, state| {
            let _ = sender_c.send(ServiceConfigMsg::ToggleEnable(key_c.clone(), state));
            gtk::glib::Propagation::Proceed
        });
        controls.append(&switch);

        let action_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        action_box.add_css_class("service-card-actions");
        action_box.set_valign(gtk::Align::Center);

        let btn_up = gtk::Button::from_icon_name("go-up-symbolic");
        btn_up.add_css_class("flat");
        btn_up.set_tooltip_text(Some(&i18n::t("Move Up")));
        let key_c = key.to_string();
        let sender_c = sender.input_sender().clone();
        btn_up.connect_clicked(move |_| {
            let _ = sender_c.send(ServiceConfigMsg::MoveUp(key_c.clone()));
        });
        action_box.append(&btn_up);

        let btn_down = gtk::Button::from_icon_name("go-down-symbolic");
        btn_down.add_css_class("flat");
        btn_down.set_tooltip_text(Some(&i18n::t("Move Down")));
        let key_c = key.to_string();
        let sender_c = sender.input_sender().clone();
        btn_down.connect_clicked(move |_| {
            let _ = sender_c.send(ServiceConfigMsg::MoveDown(key_c.clone()));
        });
        action_box.append(&btn_down);

        let btn_edit = gtk::Button::from_icon_name("document-edit-symbolic");
        btn_edit.add_css_class("flat");
        btn_edit.set_tooltip_text(Some(&i18n::t("Edit")));
        btn_edit.set_sensitive(!is_locked);
        btn_edit.set_visible(!is_locked);
        let key_c = key.to_string();
        let sender_c = sender.input_sender().clone();
        btn_edit.connect_clicked(move |_| {
            let _ = sender_c.send(ServiceConfigMsg::EditInstance(key_c.clone()));
        });
        action_box.append(&btn_edit);

        let btn_del = gtk::Button::from_icon_name("edit-delete-symbolic");
        btn_del.add_css_class("flat");
        btn_del.add_css_class("destructive-action");
        btn_del.set_tooltip_text(Some(&i18n::t("Delete")));
        btn_del.set_sensitive(!is_locked);
        btn_del.set_visible(!is_locked);
        let key_c = key.to_string();
        let sender_c = sender.input_sender().clone();
        btn_del.connect_clicked(move |_| {
            let _ = sender_c.send(ServiceConfigMsg::DeleteInstance(key_c.clone()));
        });
        action_box.append(&btn_del);

        controls.append(&action_box);
        row_box.append(&controls);

        let row = gtk::ListBoxRow::new();
        row.set_child(Some(&row_box));
        row.set_selectable(false);
        row.set_activatable(false);
        list_box.append(&row);
    }
}

// --- Show Add Dialog ---

fn show_add_dialog(
    config: &Arc<AppConfig>,
    registries: &ServiceRegistries,
    category: ServiceCategory,
    available: &[String],
    sender: &ComponentSender<ServiceConfigModel>,
) {
    let dialog = gtk::Window::new();
    dialog.set_title(Some(&i18n::t("Add Service")));
    dialog.set_modal(true);
    dialog.set_default_size(420, 220);
    dialog.add_css_class("settings-dialog");

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 12);
    vbox.set_margin_start(18);
    vbox.set_margin_end(18);
    vbox.set_margin_top(18);
    vbox.set_margin_bottom(18);

    let label = gtk::Label::new(Some(&i18n::t("Select a service to add:")));
    label.set_halign(gtk::Align::Start);
    label.add_css_class("heading");
    vbox.append(&label);

    // Dropdown to select service
    let svc_labels: Vec<String> = available
        .iter()
        .map(|service| service_display_name(service).to_string())
        .collect();
    let svc_str_refs: Vec<&str> = svc_labels.iter().map(|s| s.as_str()).collect();
    let svc_list = gtk::StringList::new(&svc_str_refs);
    let dropdown = gtk::DropDown::new(
        Some(svc_list.upcast::<gtk::gio::ListModel>()),
        gtk::Expression::NONE,
    );
    dropdown.set_hexpand(true);
    dropdown.add_css_class("service-field-control");
    vbox.append(&dropdown);

    // Button row
    let btn_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    btn_row.set_halign(gtk::Align::End);

    let cancel_btn = gtk::Button::with_label(&i18n::t("Cancel"));
    let dialog_c = dialog.clone();
    cancel_btn.connect_clicked(move |_| {
        dialog_c.close();
    });
    btn_row.append(&cancel_btn);

    let add_btn = gtk::Button::with_label(&i18n::t("Add"));
    add_btn.add_css_class("suggested-action");
    let config_c = config.clone();
    let registries_c = registries.clone_registries();
    let cat = category;
    let dialog_c = dialog.clone();
    let available_c = available.to_vec();
    let dropdown_c = dropdown.clone();
    let sender_c = sender.input_sender().clone();
    add_btn.connect_clicked(move |_| {
        let idx = dropdown_c.selected() as usize;
        if idx >= available_c.len() {
            return;
        }
        let name_owned = &available_c[idx];
        let instance_key = AppConfig::generate_instance_key(name_owned);

        let default = registries_c.default_config(cat, name_owned);
        let mut cfg_obj = if let Some(obj) = default.as_object().cloned() {
            obj
        } else {
            serde_json::Map::new()
        };
        cfg_obj.insert(
            "instanceName".into(),
            serde_json::Value::String(service_display_name(name_owned).to_string()),
        );
        cfg_obj.insert("enable".into(), serde_json::Value::Bool(true));
        config_c.set(&instance_key, cfg_obj);

        let mut list = config_c.get_service_list(cat);
        list.push(instance_key.clone());
        config_c.set_service_list(cat, &list);

        let _ = sender_c.send(ServiceConfigMsg::ReloadList);
        dialog_c.close();

        let key = instance_key.clone();
        let name = name_owned.clone();
        show_edit_dialog(&config_c, &registries_c, cat, &key, &name);
    });
    btn_row.append(&add_btn);

    vbox.append(&btn_row);
    dialog.set_child(Some(&vbox));
    dialog.present();
}

// --- Show Edit Dialog ---

fn show_edit_dialog(
    config: &Arc<AppConfig>,
    registries: &ServiceRegistries,
    category: ServiceCategory,
    instance_key: &str,
    service_name: &str,
) {
    let current_cfg = config
        .get(instance_key)
        .unwrap_or_else(|| registries.default_config(category, service_name));

    let fields = build_fields(service_name, &current_cfg, category, registries);

    let dialog = gtk::Window::new();
    dialog.set_title(Some(&format!(
        "{}: {}",
        i18n::t("Configure"),
        service_display_name(service_name)
    )));
    dialog.set_modal(true);
    dialog.set_default_size(560, 560);
    dialog.add_css_class("settings-dialog");

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
    vbox.set_margin_start(18);
    vbox.set_margin_end(18);
    vbox.set_margin_top(18);
    vbox.set_margin_bottom(18);

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_hscrollbar_policy(gtk::PolicyType::Never);

    let fields_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    fields_box.add_css_class("service-edit-card");
    let mut field_widgets: Vec<(String, FieldWidget)> = Vec::new();

    for field in &fields {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
        row.add_css_class("service-field-row");

        let lbl = gtk::Label::new(Some(&field.label));
        lbl.set_size_request(150, -1);
        lbl.set_halign(gtk::Align::Start);
        lbl.set_xalign(0.0);
        lbl.set_wrap(true);
        lbl.add_css_class("setting-label");
        row.append(&lbl);

        let widget = match field.widget_type {
            FieldType::Text => {
                let entry = gtk::Entry::new();
                entry.set_hexpand(true);
                entry.add_css_class("service-field-control");
                if let Some(v) = field.value.as_str() {
                    entry.set_text(v);
                }
                row.append(&entry);
                FieldWidget::Entry(entry)
            }
            FieldType::Password => {
                let entry = gtk::Entry::new();
                entry.set_hexpand(true);
                entry.set_visibility(false);
                entry.add_css_class("service-field-control");
                if let Some(v) = field.value.as_str() {
                    entry.set_text(v);
                }
                row.append(&entry);
                FieldWidget::Entry(entry)
            }
            FieldType::Url => {
                let entry = gtk::Entry::new();
                entry.set_hexpand(true);
                entry.add_css_class("service-field-control");
                if let Some(v) = field.value.as_str() {
                    entry.set_text(v);
                }
                row.append(&entry);
                FieldWidget::Entry(entry)
            }
            FieldType::Switch => {
                let sw = gtk::Switch::new();
                sw.set_active(field.value.as_bool().unwrap_or(false));
                sw.set_halign(gtk::Align::End);
                row.append(&sw);
                FieldWidget::Switch(sw)
            }
            FieldType::Dropdown { ref options } => {
                let display_labels: Vec<&str> = options.iter().map(|(_, d)| d.as_str()).collect();
                let list = gtk::StringList::new(&display_labels);
                let dropdown = gtk::DropDown::new(
                    Some(list.upcast::<gtk::gio::ListModel>()),
                    gtk::Expression::NONE,
                );
                dropdown.set_hexpand(true);
                dropdown.add_css_class("service-field-control");

                let current_val = field.value.as_str().unwrap_or("");
                for (i, (val, _)) in options.iter().enumerate() {
                    if val == current_val {
                        dropdown.set_selected(i as u32);
                        break;
                    }
                }
                row.append(&dropdown);
                FieldWidget::Dropdown {
                    widget: dropdown,
                    options: options.clone(),
                }
            }
        };

        // Conditional visibility for DeepL
        if field.key == "auth_key" {
            let type_val = fields
                .iter()
                .find(|f| f.key == "type")
                .and_then(|f| f.value.as_str())
                .unwrap_or("free");
            row.set_visible(type_val == "api");
            row.add_css_class("deepl-auth-key-row");
        }
        if field.key == "custom_url" && service_name == "deepl" {
            let type_val = fields
                .iter()
                .find(|f| f.key == "type")
                .and_then(|f| f.value.as_str())
                .unwrap_or("free");
            row.set_visible(type_val == "deeplx");
            row.add_css_class("deepl-custom-url-row");
        }

        field_widgets.push((field.key.clone(), widget));
        fields_box.append(&row);
    }

    if service_name == "openai" {
        append_ai_model_fetch_row(&fields_box, &field_widgets);
    }

    // Wire DeepL type dropdown to toggle conditional fields
    if service_name == "deepl" {
        let type_idx = fields.iter().position(|f| f.key == "type");
        if let Some(idx) = type_idx {
            if let Some((
                _,
                FieldWidget::Dropdown {
                    widget: dropdown, ..
                },
            )) = field_widgets.get(idx)
            {
                let fields_box_clone = fields_box.clone();
                dropdown.connect_selected_notify(move |dd| {
                    let types = ["free", "api", "deeplx"];
                    let selected = types.get(dd.selected() as usize).unwrap_or(&"free");
                    let children = fields_box_clone.observe_children();
                    for i in 0..children.n_items() {
                        if let Some(child) = children.item(i) {
                            let Ok(row) = child.downcast::<gtk::Widget>() else {
                                continue;
                            };
                            if row.has_css_class("deepl-auth-key-row") {
                                row.set_visible(selected == &"api");
                            }
                            if row.has_css_class("deepl-custom-url-row") {
                                row.set_visible(selected == &"deeplx");
                            }
                        }
                    }
                });
            }
        }
    }

    scrolled.set_child(Some(&fields_box));
    vbox.append(&scrolled);

    // Buttons
    let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    btn_box.set_halign(gtk::Align::End);

    let btn_cancel = gtk::Button::with_label(&i18n::t("Cancel"));
    let dialog_c = dialog.clone();
    btn_cancel.connect_clicked(move |_| {
        dialog_c.close();
    });
    btn_box.append(&btn_cancel);

    let btn_save = gtk::Button::with_label(&i18n::t("Save"));
    btn_save.add_css_class("suggested-action");
    let config_c = config.clone();
    let key_c = instance_key.to_string();
    let field_widgets_c = field_widgets;
    let dialog_c = dialog.clone();
    btn_save.connect_clicked(move |_| {
        let mut result = serde_json::Map::new();

        for (key, widget) in &field_widgets_c {
            let value = match widget {
                FieldWidget::Entry(e) => serde_json::Value::String(e.text().to_string()),
                FieldWidget::Switch(s) => serde_json::Value::Bool(s.is_active()),
                FieldWidget::Dropdown {
                    widget: dd,
                    options,
                } => {
                    let idx = dd.selected() as usize;
                    if let Some((val, _)) = options.get(idx) {
                        serde_json::Value::String(val.clone())
                    } else {
                        serde_json::Value::String(String::new())
                    }
                }
            };
            result.insert(key.clone(), value);
        }

        // Preserve existing fields not shown in UI
        if let Some(existing) = config_c.get(&key_c) {
            if let Some(obj) = existing.as_object() {
                for (k, v) in obj {
                    if !result.contains_key(k) {
                        result.insert(k.clone(), v.clone());
                    }
                }
            }
        }

        config_c.set(&key_c, result);
        dialog_c.close();
    });
    btn_box.append(&btn_save);

    vbox.append(&btn_box);
    dialog.set_child(Some(&vbox));
    dialog.present();
}

fn append_ai_model_fetch_row(fields_box: &gtk::Box, field_widgets: &[(String, FieldWidget)]) {
    let Some(model_entry) = find_entry_widget(field_widgets, "model") else {
        return;
    };
    let Some(api_key_entry) = find_entry_widget(field_widgets, "api_key") else {
        return;
    };
    let Some(request_url_entry) = find_entry_widget(field_widgets, "request_url") else {
        return;
    };
    let Some((api_format_dropdown, api_format_options)) =
        find_dropdown_widget(field_widgets, "api_format")
    else {
        return;
    };

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    row.add_css_class("service-field-row");

    let label = gtk::Label::new(Some(&i18n::t("Available Models")));
    label.set_size_request(150, -1);
    label.set_xalign(0.0);
    label.add_css_class("setting-label");
    row.append(&label);

    let button = gtk::Button::with_label(&i18n::t("Fetch Models"));
    button.set_hexpand(true);
    button.set_halign(gtk::Align::Fill);
    button.add_css_class("service-field-control");
    button.add_css_class("flat");

    button.connect_clicked(move |btn| {
        let api_format = dropdown_value(&api_format_dropdown, &api_format_options);
        let request_url = request_url_entry.text().to_string();
        let api_key = api_key_entry.text().to_string();

        btn.set_sensitive(false);
        btn.set_label(&i18n::t("Fetching Models..."));

        let result =
            crate::core::runtime::block_on(fetch_ai_models(api_format, request_url, api_key));

        btn.set_sensitive(true);
        btn.set_label(&i18n::t("Fetch Models"));

        match result {
            Ok(Ok(models)) => show_model_picker(&models, &model_entry),
            Ok(Err(message)) => show_model_fetch_error(&message),
            Err(_) => show_model_fetch_error(&i18n::t("Runtime is not available")),
        }
    });

    row.append(&button);
    fields_box.append(&row);
}

fn find_entry_widget(field_widgets: &[(String, FieldWidget)], key: &str) -> Option<gtk::Entry> {
    field_widgets.iter().find_map(|(field_key, widget)| {
        if field_key == key {
            if let FieldWidget::Entry(entry) = widget {
                return Some(entry.clone());
            }
        }
        None
    })
}

fn find_dropdown_widget(
    field_widgets: &[(String, FieldWidget)],
    key: &str,
) -> Option<(gtk::DropDown, Vec<(String, String)>)> {
    field_widgets.iter().find_map(|(field_key, widget)| {
        if field_key == key {
            if let FieldWidget::Dropdown { widget, options } = widget {
                return Some((widget.clone(), options.clone()));
            }
        }
        None
    })
}

fn dropdown_value(dropdown: &gtk::DropDown, options: &[(String, String)]) -> String {
    options
        .get(dropdown.selected() as usize)
        .map(|(value, _)| value.clone())
        .unwrap_or_default()
}

async fn fetch_ai_models(
    api_format: String,
    request_url: String,
    api_key: String,
) -> Result<Vec<String>, String> {
    if api_key.trim().is_empty() {
        return Err(i18n::t("API key is required"));
    }

    let url = model_list_url(&api_format, &request_url);
    let client = crate::services::translate::http_client();
    let mut request = client.get(&url);

    if api_format == "anthropic" {
        request = request
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01");
    } else {
        request = request.header("Authorization", format!("Bearer {}", api_key));
    }

    let response = request.send().await.map_err(|e| e.to_string())?;
    let status = response.status();
    let body = response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;

    if !status.is_success() {
        return Err(body
            .get("error")
            .and_then(|err| err.get("message").or(Some(err)))
            .and_then(|v| v.as_str())
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("HTTP {}", status.as_u16())));
    }

    let mut models = BTreeSet::new();
    if let Some(items) = body.get("data").and_then(|v| v.as_array()) {
        for item in items {
            if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                models.insert(id.to_string());
            }
        }
    }

    if models.is_empty() {
        Err(i18n::t("No models found"))
    } else {
        Ok(models.into_iter().collect())
    }
}

fn model_list_url(api_format: &str, request_url: &str) -> String {
    let default = if api_format == "anthropic" {
        "https://api.anthropic.com/v1/models"
    } else {
        "https://api.openai.com/v1/models"
    };
    let trimmed = request_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return default.to_string();
    }

    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return if trimmed.starts_with('/') {
            let host = if api_format == "anthropic" {
                "https://api.anthropic.com"
            } else {
                "https://api.openai.com"
            };
            format!("{}{}", host, trimmed)
        } else {
            trimmed.to_string()
        };
    }

    if trimmed.ends_with("/models") {
        trimmed.to_string()
    } else if trimmed.ends_with("/chat/completions") {
        format!(
            "{}/models",
            trimmed
                .trim_end_matches("/chat/completions")
                .trim_end_matches('/')
        )
    } else if trimmed.ends_with("/messages") {
        format!(
            "{}/models",
            trimmed.trim_end_matches("/messages").trim_end_matches('/')
        )
    } else if trimmed.ends_with("/v1") {
        format!("{}/models", trimmed)
    } else {
        trimmed.to_string()
    }
}

fn show_model_picker(models: &[String], model_entry: &gtk::Entry) {
    let dialog = gtk::Window::new();
    dialog.set_title(Some(&i18n::t("Available Models")));
    dialog.set_modal(true);
    dialog.set_default_size(460, 420);
    dialog.add_css_class("settings-dialog");

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 10);
    vbox.set_margin_start(18);
    vbox.set_margin_end(18);
    vbox.set_margin_top(18);
    vbox.set_margin_bottom(18);

    let list = gtk::Box::new(gtk::Orientation::Vertical, 4);
    list.add_css_class("service-model-list");

    for model in models {
        let button = gtk::Button::with_label(model);
        button.set_halign(gtk::Align::Fill);
        button.add_css_class("flat");
        button.add_css_class("service-model-row");
        let model_c = model.clone();
        let entry_c = model_entry.clone();
        let dialog_c = dialog.clone();
        button.connect_clicked(move |_| {
            entry_c.set_text(&model_c);
            dialog_c.close();
        });
        list.append(&button);
    }

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(&list));
    vbox.append(&scrolled);

    dialog.set_child(Some(&vbox));
    dialog.present();
}

fn show_model_fetch_error(message: &str) {
    let dialog = gtk::Window::new();
    dialog.set_title(Some(&i18n::t("Fetch Models")));
    dialog.set_modal(true);
    dialog.set_default_size(360, 160);
    dialog.add_css_class("settings-dialog");

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 12);
    vbox.set_margin_start(18);
    vbox.set_margin_end(18);
    vbox.set_margin_top(18);
    vbox.set_margin_bottom(18);

    let label = gtk::Label::new(Some(message));
    label.set_wrap(true);
    label.set_xalign(0.0);
    label.add_css_class("dim-label");
    vbox.append(&label);

    let close = gtk::Button::with_label(&i18n::t("Close"));
    close.set_halign(gtk::Align::End);
    let dialog_c = dialog.clone();
    close.connect_clicked(move |_| {
        dialog_c.close();
    });
    vbox.append(&close);

    dialog.set_child(Some(&vbox));
    dialog.present();
}

// --- Field types ---

enum FieldType {
    Text,
    Password,
    Url,
    Switch,
    Dropdown { options: Vec<(String, String)> },
}

struct FieldDef {
    key: String,
    label: String,
    widget_type: FieldType,
    value: serde_json::Value,
}

enum FieldWidget {
    Entry(gtk::Entry),
    Switch(gtk::Switch),
    Dropdown {
        widget: gtk::DropDown,
        options: Vec<(String, String)>,
    },
}

fn build_fields(
    service_name: &str,
    config: &serde_json::Value,
    category: ServiceCategory,
    registries: &ServiceRegistries,
) -> Vec<FieldDef> {
    match category {
        ServiceCategory::Translate => match service_name {
            "google" => vec![
                name_field(config),
                url_field(
                    config,
                    "custom_url",
                    &i18n::t("Custom URL"),
                    "https://translate.google.com",
                ),
            ],
            "bing" => vec![name_field(config)],
            "deepl" => vec![
                name_field(config),
                FieldDef {
                    key: "type".into(),
                    label: i18n::t("Type"),
                    widget_type: FieldType::Dropdown {
                        options: vec![
                            ("free".into(), i18n::t("Free")),
                            ("api".into(), i18n::t("API")),
                            ("deeplx".into(), "DeepLX".into()),
                        ],
                    },
                    value: config
                        .get("type")
                        .cloned()
                        .unwrap_or(serde_json::json!("free")),
                },
                FieldDef {
                    key: "auth_key".into(),
                    label: i18n::t("Auth Key"),
                    widget_type: FieldType::Password,
                    value: config
                        .get("auth_key")
                        .cloned()
                        .unwrap_or(serde_json::json!("")),
                },
                url_field(config, "custom_url", &i18n::t("Custom URL"), ""),
            ],
            "openai" => vec![
                name_field(config),
                FieldDef {
                    key: "api_format".into(),
                    label: i18n::t("API Format"),
                    widget_type: FieldType::Dropdown {
                        options: vec![
                            ("openai_compatible".into(), i18n::t("OpenAI-compatible API")),
                            ("anthropic".into(), i18n::t("Anthropic API")),
                        ],
                    },
                    value: config
                        .get("api_format")
                        .cloned()
                        .unwrap_or(serde_json::json!("openai_compatible")),
                },
                FieldDef {
                    key: "api_key".into(),
                    label: i18n::t("API Key"),
                    widget_type: FieldType::Password,
                    value: config
                        .get("api_key")
                        .cloned()
                        .unwrap_or(serde_json::json!("")),
                },
                text_field(config, "model", &i18n::t("Model"), "gpt-3.5-turbo"),
                ai_url_field(config),
            ],
            "baidu" => vec![
                name_field(config),
                text_field(config, "appid", &i18n::t("App ID"), ""),
                FieldDef {
                    key: "secret".into(),
                    label: i18n::t("Secret Key"),
                    widget_type: FieldType::Password,
                    value: config
                        .get("secret")
                        .cloned()
                        .unwrap_or(serde_json::json!("")),
                },
            ],
            "youdao" => vec![
                name_field(config),
                text_field(config, "appkey", &i18n::t("App Key"), ""),
                FieldDef {
                    key: "key".into(),
                    label: i18n::t("Key"),
                    widget_type: FieldType::Password,
                    value: config.get("key").cloned().unwrap_or(serde_json::json!("")),
                },
            ],
            "lingva" => vec![
                name_field(config),
                url_field(
                    config,
                    "custom_url",
                    &i18n::t("Custom URL"),
                    "https://lingva.pot-app.com",
                ),
            ],
            _ => generic_fields(service_name, config, registries, category),
        },
        ServiceCategory::Recognize => match service_name {
            "tesseract" => vec![name_field(config)],
            _ => generic_fields(service_name, config, registries, category),
        },
        ServiceCategory::Tts => match service_name {
            "lingva_tts" => vec![
                name_field(config),
                url_field(
                    config,
                    "requestPath",
                    &i18n::t("Host"),
                    "lingva.pot-app.com",
                ),
            ],
            _ => generic_fields(service_name, config, registries, category),
        },
        ServiceCategory::Collection => match service_name {
            "anki" => vec![
                name_field(config),
                url_field(
                    config,
                    "requestPath",
                    &i18n::t("AnkiConnect URL"),
                    "http://127.0.0.1:8765",
                ),
                text_field(config, "deck", &i18n::t("Deck"), "Pot"),
                text_field(config, "model", &i18n::t("Note Type"), "Basic"),
            ],
            _ => generic_fields(service_name, config, registries, category),
        },
    }
}

// --- Field helpers ---

fn name_field(config: &serde_json::Value) -> FieldDef {
    FieldDef {
        key: "instanceName".into(),
        label: i18n::t("Instance Name"),
        widget_type: FieldType::Text,
        value: config
            .get("instanceName")
            .cloned()
            .unwrap_or(serde_json::json!("")),
    }
}

fn text_field(config: &serde_json::Value, key: &str, label: &str, default: &str) -> FieldDef {
    FieldDef {
        key: key.into(),
        label: label.into(),
        widget_type: FieldType::Text,
        value: config
            .get(key)
            .cloned()
            .unwrap_or(serde_json::json!(default)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestConfigContext {
        _dir: tempfile::TempDir,
        config: AppConfig,
    }

    fn test_config() -> TestConfigContext {
        let dir = tempfile::tempdir().unwrap();
        let config = AppConfig::new_in_dir(dir.path().join("com.pot-app.desktop"));
        TestConfigContext { _dir: dir, config }
    }

    #[test]
    fn locked_default_instances_are_google_and_bing() {
        assert!(is_locked_default_instance(
            ServiceCategory::Translate,
            "google@abc123"
        ));
        assert!(is_locked_default_instance(
            ServiceCategory::Translate,
            "bing@def456"
        ));
        assert!(!is_locked_default_instance(
            ServiceCategory::Translate,
            "deepl@ghi789"
        ));
    }

    #[test]
    fn add_dialog_filters_locked_default_translate_services() {
        let available = vec![
            "google".to_string(),
            "bing".to_string(),
            "deepl".to_string(),
            "openai".to_string(),
        ];

        let filtered = addable_services(ServiceCategory::Translate, &available);

        assert_eq!(filtered, vec!["deepl".to_string(), "openai".to_string()]);
    }

    #[test]
    fn service_page_keys_append_missing_locked_defaults() {
        let ctx = test_config();
        ctx.config.set_service_list(
            ServiceCategory::Translate,
            &["deepl@demo".to_string(), "openai@demo".to_string()],
        );

        let keys = service_page_keys(&ctx.config, ServiceCategory::Translate);

        assert_eq!(
            keys,
            vec![
                "deepl@demo".to_string(),
                "openai@demo".to_string(),
                "google".to_string(),
                "bing".to_string(),
            ]
        );
    }

    #[test]
    fn service_page_keys_keep_locked_defaults_when_user_list_is_empty() {
        let ctx = test_config();
        ctx.config
            .set_service_list(ServiceCategory::Translate, &Vec::<String>::new());

        let keys = service_page_keys(&ctx.config, ServiceCategory::Translate);

        assert_eq!(keys, vec!["google".to_string(), "bing".to_string()]);
    }

    #[test]
    fn service_page_keys_keep_locked_defaults_after_deleting_custom_service() {
        let ctx = test_config();
        ctx.config
            .set_service_list(ServiceCategory::Translate, &["openai@demo".to_string()]);

        let mut list = ctx.config.get_service_list(ServiceCategory::Translate);
        list.retain(|key| key != "openai@demo");
        ctx.config
            .set_service_list(ServiceCategory::Translate, &list);

        let keys = service_page_keys(&ctx.config, ServiceCategory::Translate);

        assert_eq!(keys, vec!["google".to_string(), "bing".to_string()]);
    }

    #[test]
    fn display_instance_key_uses_ai_prefix_for_openai() {
        assert_eq!(display_instance_key("openai@b4438e9d"), "ai@b4438e9d");
        assert_eq!(display_instance_key("openai"), "ai");
        assert_eq!(display_instance_key("deepl@demo"), "deepl@demo");
    }

    #[test]
    fn non_translate_categories_do_not_filter_google_or_bing() {
        let available = vec!["google".to_string(), "bing".to_string()];

        let filtered = addable_services(ServiceCategory::Tts, &available);

        assert_eq!(filtered, available);
    }
}

fn url_field(config: &serde_json::Value, key: &str, label: &str, default: &str) -> FieldDef {
    FieldDef {
        key: key.into(),
        label: label.into(),
        widget_type: FieldType::Url,
        value: config
            .get(key)
            .cloned()
            .unwrap_or(serde_json::json!(default)),
    }
}

fn ai_url_field(config: &serde_json::Value) -> FieldDef {
    let value = config
        .get("request_url")
        .cloned()
        .or_else(|| config.get("request_path").cloned())
        .unwrap_or(serde_json::json!(
            "https://api.openai.com/v1/chat/completions"
        ));
    FieldDef {
        key: "request_url".into(),
        label: i18n::t("Request URL"),
        widget_type: FieldType::Url,
        value,
    }
}

fn generic_fields(
    service_name: &str,
    config: &serde_json::Value,
    registries: &ServiceRegistries,
    category: ServiceCategory,
) -> Vec<FieldDef> {
    let default = registries.default_config(category, service_name);
    let mut fields = vec![name_field(config)];

    if let Some(obj) = default.as_object() {
        for (key, default_val) in obj {
            let current = config
                .get(key)
                .cloned()
                .unwrap_or_else(|| default_val.clone());
            let widget_type = infer_widget_type(key, default_val);
            fields.push(FieldDef {
                key: key.clone(),
                label: prettify_label(key),
                widget_type,
                value: current,
            });
        }
    }

    fields
}

fn infer_widget_type(key: &str, default: &serde_json::Value) -> FieldType {
    match default {
        v if v.is_boolean() => FieldType::Switch,
        v if v.is_string() => {
            if key.contains("key")
                || key.contains("secret")
                || key.contains("password")
                || key.contains("token")
            {
                FieldType::Password
            } else if key.contains("url")
                || key.contains("path")
                || key.contains("host")
                || key.contains("Path")
            {
                FieldType::Url
            } else {
                FieldType::Text
            }
        }
        _ => FieldType::Text,
    }
}

fn prettify_label(key: &str) -> String {
    key.split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
