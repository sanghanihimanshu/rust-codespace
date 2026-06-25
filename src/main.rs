mod app_state;
mod capture;
mod config;
mod encode;
mod pipeline;
mod stream;
mod ui;

use std::sync::Arc;

use app_state::AppState;
use gpui::App;
use parking_lot::Mutex;
use tracing_subscriber::{EnvFilter, fmt};

fn main() {
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("screenstream=info,warn")),
        )
        .init();

    tracing::info!("ScreenStream starting up");

    let shared = Arc::new(Mutex::new(AppState::new()));

    {
        let displays = capture::screen::list_displays();
        let mut st = shared.lock();
        if !displays.is_empty() {
            st.available_displays = displays;
        }
    }

    {
        let devices = capture::audio::list_input_devices();
        let mut st = shared.lock();
        st.available_audio_devices = devices;
    }

    gpui_platform::application().run(move |cx: &mut App| {
        ui::main_window::open(shared, cx);
    });
}
