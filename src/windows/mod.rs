pub mod config;
pub mod recognize;
pub mod screenshot;
pub mod service_config;
pub mod translate;
pub mod updater;

pub(crate) fn set_pin_button_state(button: &gtk4::Button, pinned: bool) {
    use gtk4::prelude::*;

    if let Some(img) = button.child().and_downcast::<gtk4::Image>() {
        img.set_icon_name(Some(if pinned {
            "view-pin-symbolic"
        } else {
            "view-pin"
        }));
    }

    button.set_tooltip_text(Some(&crate::i18n::t(if pinned { "Unpin" } else { "Pin" })));
    if pinned {
        button.add_css_class("pin-active");
        button.add_css_class("suggested-action");
    } else {
        button.remove_css_class("pin-active");
        button.remove_css_class("suggested-action");
    }
}
