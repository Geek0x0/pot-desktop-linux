use crate::core::clipboard;
use crate::i18n;
use crate::services::recognize;
use gtk::prelude::*;
use relm4::prelude::*;
use std::sync::Arc;

pub struct RecognizeModel {
    recognized_text: String,
    loading: bool,
    pinned: bool,
    registry: Arc<recognize::RecognizeRegistry>,
    ocr_service: String,
    #[allow(dead_code)]
    config: Arc<crate::config::AppConfig>,
    #[allow(dead_code)]
    ocr_service_names: Vec<String>,
    #[allow(dead_code)]
    ocr_dropdown: gtk::DropDown,
    image_view: Option<gtk::Picture>,
}

#[derive(Debug)]
pub enum RecognizeMsg {
    Show,
    Recognize,
    SetRecognizedText(String),
    CopyText,
    TranslateText,
    Pin,
    Close,
    SetOcrService(String),
}

#[derive(Debug)]
pub enum RecognizeOutput {
    TranslateText(String),
    Closed,
}

#[derive(Debug)]
pub struct OcrCommandOutput {
    pub text: String,
    #[allow(dead_code)]
    pub error: bool,
}

#[relm4::component(pub)]
impl Component for RecognizeModel {
    type Init = (
        Arc<recognize::RecognizeRegistry>,
        Arc<crate::config::AppConfig>,
    );
    type Input = RecognizeMsg;
    type Output = RecognizeOutput;
    type CommandOutput = OcrCommandOutput;

