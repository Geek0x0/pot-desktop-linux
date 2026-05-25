use crate::config::AppConfig;
use crate::config::ServiceCategory;
use crate::core::clipboard;
use crate::core::history::HistoryStore;
use crate::i18n;
use crate::services::collection::{self as collection_svc};
use crate::services::translate;
use crate::services::tts::{self as tts_svc};
use crate::services::types::{TranslateRequest, TranslateResult};
use gtk::prelude::*;
use relm4::prelude::*;
use std::cell::Cell;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct TranslationItem {
    instance_key: String,
    service_name: String,
    display_name: String,
    text: String,
    is_error: bool,
}

#[derive(Debug, Clone)]
pub struct TranslateCommandOutput {
    request_id: u64,
    replace_instance_key: Option<String>,
    items: Vec<TranslationItem>,
}

pub struct TranslateModel {
    text: String,
    translated_texts: Vec<TranslationItem>,
    pinned: bool,
    pin_btn: gtk::Button,
    from_lang: String,
    to_lang: String,
    config: Arc<AppConfig>,
    registry: Arc<translate::TranslateRegistry>,
    tts_registry: Arc<tts_svc::TtsRegistry>,
    collection_registry: Arc<collection_svc::CollectionRegistry>,
    history: Arc<HistoryStore>,
    loading: bool,
    setting_source: bool,
    lang_codes: Vec<String>,
    from_dropdown: gtk::DropDown,
    to_dropdown: gtk::DropDown,
    active_request_id: u64,
    results_version: u64,
    rendered_results_version: Cell<u64>,
}

#[derive(Debug)]
pub enum TranslateMsg {
    Show(String),
    Translate,
    SetSourceText(String),
    SwapLanguages,
    SetFromLang(String),
    SetToLang(String),
    RetryService(String),
    DisableFailedService(String),
    Pin,
    FocusOut,
    Close,
}

#[derive(Debug)]
pub enum TranslateOutput {
    Closed,
}

#[relm4::component(pub)]
impl Component for TranslateModel {
    type Init = (
        Arc<AppConfig>,
        Arc<translate::TranslateRegistry>,
        Arc<tts_svc::TtsRegistry>,
        Arc<collection_svc::CollectionRegistry>,
        Arc<HistoryStore>,
    );
    type Input = TranslateMsg;
    type Output = TranslateOutput;
    type CommandOutput = TranslateCommandOutput;

    view! {
        gtk::Window {
            set_decorated: false,
            set_default_width: 420,
            set_default_height: 520,
            set_icon_name: Some("com.pot-app.pot-gtk"),
            add_css_class: "popup-overlay",

            connect_close_request[sender] => move |_| {
                let _ = sender.output(TranslateOutput::Closed);
                gtk::glib::Propagation::Stop
            },

            #[name = "toolbar_view"]
            adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {
                    set_show_start_title_buttons: false,
                    set_show_end_title_buttons: false,

                    #[wrap(Some)]
                    set_title_widget = &gtk::Label {
                        set_label: &i18n::t("Pot"),
                        add_css_class: "heading",
                    },

                    #[name = "pin_btn"]
                    pack_start = &gtk::Button {
                        set_icon_name: "view-pin",
                        set_tooltip_text: Some(&i18n::t("Pin")),
                        add_css_class: "flat",
                    },

                    pack_end = &gtk::Button {
                        set_icon_name: "window-close",
                        set_tooltip_text: Some(&i18n::t("Close")),
                        add_css_class: "flat",
                        connect_clicked => TranslateMsg::Close,
                    },
                },

                // Content
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 8,
                    set_margin_start: 6,
                    set_margin_end: 6,
                    set_margin_bottom: 6,

                    // Source text area
                    gtk::ScrolledWindow {
                        set_min_content_height: 80,
                        set_vexpand: false,

                        #[name = "source_view"]
                        gtk::TextView {
                            set_wrap_mode: gtk::WrapMode::WordChar,
                            set_top_margin: 8,
                            set_left_margin: 8,
                            set_right_margin: 8,
                            set_bottom_margin: 8,
                        },
                    },

                    // Language selector + translate button
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 6,

                        #[name = "from_lang_combo"]
                        gtk::DropDown {
                            set_hexpand: true,
                        },

                        gtk::Button {
                            set_icon_name: "object-flip-horizontal",
                            set_tooltip_text: Some(&i18n::t("Swap")),
                            add_css_class: "flat",
                            connect_clicked => TranslateMsg::SwapLanguages,
                        },

                        #[name = "to_lang_combo"]
                        gtk::DropDown {
                            set_hexpand: true,
                        },

                        gtk::Button {
                            set_label: &i18n::t("Translate"),
                            set_hexpand: false,
                            add_css_class: "suggested-action",
                            connect_clicked => TranslateMsg::Translate,
                        },
                    },

                    // Results area
                    gtk::ScrolledWindow {
                        set_vexpand: true,
                        set_min_content_height: 100,

                        #[name = "results_list"]
                        gtk::ListBox {
                            set_selection_mode: gtk::SelectionMode::None,
                            add_css_class: "boxed-list",
                        },
                    },

                    // Loading indicator
                    #[name = "spinner"]
                    gtk::Spinner {
                        set_visible: false,
                        set_margin_top: 4,
                        set_margin_bottom: 4,
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
        let (config, registry, tts_registry, collection_registry, history) = init;

        let from_lang = config
            .get("translate_source_language")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "auto".into());
        let to_lang = config
            .get("translate_target_language")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "zh_cn".into());

