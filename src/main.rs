mod backend;
mod state;
mod ui;

use gtk::gio;
use gtk::prelude::*;
use gtk::Application;
use state::SharedState;
use std::sync::{Arc, Mutex};
use ksni::{Tray, TrayService, menu::{MenuItem, StandardItem}, Icon};
use libc::{SIGINT, SIGTERM, signal};

extern "C" fn handle_exit_signal(_: libc::c_int) {
    backend::cleanup_all_limiters();
    std::process::exit(0);
}

struct LBSTray {
    state: SharedState,
}

impl Tray for LBSTray {
    fn icon_name(&self) -> String {
        "".into()
    }
    fn icon_pixmap(&self) -> Vec<Icon> {
        let is_active = if let Ok(s) = self.state.lock() {
            !s.targets.is_empty()
        } else {
            false
        };

        let icon_data: &[u8] = if is_active {
            include_bytes!("../assets/active.png")
        } else {
            include_bytes!("../assets/idle.png")
        };

        if let Ok(img) = image::load_from_memory(icon_data) {
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            let mut argb = Vec::with_capacity((width * height * 4) as usize);
            for chunk in rgba.chunks_exact(4) {
                argb.push(chunk[3]); 
                argb.push(chunk[0]); 
                argb.push(chunk[1]); 
                argb.push(chunk[2]); 
            }
            return vec![Icon {
                width: width as i32,
                height: height as i32,
                data: argb,
            }];
        }
        vec![]
    }
    fn id(&self) -> String {
        "dev.agzes.lbs".into()
    }
    fn title(&self) -> String {
        "LBS - Linux Battle Shaper".into()
    }
    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            MenuItem::Standard(StandardItem {
                label: "Show Window".into(),
                activate: Box::new(move |_| {
                    glib::idle_add(move || {
                        if let Some(app) = gio::Application::default() {
                            if let Ok(app) = app.downcast::<gtk::Application>() {
                                app.activate();
                            }
                        }
                        glib::ControlFlow::Break
                    });
                }),
                ..Default::default()
            }),
            MenuItem::Separator,
            MenuItem::Standard(StandardItem {
                label: "Quit".into(),
                activate: Box::new(|_| {
                    backend::cleanup_all_limiters();
                    std::process::exit(0);
                }),
                ..Default::default()
            }),
        ]
    }
}
fn main() -> glib::ExitCode {
    backend::sync_binary();
    unsafe {
        signal(SIGINT, handle_exit_signal as *const () as libc::sighandler_t);
        signal(SIGTERM, handle_exit_signal as *const () as libc::sighandler_t);
    }

    let state: SharedState = Arc::new(Mutex::new(state::AppState::load()));

    let app = Application::builder()
        .application_id(state::APP_ID)
        .build();

    let state_c = state.clone();
    app.connect_startup(move |_| {
        backend::start_process_scanner(state_c.clone());
        backend::start_focus_monitor();

        {
            let s = state_c.lock().unwrap();
            for target in &s.targets {
                if target.is_active {
                    backend::start_limiter(state_c.clone(), target.pid, target.name.clone());
                }
            }
        }

        let tray = LBSTray {
            state: state_c.clone(),
        };
        let service = TrayService::new(tray);
        let handle = service.handle();
        service.spawn();

        glib::timeout_add_local(std::time::Duration::from_secs(2), move || {
            handle.update(|_| {});
            glib::ControlFlow::Continue
        });
    });

    let state_ui = state.clone();
    app.connect_activate(move |obj| {
        if let Some(window) = obj.windows().first() {
            window.present();
            return;
        }
        ui::build_ui(obj, state_ui.clone());
    });

    let exit_code = app.run_with_args::<&str>(&[]);
    backend::cleanup_all_limiters();
    exit_code
}
