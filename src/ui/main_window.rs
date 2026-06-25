use gpui::{
    actions, div, px, rgb, App, Bounds, ClipboardItem, Context, FocusHandle, Focusable,
    IntoElement, ParentElement, Point, Render, Size, Styled, Window, WindowBounds, WindowOptions,
};
use gpui::prelude::*;

use crate::app_state::{SharedState, StreamStatus};
use crate::pipeline;
use super::theme;

actions!(screenstream, [ToggleStream]);

pub struct MainWindow {
    focus_handle: FocusHandle,
    shared: SharedState,
    pipeline: Option<crate::pipeline::PipelineHandle>,
    active_tab: usize,
    url_copied: bool,
}

impl MainWindow {
    pub fn new(shared: SharedState, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            shared,
            pipeline: None,
            active_tab: 0,
            url_copied: false,
        }
    }

    fn toggle_stream(&mut self, cx: &mut Context<Self>) {
        let status = self.shared.lock().status.clone();
        match status {
            StreamStatus::Idle | StreamStatus::Error(_) => self.start_stream(cx),
            StreamStatus::Streaming | StreamStatus::Starting => self.stop_stream(cx),
            StreamStatus::Stopping => {}
        }
    }

    fn start_stream(&mut self, cx: &mut Context<Self>) {
        let config = self.shared.lock().config.clone();
        match pipeline::start(config, self.shared.clone()) {
            Ok(handle) => self.pipeline = Some(handle),
            Err(e) => {
                self.shared.lock().status = StreamStatus::Error(e.to_string());
            }
        }
        cx.notify();
    }

    fn stop_stream(&mut self, cx: &mut Context<Self>) {
        self.shared.lock().status = StreamStatus::Stopping;
        self.pipeline = None;
        cx.notify();
    }

    // ── Header ────────────────────────────────────────────────────────────────

    fn status_color(&self) -> gpui::Rgba {
        let st = self.shared.lock();
        match &st.status {
            StreamStatus::Streaming => theme::green(),
            StreamStatus::Starting | StreamStatus::Stopping => theme::yellow(),
            StreamStatus::Error(_) => theme::red(),
            StreamStatus::Idle => theme::muted(),
        }
    }

    fn status_label(&self) -> &'static str {
        let st = self.shared.lock();
        match &st.status {
            StreamStatus::Idle => "● Idle",
            StreamStatus::Starting => "◌ Starting…",
            StreamStatus::Streaming => "● Live",
            StreamStatus::Stopping => "◌ Stopping",
            StreamStatus::Error(_) => "✕ Error",
        }
    }

    fn render_header(&self) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .px_6()
            .py_4()
            .bg(theme::surface())
            .border_b_1()
            .border_color(theme::overlay())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(theme::text())
                            .child("ScreenStream"),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme::subtext())
                            .child("v0.1.0 · SRT+AES-256"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .px_3()
                    .py_1()
                    .rounded_full()
                    .bg(theme::overlay())
                    .text_size(px(12.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(self.status_color())
                    .child(self.status_label()),
            )
    }

    // ── Tabs ─────────────────────────────────────────────────────────────────

    fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tabs = [("Stream", 0usize), ("Settings", 1)];
        div()
            .flex()
            .gap_1()
            .px_6()
            .pt_4()
            .children(tabs.into_iter().map(|(label, idx)| {
                let active = self.active_tab == idx;
                div()
                    .id(("tab", idx))
                    .px_4()
                    .py_2()
                    .rounded_t_md()
                    .cursor_pointer()
                    .text_size(px(13.))
                    .font_weight(if active {
                        gpui::FontWeight::SEMIBOLD
                    } else {
                        gpui::FontWeight::NORMAL
                    })
                    .text_color(if active { theme::accent() } else { theme::subtext() })
                    .bg(if active { theme::overlay() } else { theme::bg() })
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        this.active_tab = idx;
                        cx.notify();
                    }))
                    .child(label)
            }))
    }

    // ── Stream tab ────────────────────────────────────────────────────────────

    fn stat_card(&self, label: &str, value: &str) -> impl IntoElement {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .p_3()
            .rounded_lg()
            .bg(theme::surface())
            .border_1()
            .border_color(theme::overlay())
            .child(
                div()
                    .text_size(px(18.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme::text())
                    .child(value.to_owned()),
            )
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(theme::subtext())
                    .child(label.to_owned()),
            )
    }

    fn render_stream_tab(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let st = self.shared.lock();
        let is_streaming = st.status.is_active();
        let obs_url = st.primary_obs_url();
        let stats = st.stats.clone();
        let err_msg = match &st.status {
            StreamStatus::Error(m) => Some(m.clone()),
            _ => None,
        };
        drop(st);

        div()
            .flex()
            .flex_col()
            .gap_4()
            .p_6()
            // OBS connection card
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_4()
                    .rounded_lg()
                    .bg(theme::surface())
                    .border_1()
                    .border_color(theme::overlay())
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme::subtext())
                            .child("OBS SOURCE URL  (SRT · AES-256)"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(12.))
                                    .text_color(theme::accent())
                                    .child(obs_url),
                            )
                            .child(
                                div()
                                    .id("copy-btn")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .bg(if self.url_copied { theme::green() } else { theme::overlay() })
                                    .text_size(px(11.))
                                    .text_color(theme::text())
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        let url = this.shared.lock().primary_obs_url();
                                        cx.write_to_clipboard(ClipboardItem::new_string(url));
                                        this.url_copied = true;
                                        cx.notify();
                                    }))
                                    .child(if self.url_copied { "Copied!" } else { "Copy" }),
                            ),
                    ),
            )
            // Stats row
            .child(
                div()
                    .flex()
                    .gap_3()
                    .children([
                        self.stat_card("FPS", &format!("{:.1}", stats.actual_fps)),
                        self.stat_card("Video", &format!("{:.0} kbps", stats.video_kbps)),
                        self.stat_card("Audio", &format!("{:.0} kbps", stats.audio_kbps)),
                        self.stat_card("Encode", &format!("{:.1} ms", stats.encode_ms)),
                        self.stat_card("Clients", &format!("{}", stats.connected_clients)),
                    ]),
            )
            // Error message
            .child(if let Some(msg) = err_msg {
                div()
                    .p_3()
                    .rounded_md()
                    .bg(rgb(0x3d1a1a))
                    .border_1()
                    .border_color(theme::red())
                    .text_size(px(12.))
                    .text_color(theme::red())
                    .child(format!("Error: {}", msg))
                    .into_any_element()
            } else {
                div().into_any_element()
            })
            // Start / Stop button
            .child(
                div()
                    .id("toggle-btn")
                    .px_8()
                    .py_3()
                    .rounded_lg()
                    .cursor_pointer()
                    .text_size(px(15.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme::text())
                    .bg(if is_streaming { theme::red() } else { theme::accent() })
                    .on_click(cx.listener(|this, _, _window, cx| this.toggle_stream(cx)))
                    .child(if is_streaming { "Stop Streaming" } else { "Start Streaming" }),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(theme::muted())
                    .child(
                        "In OBS: Sources → + → Media Source → uncheck \"Local File\" → paste URL above",
                    ),
            )
    }

    // ── Settings tab ──────────────────────────────────────────────────────────

    fn section_header(&self, label: &str) -> impl IntoElement {
        div()
            .text_size(px(11.))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(theme::subtext())
            .pb_1()
            .border_b_1()
            .border_color(theme::overlay())
            .child(label.to_uppercase())
    }

    fn setting_row(&self, key: &str, value: &str) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .min_w(px(120.))
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(theme::subtext())
                    .child(key.to_owned()),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(theme::text())
                    .child(value.to_owned()),
            )
    }

    fn render_settings_tab(&self) -> impl IntoElement {
        let st = self.shared.lock();
        let cfg = st.config.clone();
        let ips = st.local_ips.clone();
        drop(st);

        div()
            .flex()
            .flex_col()
            .gap_4()
            .p_6()
            .child(self.section_header("Network"))
            .child(
                div()
                    .flex()
                    .gap_4()
                    .child(self.setting_row("Port", &cfg.port.to_string()))
                    .child(self.setting_row(
                        "Bind Interface",
                        cfg.bind_interface.as_deref().unwrap_or("All (0.0.0.0)"),
                    ))
                    .child(self.setting_row("Local IPs", &ips.join("  |  "))),
            )
            .child(self.section_header("Encryption"))
            .child(
                div()
                    .flex()
                    .gap_4()
                    .child(self.setting_row("Protocol", "AES-256 / SRT"))
                    .child(self.setting_row(
                        "Passphrase",
                        &"*".repeat(cfg.passphrase.len().min(16)),
                    )),
            )
            .child(self.section_header("Video"))
            .child(
                div()
                    .flex()
                    .gap_4()
                    .child(self.setting_row("Codec", cfg.video_codec.display_name()))
                    .child(self.setting_row("Bitrate", &format!("{} kbps", cfg.video_kbps)))
                    .child(self.setting_row("Resolution", cfg.resolution.display_name()))
                    .child(self.setting_row("FPS", &cfg.fps.to_string())),
            )
            .child(self.section_header("Audio"))
            .child(
                div()
                    .flex()
                    .gap_4()
                    .child(self.setting_row("Codec", "AAC"))
                    .child(self.setting_row("Bitrate", &format!("{} kbps", cfg.audio_kbps)))
                    .child(self.setting_row(
                        "Mic Device",
                        cfg.mic_device.as_deref().unwrap_or("Default"),
                    )),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(theme::muted())
                    .child("Edit config.json (next to the binary) and restart to change settings."),
            )
    }
}

impl Focusable for MainWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MainWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context("MainWindow")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::bg())
            .text_color(theme::text())
            .on_action(cx.listener(|this, _: &ToggleStream, _window, cx| {
                this.toggle_stream(cx)
            }))
            .child(self.render_header())
            .child(self.render_tabs(cx))
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(if self.active_tab == 0 {
                        self.render_stream_tab(cx).into_any_element()
                    } else {
                        self.render_settings_tab().into_any_element()
                    }),
            )
    }
}

/// Open the main application window.
pub fn open(shared: SharedState, cx: &mut App) {
    let bounds = Bounds {
        origin: Point::default(),
        size: Size {
            width: px(860.),
            height: px(560.),
        },
    };
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            app_id: Some("screenstream".into()),
            ..Default::default()
        },
        |_, cx| cx.new(|cx| MainWindow::new(shared, cx)),
    )
    .expect("Failed to open main window");
}