        // Build language dropdowns before creating the model
        let languages = crate::lang::Language::all();
        let mut lang_codes: Vec<String> = Vec::new();
        let mut lang_labels: Vec<String> = Vec::new();
        let mut from_selected: u32 = 0;
        let mut to_selected: u32 = 0;
        for (i, lang) in languages.iter().enumerate() {
            lang_codes.push(lang.code().to_string());
            lang_labels.push(lang.display_name().to_string());
            if lang.code() == from_lang {
                from_selected = i as u32;
            }
            if lang.code() == to_lang {
                to_selected = i as u32;
            }
        }
        let lang_str_refs: Vec<&str> = lang_labels.iter().map(|s| s.as_str()).collect();
        let from_list = gtk::StringList::new(&lang_str_refs);
        let from_dropdown = gtk::DropDown::new(
            Some(from_list.upcast::<gtk::gio::ListModel>()),
            gtk::Expression::NONE,
        );
        from_dropdown.set_hexpand(true);
        from_dropdown.set_selected(from_selected);

        let to_list = gtk::StringList::new(&lang_str_refs);
        let to_dropdown = gtk::DropDown::new(
            Some(to_list.upcast::<gtk::gio::ListModel>()),
            gtk::Expression::NONE,
        );
        to_dropdown.set_hexpand(true);
        to_dropdown.set_selected(to_selected);

        let pin_btn_placeholder = gtk::Button::new();

        let mut model = TranslateModel {
            text: String::new(),
            translated_texts: Vec::new(),
            pinned: false,
            pin_btn: pin_btn_placeholder,
            from_lang,
            to_lang,
            config,
            registry,
            tts_registry,
            collection_registry,
            history,
            loading: false,
            setting_source: false,
            lang_codes: lang_codes.clone(),
            from_dropdown: from_dropdown.clone(),
            to_dropdown: to_dropdown.clone(),
            active_request_id: 0,
            results_version: 0,
            rendered_results_version: Cell::new(0),
        };

        let widgets = view_output!();

        // Store pin_btn reference and connect clicked signal
        let pin_btn_ref = widgets.pin_btn.clone();
        let sender_pin = sender.input_sender().clone();
        widgets.pin_btn.connect_clicked(move |_| {
            let _ = sender_pin.send(TranslateMsg::Pin);
        });

        // Replace the placeholder dropdowns in the view
        let from_parent = widgets.from_lang_combo.parent().unwrap();
        let to_parent = widgets.to_lang_combo.parent().unwrap();
        let from_box = from_parent.downcast::<gtk::Box>().unwrap();
        let to_box = to_parent.downcast::<gtk::Box>().unwrap();

        let from_pos = {
            let children = from_box.observe_children();
            let mut pos = 0u32;
            for i in 0..children.n_items() {
                if let Some(child) = children.item(i) {
                    if child == widgets.from_lang_combo {
                        pos = i;
                        break;
                    }
                }
            }
            pos
        };
        from_box.remove(&widgets.from_lang_combo);
        let from_sibling = if from_pos > 0 {
            let children = from_box.observe_children();
            children.item(from_pos - 1).and_downcast::<gtk::Widget>()
        } else {
            None
        };
        from_box.insert_child_after(&from_dropdown, from_sibling.as_ref());

