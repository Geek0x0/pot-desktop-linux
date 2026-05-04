use crate::core::image_utils;
use crate::core::screenshot::{self, CaptureInfo};
use gtk::prelude::*;
use log::warn;
use relm4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Shared selection rect state accessible from both the draw func and update.
type SelectionState = Rc<RefCell<Option<(f64, f64, f64, f64)>>>;

/// Find the GDK monitor that contains the mouse cursor.
/// On Wayland, mouse_position may return unreliable coordinates,
/// so we fall back to the surface's current monitor.
fn find_mouse_monitor(root: &gtk::Window) -> Option<gtk::gdk::Monitor> {
    let display = gtk::gdk::Display::default()?;

    // Try to match by mouse position (works reliably on X11)
    if let mouse_position::mouse_position::Mouse::Position { x, y } =
        mouse_position::mouse_position::Mouse::get_mouse_position()
    {
        let monitors = display.monitors();
        for i in 0..monitors.n_items() {
            if let Some(monitor) = monitors.item(i).and_downcast::<gtk::gdk::Monitor>() {
                let geom = monitor.geometry();
                if x >= geom.x()
                    && x < geom.x() + geom.width()
                    && y >= geom.y()
                    && y < geom.y() + geom.height()
                {
                    return Some(monitor);
                }
            }
        }
    }

    // Fallback: use the surface's current monitor (works on Wayland after present)
    if let Some(surface) = root.surface() {
        if let Some(monitor) = display.monitor_at_surface(&surface) {
            return Some(monitor);
        }
    }

    None
}

pub struct ScreenshotModel {
    selecting: bool,
    start_x: f64,
    start_y: f64,
    current_x: f64,
    current_y: f64,
    sel_state: SelectionState,
    capture: Option<CaptureInfo>,
    image_view: gtk::Picture,
    drawing_area: gtk::DrawingArea,
}

#[derive(Debug)]
pub enum ScreenshotMsg {
    StartCapture(i32, i32),
    MouseDown(f64, f64),
    MouseMove(f64, f64),
    MouseUp(f64, f64),
    Cancel,
}

#[derive(Debug, Clone)]
pub enum ScreenshotOutput {
    Captured,
    Cancelled,
}

#[relm4::component(pub)]
impl Component for ScreenshotModel {
    type Init = ();
    type Input = ScreenshotMsg;
    type Output = ScreenshotOutput;
    type CommandOutput = ();

    view! {
        gtk::Window {
            set_decorated: false,
            set_icon_name: Some("com.pot-app.pot-gtk"),

            connect_close_request[sender] => move |_| {
                let _ = sender.output(ScreenshotOutput::Cancelled);
                gtk::glib::Propagation::Stop
            },

        }
    }

