use gtk::prelude::*;
use relm4::prelude::*;

use crate::i18n;

const UPDATE_REPO: &str = "Geek0x0/pot-desktop-linux";
const UPDATE_API_LATEST: &str =
    "https://api.github.com/repos/Geek0x0/pot-desktop-linux/releases/latest";
const UPDATE_RELEASES_URL: &str = "https://github.com/Geek0x0/pot-desktop-linux/releases";

pub struct UpdaterModel {
    checking: bool,
    checked: bool,
    check_failed: bool,
    update_available: bool,
    current_version: String,
    latest_version: String,
    release_notes: String,
    release_url: String,
}

#[derive(Debug)]
pub enum UpdaterMsg {
    #[allow(dead_code)]
    Show,
    CheckUpdate,
    InstallUpdate,
}

#[derive(Debug)]
pub struct UpdateCheckResult {
    pub available: bool,
    pub latest: String,
    pub notes: String,
    pub release_url: String,
    pub failed: bool,
}

#[relm4::component(pub)]
impl Component for UpdaterModel {
    type Init = ();
    type Input = UpdaterMsg;
    type Output = ();
    type CommandOutput = UpdateCheckResult;

    view! {
        gtk::Window {
            set_title: Some(&i18n::t("Pot - Updater")),
            set_default_width: 640,
            set_default_height: 460,
            set_hide_on_close: true,
            set_icon_name: Some("com.pot-app.pot-gtk"),

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 14,
                set_margin_start: 18,
                set_margin_end: 18,
                set_margin_top: 18,
                set_margin_bottom: 18,

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 4,

                    gtk::Label {
                        set_label: &i18n::t("Update"),
                        add_css_class: "title-2",
                        set_halign: gtk::Align::Start,
                    },

                    gtk::Label {
                        set_label: UPDATE_REPO,
                        add_css_class: "dim-label",
                        set_halign: gtk::Align::Start,
                    },
                },

                #[name = "status_label"]
                gtk::Label {
                    set_label: &format!("{}: {}", i18n::t("Current version"), env!("CARGO_PKG_VERSION")),
                    set_halign: gtk::Align::Start,
                    set_wrap: true,
                },

                #[name = "spinner"]
                gtk::Spinner {
                    set_visible: false,
                },

                #[name = "notes_scroll"]
                gtk::ScrolledWindow {
                    set_vexpand: true,
                    set_hscrollbar_policy: gtk::PolicyType::Never,
                    set_visible: false,
                    add_css_class: "updater-notes",

                    #[name = "notes_view"]
                    gtk::TextView {
                        set_wrap_mode: gtk::WrapMode::WordChar,
                        set_editable: false,
                        set_top_margin: 8,
                        set_left_margin: 8,
                        set_right_margin: 8,
                        set_bottom_margin: 8,
                    },
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 8,
                    set_halign: gtk::Align::End,

                    #[name = "check_btn"]
                    gtk::Button {
                        set_label: &i18n::t("Check for Updates"),
                        connect_clicked => UpdaterMsg::CheckUpdate,
                    },

                    #[name = "update_btn"]
                    gtk::Button {
                        set_label: &i18n::t("Open Release Page"),
                        add_css_class: "suggested-action",
                        set_visible: false,
                        connect_clicked => UpdaterMsg::InstallUpdate,
                    },
                },
            },
        }
    }

    fn init(_init: (), root: Self::Root, _sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let model = UpdaterModel {
            checking: false,
            checked: false,
            check_failed: false,
            update_available: false,
            current_version: env!("CARGO_PKG_VERSION").to_string(),
            latest_version: String::new(),
            release_notes: String::new(),
            release_url: UPDATE_RELEASES_URL.into(),
        };

        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            UpdaterMsg::Show => {
                let _ = sender.input_sender().send(UpdaterMsg::CheckUpdate);
            }
            UpdaterMsg::CheckUpdate => {
                self.checking = true;
                self.checked = false;
                self.check_failed = false;
                self.update_available = false;
                self.latest_version.clear();
                self.release_notes.clear();
                self.release_url = UPDATE_RELEASES_URL.into();

                let current = self.current_version.clone();
                sender.spawn_command(move |out_sender| {
                    let output =
                        match crate::core::runtime::block_on(check_latest_release(&current)) {
                            Ok(result) => result,
                            Err(_) => {
                                log::error!("Updater: shared runtime not available");
                                update_error(i18n::t("Check failed"))
                            }
                        };
                    let _ = out_sender.send(output);
                });
            }
            UpdaterMsg::InstallUpdate => {
                let url = if self.release_url.trim().is_empty() {
                    UPDATE_RELEASES_URL
                } else {
                    &self.release_url
                };
                if let Err(e) = gtk::gio::AppInfo::launch_default_for_uri(
                    url,
                    None::<&gtk::gio::AppLaunchContext>,
                ) {
                    log::warn!("Failed to open release page: {}", e);
                }
            }
        }
    }

    fn update_cmd(
        &mut self,
        output: Self::CommandOutput,
        _sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        self.checking = false;
        self.checked = true;
        self.check_failed = output.failed;
        self.update_available = output.available;
        self.latest_version = output.latest;
        self.release_notes = output.notes;
        self.release_url = output.release_url;
    }

    fn post_view() {
        if model.checking {
            spinner.set_visible(true);
            spinner.start();
            status_label.set_label(&i18n::t("Checking for updates..."));
            check_btn.set_sensitive(false);
            update_btn.set_visible(false);
            notes_scroll.set_visible(false);
            return;
        }

        spinner.set_visible(false);
        spinner.stop();
        check_btn.set_sensitive(true);

        if model.check_failed {
            status_label.set_label(&model.release_notes);
            update_btn.set_visible(true);
            notes_scroll.set_visible(false);
        } else if model.update_available {
            status_label.set_label(&format!(
                "{}: {} -> {}",
                i18n::t("Update available"),
                model.current_version,
                model.latest_version
            ));
            update_btn.set_visible(true);
            notes_scroll.set_visible(true);
            notes_view.buffer().set_text(&model.release_notes);
        } else if model.checked {
            status_label.set_label(&format!(
                "{} (v{})",
                i18n::t("You are up to date"),
                model.current_version
            ));
            update_btn.set_visible(false);
            notes_scroll.set_visible(false);
        } else {
            status_label.set_label(&format!(
                "{}: {}",
                i18n::t("Current version"),
                model.current_version
            ));
            update_btn.set_visible(false);
            notes_scroll.set_visible(false);
        }
    }
}