        let to_pos = {
            let children = to_box.observe_children();
            let mut pos = 0u32;
            for i in 0..children.n_items() {
                if let Some(child) = children.item(i) {
                    if child == widgets.to_lang_combo {
                        pos = i;
                        break;
                    }
                }
            }
            pos
        };
        to_box.remove(&widgets.to_lang_combo);
        let to_sibling = if to_pos > 0 {
            let children = to_box.observe_children();
            children.item(to_pos - 1).and_downcast::<gtk::Widget>()
        } else {
            None
        };
        to_box.insert_child_after(&to_dropdown, to_sibling.as_ref());

        // Track source text changes
        let buffer = widgets.source_view.buffer();
        let sender_clone = sender.input_sender().clone();
        buffer.connect_changed(move |buf| {
            let text = buf.text(&buf.start_iter(), &buf.end_iter(), false);
            let _ = sender_clone.send(TranslateMsg::SetSourceText(text.to_string()));
        });

        // Track language dropdown changes
        let lang_codes_from = lang_codes.clone();
        let sender_clone = sender.input_sender().clone();
        from_dropdown.connect_selected_notify(move |dd| {
            let idx = dd.selected() as usize;
            if idx < lang_codes_from.len() {
                let _ = sender_clone.send(TranslateMsg::SetFromLang(lang_codes_from[idx].clone()));
            }
        });
        let lang_codes_to = lang_codes.clone();
        let sender_clone = sender.input_sender().clone();
        to_dropdown.connect_selected_notify(move |dd| {
            let idx = dd.selected() as usize;
            if idx < lang_codes_to.len() {
                let _ = sender_clone.send(TranslateMsg::SetToLang(lang_codes_to[idx].clone()));
            }
        });

        // Close-on-blur: hide window when it loses focus
        let close_on_blur = model
            .config
            .get("translate_close_on_blur")
            .map(|v| v.as_bool().unwrap_or(true))
            .unwrap_or(true);
        if close_on_blur {
            let focus_controller = gtk::EventControllerFocus::new();
            let sender_focus = sender.input_sender().clone();
            focus_controller.connect_leave(move |_controller| {
                let _ = sender_focus.send(TranslateMsg::FocusOut);
            });
            root.add_controller(focus_controller);
        }