    fn init(_init: (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let sel: SelectionState = Rc::new(RefCell::new(None));

        let widgets = view_output!();

        let overlay = gtk::Overlay::new();
        overlay.set_hexpand(true);
        overlay.set_vexpand(true);

        let image_view = gtk::Picture::new();
        image_view.set_hexpand(true);
        image_view.set_vexpand(true);
        image_view.set_can_shrink(false);
        image_view.set_content_fit(gtk::ContentFit::Fill);
        overlay.set_child(Some(&image_view));

        let drawing_area = gtk::DrawingArea::new();
        drawing_area.set_hexpand(true);
        drawing_area.set_vexpand(true);
        overlay.add_overlay(&drawing_area);
        root.set_child(Some(&overlay));

        // Drawing function reads from shared selection state
        let sel_draw = sel.clone();
        drawing_area.set_draw_func(move |_area, cr, _w, _h| {
            // Dark overlay over the screenshot preview.
            cr.set_source_rgba(0.0, 0.0, 0.0, 0.4);
            let _ = cr.paint();

            if let Some((x1, y1, x2, y2)) = *sel_draw.borrow() {
                let sx = x1.min(x2);
                let sy = y1.min(y2);
                let sw = (x2 - x1).abs();
                let sh = (y2 - y1).abs();

                // Reveal the screenshot under the selected area.
                cr.save().ok();
                cr.set_operator(gtk::cairo::Operator::Clear);
                cr.rectangle(sx, sy, sw, sh);
                cr.fill().ok();
                cr.restore().ok();

                cr.set_source_rgb(0.13, 0.59, 0.95);
                cr.set_line_width(2.0);
                cr.rectangle(sx, sy, sw, sh);
                let _ = cr.stroke();
            }
        });

        let model = ScreenshotModel {
            selecting: false,
            start_x: 0.0,
            start_y: 0.0,
            current_x: 0.0,
            current_y: 0.0,
            sel_state: sel.clone(),
            capture: None,
            image_view,
            drawing_area,
        };

        // Click gesture
        let click = gtk::GestureClick::new();
        let sender_click = sender.input_sender().clone();
        click.connect_pressed(move |_gesture, _n_press, x, y| {
            let _ = sender_click.send(ScreenshotMsg::MouseDown(x, y));
        });
        let sender_release = sender.input_sender().clone();
        click.connect_released(move |_gesture, _n_press, x, y| {
            let _ = sender_release.send(ScreenshotMsg::MouseUp(x, y));
        });
        let sender_cancel = sender.input_sender().clone();
        click.connect_stopped(move |gesture| {
            if gesture.current_button() == gtk::gdk::BUTTON_SECONDARY {
                let _ = sender_cancel.send(ScreenshotMsg::Cancel);
            }
        });
        root.add_controller(click);

        // Motion tracking
        let motion = gtk::EventControllerMotion::new();
        let sender_motion = sender.input_sender().clone();
        motion.connect_motion(move |_controller, x, y| {
            let _ = sender_motion.send(ScreenshotMsg::MouseMove(x, y));
        });
        root.add_controller(motion);

        // Escape key cancels
        let key_controller = gtk::EventControllerKey::new();
        let sender_key = sender.input_sender().clone();
        key_controller.connect_key_pressed(move |_controller, keyval, _keycode, _state| {
            if keyval == gtk::gdk::Key::Escape {
                let _ = sender_key.send(ScreenshotMsg::Cancel);
                gtk::glib::Propagation::Stop
            } else {
                gtk::glib::Propagation::Proceed
            }
        });
        root.add_controller(key_controller);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, root: &Self::Root) {
        match msg {
            ScreenshotMsg::StartCapture(_x, _y) => {
                self.selecting = false;
                *self.sel_state.borrow_mut() = None;

                if screenshot::is_wayland_session() {
                    match screenshot::capture_interactive_selection() {
                        Ok(_capture) => {
                            root.set_visible(false);
                            let _ = sender.output(ScreenshotOutput::Captured);
                        }
                        Err(e) => {
                            warn!("Wayland screenshot failed: {}", e);
                            root.set_visible(false);
                            let _ = sender.output(ScreenshotOutput::Cancelled);
                        }
                    }
                    return;
                }

                match screenshot::capture_interactive_selection() {
                    Ok(_capture) => {
                        root.set_visible(false);
                        let _ = sender.output(ScreenshotOutput::Captured);
                        return;
                    }
                    Err(e) => {
                        warn!(
                            "Native/portal region screenshot failed, falling back to GTK overlay: {}",
                            e
                        );
                    }
                }

                match screenshot::capture_all_screens() {
                    Ok(capture) => {
                        if let Ok(texture) =
                            gtk::gdk::Texture::from_file(&gtk::gio::File::for_path(&capture.path))
                        {
                            self.image_view.set_paintable(Some(&texture));
                        } else {
                            warn!("Failed to load screenshot preview: {:?}", capture.path);
                        }
                        self.capture = Some(capture);
                    }
                    Err(e) => {
                        warn!("Screenshot capture failed: {}", e);
                        root.set_visible(false);
                        let _ = sender.output(ScreenshotOutput::Cancelled);
                        return;
                    }
                }

                // Find the monitor under the mouse and fullscreen on it
                if let Some(monitor) = find_mouse_monitor(&root) {
                    root.fullscreen_on_monitor(&monitor);
                } else {
                    root.fullscreen();
                }
                root.present();
            }
            ScreenshotMsg::MouseDown(x, y) => {
                self.selecting = true;
                self.start_x = x;
                self.start_y = y;
                self.current_x = x;
                self.current_y = y;
            }
            ScreenshotMsg::MouseMove(x, y) => {
                if self.selecting {
                    self.current_x = x;
                    self.current_y = y;
                }
            }
            ScreenshotMsg::MouseUp(x, y) => {
                if !self.selecting {
                    return;
                }
                self.selecting = false;
                *self.sel_state.borrow_mut() = None;

                let x1 = self.start_x.min(x) as u32;
                let y1 = self.start_y.min(y) as u32;
                let x2 = self.start_x.max(x) as u32;
                let y2 = self.start_y.max(y) as u32;
                let w = x2 - x1;
                let h = y2 - y1;

                root.unfullscreen();
                root.set_visible(false);

                if w > 5 && h > 5 {
                    let result = if let Some(capture) = &self.capture {
                        image_utils::cut_image_scaled(
                            x1 as f64,
                            y1 as f64,
                            w as f64,
                            h as f64,
                            self.drawing_area.width(),
                            self.drawing_area.height(),
                            capture.width,
                            capture.height,
                        )
                    } else {
                        image_utils::cut_image(x1, y1, w, h)
                    };

                    match result {
                        Ok(()) => {
                            let _ = sender.output(ScreenshotOutput::Captured);
                        }
                        Err(e) => {
                            warn!("Failed to crop screenshot: {}", e);
                            let _ = sender.output(ScreenshotOutput::Cancelled);
                        }
                    }
                } else {
                    let _ = sender.output(ScreenshotOutput::Cancelled);
                }
            }
            ScreenshotMsg::Cancel => {
                self.selecting = false;
                *self.sel_state.borrow_mut() = None;
                root.unfullscreen();
                root.set_visible(false);
                let _ = sender.output(ScreenshotOutput::Cancelled);
            }
        }
    }

    fn post_view() {
        // Update shared selection state from model
        if model.selecting {
            *model.sel_state.borrow_mut() = Some((
                model.start_x,
                model.start_y,
                model.current_x,
                model.current_y,
            ));
        } else {
            *model.sel_state.borrow_mut() = None;
        }
        model.drawing_area.queue_draw();
    }
}