async fn check_latest_release(current: &str) -> UpdateCheckResult {
    let client = match reqwest::Client::builder()
        .user_agent("pot-gtk-updater")
        .timeout(std::time::Duration::from_secs(12))
        .build()
    {
        Ok(client) => client,
        Err(e) => return update_error(format!("{}: {}", i18n::t("Check failed"), e)),
    };

    let response = match client
        .get(UPDATE_API_LATEST)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
    {
        Ok(response) => response,
        Err(e) => return update_error(format!("{}: {}", i18n::t("Check failed"), e)),
    };

    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return UpdateCheckResult {
            available: false,
            latest: String::new(),
            notes: i18n::t("No release found."),
            release_url: UPDATE_RELEASES_URL.into(),
            failed: true,
        };
    }
    if !status.is_success() {
        return update_error(format!(
            "{}: HTTP {}",
            i18n::t("Check failed"),
            status.as_u16()
        ));
    }

    let json = match response.json::<serde_json::Value>().await {
        Ok(json) => json,
        Err(e) => return update_error(format!("{}: {}", i18n::t("Check failed"), e)),
    };

    release_result_from_json(current, &json)
}

fn update_error(message: String) -> UpdateCheckResult {
    UpdateCheckResult {
        available: false,
        latest: String::new(),
        notes: message,
        release_url: UPDATE_RELEASES_URL.into(),
        failed: true,
    }
}

fn release_result_from_json(current: &str, json: &serde_json::Value) -> UpdateCheckResult {
    let tag = json
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let latest = normalize_version(tag);
    if latest.is_empty() {
        return update_error(i18n::t("No release found."));
    }

    let body = json
        .get("body")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    let notes = if body.is_empty() {
        i18n::t("No release notes.")
    } else {
        body.to_string()
    };

    let release_url = json
        .get("html_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(UPDATE_RELEASES_URL)
        .to_string();

    UpdateCheckResult {
        available: is_newer_version(&latest, current),
        latest,
        notes,
        release_url,
        failed: false,
    }
}

fn normalize_version(version: &str) -> String {
    version
        .trim()
        .trim_start_matches(['v', 'V'])
        .trim()
        .to_string()
}

fn is_newer_version(latest: &str, current: &str) -> bool {
    let latest_parts = version_parts(latest);
    let current_parts = version_parts(current);
    for i in 0..latest_parts.len().max(current_parts.len()).max(3) {
        let latest_part = latest_parts.get(i).copied().unwrap_or(0);
        let current_part = current_parts.get(i).copied().unwrap_or(0);
        if latest_part > current_part {
            return true;
        }
        if latest_part < current_part {
            return false;
        }
    }
    false
}

fn version_parts(version: &str) -> Vec<u64> {
    normalize_version(version)
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u64>().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions_numerically() {
        assert!(is_newer_version("0.10.0", "0.9.9"));
        assert!(is_newer_version("v1.0.1", "1.0.0"));
        assert!(!is_newer_version("0.1.0", "0.1.0"));
        assert!(!is_newer_version("0.1.0", "0.2.0"));
    }

    #[test]
    fn parses_github_release_json() {
        let json = serde_json::json!({
            "tag_name": "v0.2.0",
            "html_url": "https://github.com/Geek0x0/pot-desktop-linux/releases/tag/v0.2.0",
            "body": "hello"
        });
        let result = release_result_from_json("0.1.0", &json);
        assert!(result.available);
        assert_eq!(result.latest, "0.2.0");
        assert_eq!(result.notes, "hello");
    }
}