        model.pin_btn = pin_btn_ref;

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            TranslateMsg::Show(text) => {
                self.setting_source = true;
                if self.text != text {
                    self.active_request_id = self.active_request_id.wrapping_add(1);
                    self.loading = false;
                    self.translated_texts.clear();
                    self.results_version = self.results_version.wrapping_add(1);
                }
                self.text = text;

                if !self.text.is_empty() {
                    let _ = sender.input_sender().send(TranslateMsg::Translate);
                } else {
                    self.translated_texts.clear();
                    self.results_version = self.results_version.wrapping_add(1);
                }
            }
            TranslateMsg::SetSourceText(text) => {
                if !self.setting_source {
                    if self.text != text {
                        self.text = text;
                        self.active_request_id = self.active_request_id.wrapping_add(1);
                        self.loading = false;
                        self.translated_texts.clear();
                        self.results_version = self.results_version.wrapping_add(1);
                    }
                }
                self.setting_source = false;
            }
            TranslateMsg::Translate => {
                if self.text.is_empty() || self.loading {
                    return;
                }

                self.active_request_id = self.active_request_id.wrapping_add(1);
                let request_id = self.active_request_id;
                self.loading = true;
                self.translated_texts.clear();
                self.results_version = self.results_version.wrapping_add(1);

                let service_list = enabled_translate_service_list(&self.config);
                if service_list.is_empty() {
                    self.loading = false;
                    return;
                }

                let text = self.text.clone();
                let delete_newline = self
                    .config
                    .get("translate_delete_newline")
                    .map(|v| v.as_bool().unwrap_or(false))
                    .unwrap_or(false);
                let text = if delete_newline {
                    text.replace('\n', " ").replace('\r', "")
                } else {
                    text
                };
                let from = self.from_lang.clone();
                let to = self.to_lang.clone();
                let registry = self.registry.clone();
                let config = self.config.clone();

                sender.spawn_command(move |out_sender| {
                    let Some(handle) = crate::core::runtime::handle() else {
                        log::error!("Translate: shared runtime not available");
                        let _ = out_sender.send(TranslateCommandOutput {
                            request_id,
                            replace_instance_key: None,
                            items: Vec::new(),
                        });
                        return;
                    };
                    handle.spawn(async move {
                        let results =
                            translate_instances(registry, service_list, text, from, to, config)
                                .await;
                        let _ = out_sender.send(TranslateCommandOutput {
                            request_id,
                            replace_instance_key: None,
                            items: results,
                        });
                    });
                });
            }
            TranslateMsg::RetryService(instance_key) => {
                if self.text.is_empty() || self.loading {
                    return;
                }

                self.active_request_id = self.active_request_id.wrapping_add(1);
                let request_id = self.active_request_id;
                self.loading = true;
                let text = self.text.clone();
                let delete_newline = self
                    .config
                    .get("translate_delete_newline")
                    .map(|v| v.as_bool().unwrap_or(false))
                    .unwrap_or(false);
                let text = if delete_newline {
                    text.replace('\n', " ").replace('\r', "")
                } else {
                    text
                };
                let from = self.from_lang.clone();
                let to = self.to_lang.clone();
                let registry = self.registry.clone();
                let config = self.config.clone();
                let service_list = vec![instance_key.clone()];

                sender.spawn_command(move |out_sender| {
                    let Some(handle) = crate::core::runtime::handle() else {
                        log::error!("Translate retry: shared runtime not available");
                        let _ = out_sender.send(TranslateCommandOutput {
                            request_id,
                            replace_instance_key: Some(instance_key),
                            items: Vec::new(),
                        });
                        return;
                    };
                    handle.spawn(async move {
                        let results =
                            translate_instances(registry, service_list, text, from, to, config)
                                .await;
                        let _ = out_sender.send(TranslateCommandOutput {
                            request_id,
                            replace_instance_key: Some(instance_key),
                            items: results,
                        });
                    });
                });
            }
            TranslateMsg::DisableFailedService(instance_key) => {
                let mut cfg = self
                    .config
                    .get(&instance_key)
                    .and_then(|v| v.as_object().cloned())
                    .unwrap_or_default();
                cfg.insert("enable".into(), serde_json::Value::Bool(false));
                self.config.set(&instance_key, cfg);
                self.translated_texts
                    .retain(|item| item.instance_key != instance_key);
                self.results_version = self.results_version.wrapping_add(1);
            }
            TranslateMsg::SwapLanguages => {
                std::mem::swap(&mut self.from_lang, &mut self.to_lang);
                self.active_request_id = self.active_request_id.wrapping_add(1);
                self.loading = false;
                self.translated_texts.clear();
                self.results_version = self.results_version.wrapping_add(1);
                if let Some(idx) = self.lang_codes.iter().position(|c| *c == self.from_lang) {
                    self.from_dropdown.set_selected(idx as u32);
                }
                if let Some(idx) = self.lang_codes.iter().position(|c| *c == self.to_lang) {
                    self.to_dropdown.set_selected(idx as u32);
                }
            }
            TranslateMsg::SetFromLang(lang) => {
                if self.from_lang != lang {
                    self.from_lang = lang;
                    self.active_request_id = self.active_request_id.wrapping_add(1);
                    self.loading = false;
                    self.translated_texts.clear();
                    self.results_version = self.results_version.wrapping_add(1);
                }
            }
            TranslateMsg::SetToLang(lang) => {
                if self.to_lang != lang {
                    self.to_lang = lang;
                    self.active_request_id = self.active_request_id.wrapping_add(1);
                    self.loading = false;
                    self.translated_texts.clear();
                    self.results_version = self.results_version.wrapping_add(1);
                }
            }
            TranslateMsg::FocusOut => {
                if !self.pinned {
                    let _ = sender.output(TranslateOutput::Closed);
                }
            }
            TranslateMsg::Pin => {
                self.pinned = !self.pinned;
            }
            TranslateMsg::Close => {
                let _ = sender.output(TranslateOutput::Closed);
            }
        }
    }

    fn update_cmd(
        &mut self,
        results: Self::CommandOutput,
        _sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        if results.request_id != self.active_request_id {
            return;
        }

        self.loading = false;
        if let Some(instance_key) = &results.replace_instance_key {
            if let Some(item) = results.items.into_iter().next() {
                if let Some(existing) = self
                    .translated_texts
                    .iter_mut()
                    .find(|old| old.instance_key == *instance_key)
                {
                    *existing = item;
                } else {
                    self.translated_texts.push(item);
                }
            }
        } else {
            self.translated_texts = results.items;
        }
        self.results_version = self.results_version.wrapping_add(1);

        // Record first successful result to history
        if let Some(item) = self.translated_texts.iter().find(|item| !item.is_error) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let _ = self.history.insert(
                &self.text,
                &item.text,
                &self.from_lang,
                &self.to_lang,
                &item.service_name,
                now,
            );
        }
    }

    fn post_view() {
        // Pin button state
        crate::windows::set_pin_button_state(&model.pin_btn, model.pinned);

        // Sync source text to buffer
        let buf = source_view.buffer();
        let current = buf
            .text(&buf.start_iter(), &buf.end_iter(), false)
            .to_string();
        if current != model.text {
            buf.set_text(&model.text);
        }

        // Spinner
        spinner.set_visible(model.loading);
        if model.loading {
            spinner.start();
        } else {
            spinner.stop();
        }

        // Rebuild results list when result content changes, even when the
        // number of services stays the same after a retry.
        if model.rendered_results_version.get() != model.results_version {
            clear_results_list(&results_list);
            model.rendered_results_version.set(model.results_version);

            for item in &model.translated_texts {
                let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
                row.add_css_class("result-card");

                let header = gtk::Box::new(gtk::Orientation::Horizontal, 4);
                header.set_hexpand(true);

                let label_svc = gtk::Label::new(Some(&item.display_name));
                label_svc.set_halign(gtk::Align::Start);
                label_svc.set_hexpand(true);
                label_svc.set_xalign(0.0);
                label_svc.add_css_class("caption");
                label_svc.add_css_class("dim-label");
                header.append(&label_svc);

                let retry_btn = gtk::Button::from_icon_name("view-refresh-symbolic");
                retry_btn.add_css_class("flat");
                retry_btn.set_tooltip_text(Some(&i18n::t("Retry Service")));
                let retry_key = item.instance_key.clone();
                let retry_sender = sender.input_sender().clone();
                retry_btn.connect_clicked(move |_| {
                    let _ = retry_sender.send(TranslateMsg::RetryService(retry_key.clone()));
                });
                header.append(&retry_btn);

                if item.is_error {
                    let disable_btn = gtk::Button::from_icon_name("process-stop-symbolic");
                    disable_btn.add_css_class("flat");
                    disable_btn.add_css_class("destructive-action");
                    disable_btn.set_tooltip_text(Some(&i18n::t("Disable Failed Service")));
                    let disable_key = item.instance_key.clone();
                    let disable_sender = sender.input_sender().clone();
                    disable_btn.connect_clicked(move |_| {
                        let _ = disable_sender
                            .send(TranslateMsg::DisableFailedService(disable_key.clone()));
                    });
                    header.append(&disable_btn);
                }

                let copy_btn = gtk::Button::from_icon_name("edit-copy");
                copy_btn.add_css_class("flat");
                copy_btn.set_tooltip_text(Some(&i18n::t("Copy")));
                let text_copy = item.text.clone();
                copy_btn.connect_clicked(move |_| {
                    let _ = clipboard::ClipboardMonitor::write_text(&text_copy);
                });
                header.append(&copy_btn);

                // TTS button
                let speak_btn = gtk::Button::from_icon_name("audio-speakers");
                speak_btn.add_css_class("flat");
                speak_btn.set_tooltip_text(Some(&i18n::t("Speak")));
                let text_speak = item.text.clone();
                let tts_reg = model.tts_registry.clone();
                let tts_config = model.config.clone();
                let tts_lang = model.to_lang.clone();
                speak_btn.connect_clicked(move |_| {
                    let text = text_speak.clone();
                    let reg = tts_reg.clone();
                    let cfg = tts_config.clone();
                    let lang = tts_lang.clone();
                    std::thread::spawn(move || {
                        use base64::{engine::general_purpose, Engine as _};

                        let svc_list = cfg
                            .get("tts_service_list")
                            .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
                            .unwrap_or_default();
                        let svc_name = svc_list.first().map(|s| s.as_str()).unwrap_or("lingva_tts");
                        let instance_config = cfg.get(svc_name).unwrap_or_else(|| {
                            reg.get(svc_name)
                                .map(|s| s.default_config())
                                .unwrap_or(serde_json::Value::Null)
                        });
                        let req = tts_svc::TtsRequest {
                            text,
                            language: lang,
                            config: instance_config,
                        };
                        let speak_result =
                            match crate::core::runtime::block_on(reg.speak(svc_name, req)) {
                                Ok(r) => r,
                                Err(_) => {
                                    log::error!("TTS: shared runtime not available");
                                    return;
                                }
                            };
                        match speak_result {
                            Ok(result) => {
                                if let Ok(data) =
                                    general_purpose::STANDARD.decode(&result.audio_base64)
                                {
                                    let path = std::env::temp_dir()
                                        .join(format!("pot_tts_{}.wav", crate::util::nanoid(6)));
                                    if std::fs::write(&path, &data).is_ok() {
                                        #[cfg(feature = "tts")]
                                        {
                                            play_audio_gstreamer(&path);
                                        }
                                        #[cfg(not(feature = "tts"))]
                                        {
                                            let _ = std::process::Command::new("aplay")
                                                .arg(&path)
                                                .status();
                                        }
                                        // Clean up after a delay
                                        let cleanup_path = path.clone();
                                        std::thread::spawn(move || {
                                            std::thread::sleep(std::time::Duration::from_secs(30));
                                            let _ = std::fs::remove_file(&cleanup_path);
                                        });
                                    }
                                }
                            }
                            Err(e) => log::warn!("TTS failed: {}", e),
                        }
                    });
                });
                header.append(&speak_btn);

                // Collect button
                let collect_btn = gtk::Button::from_icon_name("bookmark-new");
                collect_btn.add_css_class("flat");
                collect_btn.set_tooltip_text(Some(&i18n::t("Collect")));
                let text_collect_src = model.text.clone();
                let text_collect_result = item.text.clone();
                let col_reg = model.collection_registry.clone();
                let col_config = model.config.clone();
                collect_btn.connect_clicked(move |_| {
                    let src = text_collect_src.clone();
                    let result = text_collect_result.clone();
                    let reg = col_reg.clone();
                    let cfg = col_config.clone();
                    std::thread::spawn(move || {
                        let svc_list = cfg
                            .get("collection_service_list")
                            .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
                            .unwrap_or_default();
                        let svc_name = svc_list.first().map(|s| s.as_str()).unwrap_or("anki");
                        let instance_config = cfg.get(svc_name).unwrap_or_else(|| {
                            reg.get(svc_name)
                                .map(|s| s.default_config())
                                .unwrap_or(serde_json::Value::Null)
                        });
                        let req = collection_svc::CollectionRequest {
                            source_text: src,
                            result_text: result,
                            source_lang: String::new(),
                            result_lang: String::new(),
                            config: instance_config,
                        };
                        let collect_result =
                            match crate::core::runtime::block_on(reg.collect(svc_name, req)) {
                                Ok(r) => r,
                                Err(_) => {
                                    log::error!("Collection: shared runtime not available");
                                    return;
                                }
                            };
                        match collect_result {
                            Ok(r) => log::info!("Collection: {}", r.message),
                            Err(e) => log::warn!("Collection failed: {}", e),
                        }
                    });
                });
                header.append(&collect_btn);

                row.append(&header);

                let label = gtk::Label::new(Some(&item.text));
                label.set_halign(gtk::Align::Start);
                label.set_wrap(true);
                label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
                label.set_selectable(true);
                label.set_xalign(0.0);
                row.append(&label);

                let list_row = gtk::ListBoxRow::new();
                list_row.set_child(Some(&row));
                list_row.set_selectable(false);
                list_row.set_activatable(false);
                results_list.append(&list_row);
            }
        }
    }
}

