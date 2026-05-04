use log::info;
#[cfg(feature = "tray")]
use log::warn;
use tokio::sync::mpsc;

use super::http_server::ActionEvent;
#[cfg(feature = "tray")]
use super::http_server::AppAction;
#[cfg(feature = "tray")]
use crate::i18n;

#[cfg(feature = "tray")]
mod ksni_tray {
    use super::*;

    use ksni::Tray;

    pub struct PotTray {
        pub action_tx: mpsc::UnboundedSender<ActionEvent>,
    }

    impl Tray for PotTray {
        fn id(&self) -> String {
            "com.pot-app.desktop".into()
        }

        fn icon_name(&self) -> String {
            "com.pot-app.pot-gtk".into()
        }

        fn title(&self) -> String {
            i18n::t("Pot Desktop")
        }

        fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
            use ksni::menu::*;
            vec![
                StandardItem {
                    label: i18n::t("Selection Translate"),
                    icon_name: "document-edit".into(),
                    activate: Box::new(move |this: &mut Self| {
                        let _ = this
                            .action_tx
                            .send(ActionEvent::new(AppAction::SelectionTranslate));
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: i18n::t("Input Translate"),
                    icon_name: "input-keyboard".into(),
                    activate: Box::new(move |this: &mut Self| {
                        let _ = this
                            .action_tx
                            .send(ActionEvent::new(AppAction::InputTranslate));
                    }),
                    ..Default::default()
                }
                .into(),
                MenuItem::Separator,
                StandardItem {
                    label: i18n::t("OCR Recognize"),
                    icon_name: "document-scanner".into(),
                    activate: Box::new(move |this: &mut Self| {
                        let _ = this
                            .action_tx
                            .send(ActionEvent::new(AppAction::OcrRecognize {
                                screenshot: true,
                            }));
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: i18n::t("OCR Translate"),
                    icon_name: "edit-find-replace".into(),
                    activate: Box::new(move |this: &mut Self| {
                        let _ = this
                            .action_tx
                            .send(ActionEvent::new(AppAction::OcrTranslate {
                                screenshot: true,
                            }));
                    }),
                    ..Default::default()
                }
                .into(),
                MenuItem::Separator,
                StandardItem {
                    label: i18n::t("Settings"),
                    icon_name: "preferences-system".into(),
                    activate: Box::new(move |this: &mut Self| {
                        let _ = this.action_tx.send(ActionEvent::new(AppAction::ShowConfig));
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: i18n::t("Check for Updates"),
                    icon_name: "software-update-available".into(),
                    activate: Box::new(move |this: &mut Self| {
                        let _ = this
                            .action_tx
                            .send(ActionEvent::new(AppAction::ShowUpdater));
                    }),
                    ..Default::default()
                }
                .into(),
                MenuItem::Separator,
                StandardItem {
                    label: i18n::t("Quit"),
                    icon_name: "application-exit".into(),
                    activate: Box::new(move |this: &mut Self| {
                        let _ = this.action_tx.send(ActionEvent::new(AppAction::Quit));
                    }),
                    ..Default::default()
                }
                .into(),
            ]
        }
    }
}

#[cfg(feature = "tray")]
mod gtk_fallback {
    use gtk4::prelude::*;

    use super::*;

    pub struct GtkFallbackTray {
        pub menu_window: gtk4::Window,
        action_tx: mpsc::UnboundedSender<ActionEvent>,
    }

    impl GtkFallbackTray {
        pub fn new(action_tx: mpsc::UnboundedSender<ActionEvent>) -> Self {
            let menu_window = gtk4::Window::builder()
                .title("Pot")
                .decorated(false)
                .resizable(false)
                .default_width(220)
                .build();
            menu_window.add_css_class("tray-menu");

            let vbox = build_menu_box(&action_tx);
            menu_window.set_child(Some(&vbox));
            menu_window.set_hide_on_close(true);

            let focus = gtk4::EventControllerFocus::new();
            let win_clone = menu_window.clone();
            focus.connect_leave(move |_| {
                win_clone.hide();
            });
            menu_window.add_controller(focus);

            Self {
                menu_window,
                action_tx,
            }
        }

        pub fn show_menu(&self) {
            self.menu_window.present();
        }

        pub fn hide_menu(&self) {
            self.menu_window.hide();
        }

        pub fn refresh_language(&self) {
            let vbox = build_menu_box(&self.action_tx);
            self.menu_window.set_child(Some(&vbox));
        }
    }

    fn build_menu_box(action_tx: &mpsc::UnboundedSender<ActionEvent>) -> gtk4::Box {
        let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        vbox.set_margin_top(6);
        vbox.set_margin_bottom(6);
        vbox.set_margin_start(6);
        vbox.set_margin_end(6);
        vbox.set_spacing(2);

        append_item(&vbox, "document-edit", &i18n::t("Selection Translate"), {
            let tx = action_tx.clone();
            move || {
                let _ = tx.send(ActionEvent::new(AppAction::SelectionTranslate));
            }
        });
        append_item(&vbox, "input-keyboard", &i18n::t("Input Translate"), {
            let tx = action_tx.clone();
            move || {
                let _ = tx.send(ActionEvent::new(AppAction::InputTranslate));
            }
        });
        append_sep(&vbox);
        append_item(&vbox, "document-scanner", &i18n::t("OCR Recognize"), {
            let tx = action_tx.clone();
            move || {
                let _ = tx.send(ActionEvent::new(AppAction::OcrRecognize {
                    screenshot: true,
                }));
            }
        });
        append_item(&vbox, "edit-find-replace", &i18n::t("OCR Translate"), {
            let tx = action_tx.clone();
            move || {
                let _ = tx.send(ActionEvent::new(AppAction::OcrTranslate {
                    screenshot: true,
                }));
            }
        });
        append_sep(&vbox);
        append_item(&vbox, "preferences-system", &i18n::t("Settings"), {
            let tx = action_tx.clone();
            move || {
                let _ = tx.send(ActionEvent::new(AppAction::ShowConfig));
            }
        });
        append_item(
            &vbox,
            "software-update-available",
            &i18n::t("Check for Updates"),
            {
                let tx = action_tx.clone();
                move || {
                    let _ = tx.send(ActionEvent::new(AppAction::ShowUpdater));
                }
            },
        );
        append_sep(&vbox);
        append_item(&vbox, "application-exit", &i18n::t("Quit"), {
            let tx = action_tx.clone();
            move || {
                let _ = tx.send(ActionEvent::new(AppAction::Quit));
            }
        });

        vbox
    }

    fn append_item<F: Fn() + 'static>(parent: &gtk4::Box, icon: &str, label: &str, cb: F) {
        let btn = gtk4::Button::new();
        btn.add_css_class("flat");
        let inner = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        inner.append(&gtk4::Image::from_icon_name(icon));
        let lbl = gtk4::Label::new(Some(label));
        lbl.set_hexpand(true);
        lbl.set_xalign(0.0);
        inner.append(&lbl);
        btn.set_child(Some(&inner));
        btn.connect_clicked(move |_| cb());
        parent.append(&btn);
    }

    fn append_sep(parent: &gtk4::Box) {
        let sep = gtk4::Separator::new(gtk4::Orientation::Horizontal);
        sep.set_margin_top(4);
        sep.set_margin_bottom(4);
        parent.append(&sep);
    }
}

pub struct SystemTray {
    #[cfg(feature = "tray")]
    #[allow(dead_code)]
    ksni_handle: Option<ksni::Handle<ksni_tray::PotTray>>,
    #[cfg(feature = "tray")]
    fallback: Option<gtk_fallback::GtkFallbackTray>,
    #[allow(dead_code)]
    action_tx: mpsc::UnboundedSender<ActionEvent>,
}

impl SystemTray {
    pub fn new(action_tx: mpsc::UnboundedSender<ActionEvent>) -> Self {
        #[cfg(feature = "tray")]
        {
            let tray = ksni_tray::PotTray {
                action_tx: action_tx.clone(),
            };

            let handle = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    use ksni::TrayMethods;
                    tray.spawn().await.ok()
                })
            });

            let fallback = if handle.is_some() {
                info!("Using ksni system tray");
                None
            } else {
                warn!("ksni unavailable, falling back to GTK window tray");
                Some(gtk_fallback::GtkFallbackTray::new(action_tx.clone()))
            };

            Self {
                ksni_handle: handle,
                fallback,
                action_tx,
            }
        }

        #[cfg(not(feature = "tray"))]
        {
            info!("System tray disabled (tray feature not enabled)");
            Self { action_tx }
        }
    }

    #[allow(dead_code)]
    pub fn show_menu(&self) {
        #[cfg(feature = "tray")]
        if let Some(ref fb) = self.fallback {
            fb.show_menu();
        }
    }

    #[allow(dead_code)]
    pub fn hide_menu(&self) {
        #[cfg(feature = "tray")]
        if let Some(ref fb) = self.fallback {
            fb.hide_menu();
        }
    }

    #[allow(dead_code)]
    pub fn update_language(&self, language: &str) {
        crate::i18n::set_language(language);

        #[cfg(feature = "tray")]
        {
            if let Some(ref fb) = self.fallback {
                fb.refresh_language();
            }

            if let Some(handle) = self.ksni_handle.clone() {
                if let Some(runtime) = crate::core::runtime::handle() {
                    runtime.spawn(async move {
                        let _ = handle.update(|_| {}).await;
                    });
                }
            }
        }
    }
    #[allow(dead_code)]
    pub fn set_clipboard_monitor(&self, _enabled: bool) {}
    #[allow(dead_code)]
    pub fn set_copy_mode(&self, _mode: &str) {}
}
