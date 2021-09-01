use std::{
    collections::VecDeque,
    str::from_utf8,
    time::{Duration, Instant},
};

use egui::{
    plot::{Line, Plot, Value, Values},
    Align, Color32, CtxRef, DragValue, RadioButton, ScrollArea, Slider, Window,
};
use network::{Connection, ConnectionStats, ConnectionStatsDiff};
use protocol::Request;
use tracing::info;
use tracing_subscriber::filter::LevelFilter;
use util::inspect::Inspect;

use crate::{renderer::RenderTimestamps, scene::SceneContext, subscriber::FilterHandle};

pub struct DebugContext {
    frame_count: usize,
    frames_per_second: usize,
    last_frame_count: usize,
    last_stats: Option<ConnectionStats>,
    stats_diff: Option<ConnectionStatsDiff>,
    last_second: Instant,
    frame_start: Instant,
    frame_times: VecDeque<(Instant, Instant, Option<RenderTimestamps>)>,
    frame_times_secs: f64,
    show_cpu_time: bool,
    show_gpu_time: bool,
    log: String,
    log_scroll_to_bottom: bool,
    log_limit_entries: bool,
    log_size_limit: usize,
    log_fps: bool,
}

#[derive(Default)]
pub struct DebugWindows {
    pub frame_times: bool,
    pub information: bool,
    pub log: bool,
}

impl DebugContext {
    pub fn begin_frame(&mut self) {
        self.frame_start = Instant::now();
    }

    pub fn log(&mut self, buf: Vec<u8>) {
        self.log.push_str(from_utf8(&buf).unwrap());
        self.trim_log();
    }

    pub fn trim_log(&mut self) {
        while self.log_limit_entries && self.log_size_limit < self.log.len() {
            self.log = match self.log.split_once('\n') {
                Some((_, log)) => log,
                None => "",
            }
            .to_string()
        }
    }

    pub fn end_frame(
        &mut self,
        last_timestamps: Option<RenderTimestamps>,
        connection: Option<&Connection<Request>>,
    ) {
        self.frame_count += 1;
        let now = Instant::now();
        if now.duration_since(self.last_second).as_secs() >= 1 {
            let current_stats = connection.map(Connection::stats);
            self.stats_diff = if let Some(current_stats) = current_stats {
                self.last_stats.map(|last_stats| current_stats - last_stats)
            } else {
                None
            };
            self.frames_per_second = self.frame_count - self.last_frame_count;
            self.last_frame_count = self.frame_count;
            self.last_stats = current_stats;
            self.last_second += Duration::from_secs(1);
            if self.log_fps {
                info!("FPS: {}", self.frames_per_second);
            }
        }
        if let Some((_, _, timestamps)) = self.frame_times.back_mut() {
            *timestamps = last_timestamps;
        }
        self.frame_times.push_back((self.frame_start, now, None));
        while let Some(front) = self.frame_times.front() {
            if now.duration_since(front.1).as_secs_f64() > self.frame_times_secs {
                self.frame_times.pop_front();
            } else {
                break;
            }
        }
    }