fn clear_results_list(results_list: &gtk::ListBox) {
    while let Some(child) = results_list.last_child() {
        results_list.remove(&child);
    }
}

fn enabled_translate_service_list(config: &Arc<AppConfig>) -> Vec<String> {
    config
        .effective_service_list(ServiceCategory::Translate)
        .into_iter()
        .filter(|instance_key| {
            config
                .get(instance_key)
                .and_then(|v| v.get("enable").and_then(|value| value.as_bool()))
                .unwrap_or(true)
        })
        .collect()
}

async fn translate_instances(
    registry: Arc<translate::TranslateRegistry>,
    service_list: Vec<String>,
    text: String,
    from: String,
    to: String,
    config: Arc<AppConfig>,
) -> Vec<TranslationItem> {
    let mut handles = Vec::new();

    for instance_key in service_list {
        let service_name = instance_key.split('@').next().unwrap_or("").to_string();
        let display_name = display_name_for_instance(&config, &instance_key, &service_name);
        let registry = registry.clone();
        let config = config.clone();
        let text = text.clone();
        let from = from.clone();
        let to = to.clone();

        let handle = tokio::spawn(async move {
            let instance_config = config.get(&instance_key).unwrap_or_else(|| {
                registry
                    .get(&service_name)
                    .map(|service| service.default_config())
                    .unwrap_or(serde_json::Value::Null)
            });
            let req = TranslateRequest {
                text,
                from,
                to,
                config: instance_config,
            };
            let result = registry.translate(&service_name, req).await;
            (
                instance_key,
                service_name,
                display_name,
                translate_result_to_text(result),
            )
        });
        handles.push(handle);
    }

    let mut results = Vec::new();
    for handle in handles {
        match handle.await {
            Ok((instance_key, service_name, display_name, (text, is_error))) => {
                results.push(TranslationItem {
                    instance_key,
                    service_name,
                    display_name,
                    text,
                    is_error,
                });
            }
            Err(e) => {
                log::warn!("Translate task join error: {}", e);
            }
        }
    }
    results
}