    view! {
        gtk::Window {
            set_title: Some(&i18n::t("Pot - OCR")),
            set_default_width: 800,
            set_default_height: 400,
            set_icon_name: Some("com.pot-app.pot-gtk"),

            connect_close_request[sender] => move |_| {
                let _ = sender.output(RecognizeOutput::Closed);
                gtk::glib::Propagation::Stop
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 4,

                // Title bar
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_margin_start: 8,
                    set_margin_end: 8,
                    set_margin_top: 4,
                    set_margin_bottom: 4,

                    gtk::Label {
                        set_label: &i18n::t("OCR Recognize"),
                        set_hexpand: true,
                        set_halign: gtk::Align::Start,
                        add_css_class: "title-4",
                    },

                    #[name = "pin_btn"]
                    gtk::Button {
                        set_icon_name: "view-pin",
                        set_tooltip_text: Some(&i18n::t("Pin")),
                        connect_clicked => RecognizeMsg::Pin,
                    },

                    gtk::Button {
                        set_icon_name: "window-close",
                        set_tooltip_text: Some(&i18n::t("Close")),
                        connect_clicked => RecognizeMsg::Close,
                    },
                },

                // Content: two columns
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 4,
                    set_vexpand: true,
                    set_margin_start: 8,
                    set_margin_end: 8,

                    // Left: image preview
                    gtk::ScrolledWindow {
                        set_hscrollbar_policy: gtk::PolicyType::Automatic,
                        set_vscrollbar_policy: gtk::PolicyType::Automatic,
                        set_hexpand: true,

                        #[name = "image_view"]
                        gtk::Picture {
                            set_hexpand: true,
                            set_vexpand: true,
                        },
                    },

                    // Right: recognized text
                    gtk::ScrolledWindow {
                        set_hscrollbar_policy: gtk::PolicyType::Automatic,
                        set_vscrollbar_policy: gtk::PolicyType::Automatic,
                        set_hexpand: true,

                        #[name = "text_view"]
                        gtk::TextView {
                            set_wrap_mode: gtk::WrapMode::WordChar,
                            set_top_margin: 4,
                            set_left_margin: 8,
                            set_right_margin: 8,
                            set_bottom_margin: 4,
                            set_editable: true,
                        },
                    },
                },

                // Bottom bar
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 4,
                    set_margin_start: 8,
                    set_margin_end: 8,
                    set_margin_bottom: 4,

                    #[name = "ocr_combo"]
                    gtk::DropDown {
                        set_tooltip_text: Some(&i18n::t("OCR Service")),
                    },

                    gtk::Button {
                        set_icon_name: "edit-copy",
                        set_tooltip_text: Some(&i18n::t("Copy text")),
                        connect_clicked => RecognizeMsg::CopyText,
                    },

                    gtk::Button {
                        set_label: &i18n::t("Recognize"),
                        add_css_class: "suggested-action",
                        connect_clicked => RecognizeMsg::Recognize,
                    },

                    gtk::Button {
                        set_label: &i18n::t("Translate"),
                        connect_clicked => RecognizeMsg::TranslateText,
                    },

                    #[name = "spinner"]
                    gtk::Spinner {
                        set_visible: false,
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
        let (registry, config) = init;

        let ocr_service = config
            .get("recognize_service_list")
            .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
            .and_then(|list| list.first().cloned())
            .unwrap_or_else(|| "tesseract".into());

        // Build OCR service dropdown before creating the model
        let services: Vec<String> = registry.list().iter().map(|s| s.to_string()).collect();
        let svc_str_refs: Vec<&str> = services.iter().map(|s| s.as_str()).collect();
        let svc_list = gtk::StringList::new(&svc_str_refs);
        let ocr_dropdown = gtk::DropDown::new(
            Some(svc_list.upcast::<gtk::gio::ListModel>()),
            gtk::Expression::NONE,
        );
        ocr_dropdown.set_tooltip_text(Some(&i18n::t("OCR Service")));

        if let Some(idx) = services.iter().position(|s| *s == ocr_service) {
            ocr_dropdown.set_selected(idx as u32);
        }

        let mut model = RecognizeModel {
            recognized_text: String::new(),
            loading: false,
            pinned: false,
            registry,
            ocr_service,
            config,
            ocr_service_names: services.clone(),
            ocr_dropdown: ocr_dropdown.clone(),
            image_view: None,
        };

        let widgets = view_output!();
        model.image_view = Some(widgets.image_view.clone());

        // Replace placeholder in view
        let parent = widgets.ocr_combo.parent().unwrap();
        let parent_box = parent.downcast::<gtk::Box>().unwrap();
        let pos = {
            let children = parent_box.observe_children();
            let mut p = 0u32;
            for i in 0..children.n_items() {
                if let Some(child) = children.item(i) {
                    if child == widgets.ocr_combo {
                        p = i;
                        break;
                    }
                }
            }
            p
        };
        parent_box.remove(&widgets.ocr_combo);
        let sibling = if pos > 0 {
            let children = parent_box.observe_children();
            children.item(pos - 1).and_downcast::<gtk::Widget>()
        } else {
            None
        };
        parent_box.insert_child_after(&ocr_dropdown, sibling.as_ref());

        let sender_c = sender.input_sender().clone();
        let services_clone = services.clone();
        ocr_dropdown.connect_selected_notify(move |dd| {
            let idx = dd.selected() as usize;
            if idx < services_clone.len() {
                let _ = sender_c.send(RecognizeMsg::SetOcrService(services_clone[idx].clone()));
            }
        });

        let text_sender = sender.input_sender().clone();
        widgets.text_view.buffer().connect_changed(move |buf| {
            let text = buf.text(&buf.start_iter(), &buf.end_iter(), false);
            let _ = text_sender.send(RecognizeMsg::SetRecognizedText(text.to_string()));
        });

        load_cut_image(&widgets.image_view);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            RecognizeMsg::Show => {
                if let Some(image_view) = &self.image_view {
                    load_cut_image(image_view);
                }
                self.loading = true;
                let _ = sender.input_sender().send(RecognizeMsg::Recognize);
            }
            RecognizeMsg::Recognize => {
                self.loading = true;
                let registry = self.registry.clone();
                let ocr_service = self.ocr_service.clone();
                sender.spawn_command(move |out_sender| {
                    let base64 = match crate::core::image_utils::get_base64() {
                        Ok(b) if !b.is_empty() => b,
                        Ok(_) => {
                            let _ = out_sender.send(OcrCommandOutput {
                                text: i18n::t("No image to recognize."),
                                error: true,
                            });
                            return;
                        }
                        Err(e) => {
                            let _ = out_sender.send(OcrCommandOutput {
                                text: format!("{}: {}", i18n::t("Image read error"), e),
                                error: true,
                            });
                            return;
                        }
                    };

                    let req = recognize::RecognizeRequest {
                        image_base64: base64,
                        language: "auto".into(),
                        config: serde_json::Value::Null,
                    };

                    let text = match registry.recognize(&ocr_service, req) {
                        Ok(result) => result.text,
                        Err(e) => format!("{}: {}", i18n::t("OCR error"), e),
                    };

                    let _ = out_sender.send(OcrCommandOutput { text, error: false });
                });
            }
            RecognizeMsg::SetRecognizedText(text) => {
                self.recognized_text = text;
            }
            RecognizeMsg::CopyText => {
                clipboard::ClipboardMonitor::write_text(&self.recognized_text);
            }
            RecognizeMsg::TranslateText => {
                let text = self.recognized_text.clone();
                let _ = sender.output(RecognizeOutput::TranslateText(text));
            }
            RecognizeMsg::Pin => {
                self.pinned = !self.pinned;
            }
            RecognizeMsg::Close => {
                let _ = sender.output(RecognizeOutput::Closed);
            }
            RecognizeMsg::SetOcrService(svc) => {
                self.ocr_service = svc;
            }
        }
    }

    fn update_cmd(
        &mut self,
        output: Self::CommandOutput,
        _sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        self.loading = false;
        self.recognized_text = output.text;
    }

    fn post_view() {
        crate::windows::set_pin_button_state(&pin_btn, model.pinned);

        // Update text view
        let buf = text_view.buffer();
        let current = buf
            .text(&buf.start_iter(), &buf.end_iter(), false)
            .to_string();
        if current != model.recognized_text {
            buf.set_text(&model.recognized_text);
        }

        // Spinner
        spinner.set_visible(model.loading);
        if model.loading {
            spinner.start();
        } else {
            spinner.stop();
        }
    }
}

fn load_cut_image(image_view: &gtk::Picture) {
    let cut_path = dirs::cache_dir()
        .map(|d| d.join(crate::config::APP_ID).join("pot_screenshot_cut.png"))
        .unwrap_or_default();
    if cut_path.exists() {
        if let Ok(texture) = gtk::gdk::Texture::from_file(&gtk::gio::File::for_path(&cut_path)) {
            image_view.set_paintable(Some(&texture));
        }
    }
}