    fn render_information(&mut self, ctx: &CtxRef, open: &mut bool, scene: Option<&SceneContext>) {
        Window::new("Information").open(open).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!(
                    "FPS: {} Frame Count: {}",
                    self.frames_per_second, self.frame_count
                ));
                ui.checkbox(&mut self.log_fps, "Log FPS");
            });
            if let Some(stats_diff) = &self.stats_diff {
                stats_diff.inspect("Last second", ui);
            }
            if let Some(stats) = &self.last_stats {
                stats.inspect("Total", ui);
            }
            if let Some(scene) = scene {
                ui.collapsing("Camera", |ui| {
                    ui.label(format!("x: {}", scene.camera.translation.x));
                    ui.label(format!("y: {}", scene.camera.translation.y));
                    ui.label(format!("z: {}", scene.camera.translation.z));
                    ui.label(format!("roll: {}", scene.camera.roll));
                    ui.label(format!("pitch: {}", scene.camera.pitch));
                    ui.label(format!("yaw: {}", scene.camera.yaw));
                });
            }
        });
    }

    fn render_frame_times(&mut self, ctx: &CtxRef, open: &mut bool) {
        Window::new("Frame times").open(open).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Frame time storage duration [s]");
                ui.add(Slider::new(&mut self.frame_times_secs, 0.0f64..=120.0f64));
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.show_cpu_time, "Show CPU Time");
                ui.checkbox(&mut self.show_gpu_time, "Show GPU Time");
            });
            if let Some(front) = self.frame_times.front() {
                let origin = front.0;
                let plot = Plot::new("frame_times")
                    .include_x(0.0)
                    .include_x(self.frame_times_secs)
                    .include_y(0.0);
                let plot = if self.show_cpu_time {
                    plot.line(
                        Line::new(Values::from_values_iter(self.frame_times.iter().map(
                            |(begin, end, _)| Value {
                                x: begin.duration_since(origin).as_secs_f64(),
                                y: end.duration_since(*begin).as_secs_f64() * 1000.0,
                            },
                        )))
                        .color(Color32::RED)
                        .name("CPU Time"),
                    )
                } else {
                    plot
                };
                let plot = if self.show_gpu_time {
                    plot.line(
                        Line::new(Values::from_values_iter(
                            self.frame_times
                                .iter()
                                .filter_map(|(begin, _, timestamps)| {
                                    timestamps.as_ref().map(|timestamps| Value {
                                        x: begin.duration_since(origin).as_secs_f64(),
                                        y: timestamps.end - timestamps.begin,
                                    })
                                }),
                        ))
                        .color(Color32::BLUE)
                        .name("GPU Time"),
                    )
                } else {
                    plot
                };
                ui.add(plot);
            }
        });
    }

    fn render_log(&mut self, ctx: &CtxRef, open: &mut bool, handle: &FilterHandle) {
        let level = handle.clone_current().unwrap();
        Window::new("Log").open(open).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .add(RadioButton::new(level == LevelFilter::OFF, "Off"))
                    .clicked()
                {
                    handle.reload(LevelFilter::OFF).unwrap();
                }
                if ui
                    .add(RadioButton::new(level == LevelFilter::ERROR, "Error"))
                    .clicked()
                {
                    handle.reload(LevelFilter::ERROR).unwrap();
                }
                if ui
                    .add(RadioButton::new(level == LevelFilter::WARN, "Warn"))
                    .clicked()
                {
                    handle.reload(LevelFilter::WARN).unwrap();
                }
                if ui
                    .add(RadioButton::new(level == LevelFilter::INFO, "Info"))
                    .clicked()
                {
                    handle.reload(LevelFilter::INFO).unwrap();
                }
                if ui
                    .add(RadioButton::new(level == LevelFilter::DEBUG, "Debug"))
                    .clicked()
                {
                    handle.reload(LevelFilter::DEBUG).unwrap();
                }
                if ui
                    .add(RadioButton::new(level == LevelFilter::TRACE, "Trace"))
                    .clicked()
                {
                    handle.reload(LevelFilter::TRACE).unwrap();
                }
                ui.checkbox(&mut self.log_scroll_to_bottom, "Scroll to bottom");
                ui.checkbox(&mut self.log_limit_entries, "Limit log entries");
                if self.log_limit_entries {
                    ui.add(
                        DragValue::new(&mut self.log_size_limit)
                            .speed(1)
                            .prefix("Entry limit: "),
                    );
                    self.trim_log();
                }
                if ui.button("Clear log").clicked() {
                    self.log.clear();
                }
            });
            ScrollArea::from_max_height(600.0).show(ui, |ui| {
                ui.code(&self.log);
                if self.log_scroll_to_bottom {
                    ui.scroll_to_cursor(Align::BOTTOM);
                }
            });
        });
    }

    pub fn render(
        &mut self,
        ctx: &CtxRef,
        windows: &mut DebugWindows,
        scene: Option<&SceneContext>,
        handle: &FilterHandle,
    ) {
        self.render_information(ctx, &mut windows.information, scene);
        self.render_frame_times(ctx, &mut windows.frame_times);
        self.render_log(ctx, &mut windows.log, handle);
    }
}

impl Default for DebugContext {
    fn default() -> Self {
        Self {
            frame_count: 0,
            frames_per_second: 0,
            last_frame_count: 0,
            last_second: Instant::now(),
            last_stats: None,
            stats_diff: Default::default(),
            frame_start: Instant::now(),
            frame_times: VecDeque::new(),
            frame_times_secs: 10.0,
            show_cpu_time: true,
            show_gpu_time: true,
            log: String::new(),
            log_scroll_to_bottom: true,
            log_limit_entries: false,
            log_size_limit: 64 * 1024,
            log_fps: false,
        }
    }
}