fn translate_result_to_text(
    result: Result<TranslateResult, crate::services::types::ServiceError>,
) -> (String, bool) {
    match result {
        Ok(TranslateResult::Text(text)) => (text, false),
        Ok(TranslateResult::Dictionary(dict)) => {
            let dict_text = dict
                .explanations
                .iter()
                .map(|explanation| {
                    format!(
                        "{}: {}",
                        explanation.trait_name,
                        explanation
                            .explains
                            .iter()
                            .map(|item| item.text.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            (
                if dict_text.is_empty() {
                    i18n::t("(dictionary)")
                } else {
                    dict_text
                },
                false,
            )
        }
        Err(error) => (format!("{}: {}", i18n::t("Error"), error.message), true),
    }
}

fn display_name_for_instance(
    config: &Arc<AppConfig>,
    instance_key: &str,
    service_name: &str,
) -> String {
    config
        .get(instance_key)
        .and_then(|value| {
            value
                .get("instanceName")
                .and_then(|name| name.as_str())
                .filter(|name| !name.trim().is_empty())
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| service_display_name(service_name).to_string())
}

fn service_display_name(service_name: &str) -> &str {
    match service_name {
        "openai" => "AI",
        "lingva_tts" => "Lingva TTS",
        other => other,
    }
}

#[cfg(feature = "tts")]
fn play_audio_gstreamer(path: &std::path::Path) {
    use gstreamer::prelude::*;

    if gstreamer::init().is_err() {
        log::warn!("GStreamer init failed, falling back to aplay");
        let _ = std::process::Command::new("aplay").arg(path).status();
        return;
    }

    let uri = format!("file://{}", path.display());
    let playbin = match gstreamer::ElementFactory::make("playbin")
        .property("uri", &uri)
        .build()
    {
        Ok(p) => p,
        Err(_) => {
            let _ = std::process::Command::new("aplay").arg(path).status();
            return;
        }
    };

    playbin.set_state(gstreamer::State::Playing).ok();

    let bus = playbin.bus().unwrap();
    let playbin_clone = playbin.clone();
    std::thread::spawn(move || {
        while let Some(msg) = bus.timed_pop(gstreamer::ClockTime::NONE) {
            use gstreamer::MessageView;
            match msg.view() {
                MessageView::Eos(..) | MessageView::Error(..) => {
                    playbin_clone.set_state(gstreamer::State::Null).ok();
                    return;
                }
                _ => {}
            }
        }
    });
}
