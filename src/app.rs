use eframe::{egui, epi};
use egui::{containers::*, *};
use std::sync::mpsc;
mod pipeline;
use std::collections::VecDeque;
use std::time::Duration;

static RMS_SIZE: usize = 250;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
pub struct PlayerApp {
    file_name: String,
    status_message: String,
    pipeline: Option<pipeline::Pipeline>,
    sender: mpsc::SyncSender<f64>,
    receiver: std::sync::mpsc::Receiver<f64>,
    rms: VecDeque<f64>,
}

impl Default for PlayerApp {
    fn default() -> Self {
        let (sender, receiver) = mpsc::sync_channel(220);
        let mut rms: VecDeque<f64> = VecDeque::with_capacity(RMS_SIZE);
        for _ in 0..RMS_SIZE {
            rms.push_back(0.0);
        }

        Self {
            file_name: r##"resources/youve_got_speed.mp3"##.to_owned(),
            status_message: "".to_owned(),
            pipeline: Option::None,
            sender,
            receiver,
            rms,
        }
    }
}

impl epi::App for PlayerApp {
    fn name(&self) -> &str {
        "rplayer"
    }

    /// Called by the framework to load old app state (if any).
    #[cfg(feature = "persistence")]
    fn load(&mut self, storage: &dyn epi::Storage) {
        *self = epi::get_value(storage, epi::APP_KEY).unwrap_or_default()
    }

    /// Called by the frame work to save state before shutdown.
    #[cfg(feature = "persistence")]
    fn save(&mut self, storage: &mut dyn epi::Storage) {
        epi::set_value(storage, epi::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::CtxRef, _frame: &mut epi::Frame<'_>) {
        let PlayerApp {
            file_name,
            status_message,
            pipeline,
            sender,
            receiver,
            rms,
        } = self;

        if let Ok(r) = receiver.recv_timeout(Duration::from_millis(10)) {
            rms.push_back(r);
            if rms.len() > RMS_SIZE {
                rms.pop_front();
            }
            *status_message = "".to_string();
        }

        egui::TopPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Play / Pause Button
                let btn_txt = pipeline
                    .as_ref()
                    .filter(|p| p.get_current_state() == gst::State::Playing)
                    .map_or("\u{25B6}", |_| "\u{23f8}");

                if ui.button(btn_txt).clicked() {
                    if let Some(p) = pipeline {
                        p.play_pause()
                            .map_err(|err| {
                                *status_message = format!("Error: {}", err);
                                *pipeline = Option::None;
                            })
                            .ok();
                    } else {
                        pipeline::Pipeline::new(file_name, sender.clone())
                            .map_err(|err| {
                                *status_message =
                                    format!("Error: could not create pipeline. {}", err);
                                *pipeline = Option::None;
                            })
                            .and_then(|p| {
                                p.play()
                                    .map_err(|err| {
                                        *status_message = format!("Error: could not play. {}", err);
                                        *pipeline = Option::None;
                                    })
                                    .and_then(|_| {
                                        *pipeline = Option::Some(p);
                                        Ok(())
                                    })
                            })
                            .ok();
                    }
                }

                // Stop button
                if ui.button("\u{23F9}").clicked() {
                    if let Some(p) = pipeline {
                        p.stop()
                            .map_err(|err| {
                                *status_message = format!("Error: could not stop. {}", err);
                            })
                            .ok();
                    }
                    *pipeline = Option::None;
                }
            });

            ui.horizontal(|ui| {
                ui.label("File: ");
                ui.text_edit_singleline(file_name);
                ui.visuals_mut().override_text_color = Some(egui::Color32::RED);
                ui.label(status_message.clone());
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("RMS");
            ui.separator();

            let desired_size = ui.available_width() * vec2(1.0, 0.35);
            let (_id, rect) = ui.allocate_space(desired_size);

            let to_screen = emath::RectTransform::from_to(
                Rect::from_x_y_ranges(0.0..=RMS_SIZE as f32, 0.52..=0.55),
                rect,
            );

            let mut shapes = vec![];
            let thickness = 4.0;

            // let points: Vec<Pos2> = (0..RMS_SIZE)
            //     .filter(|i| rms[*i] > 0.0)
            //     .map(|i| to_screen * pos2(i as f32, rms[i] as f32))
            //     .collect();
            // shapes.push(epaint::Shape::line(
            //     points,
            //     Stroke::new(thickness, Color32::from_additive_luminance(196)),
            // ));

            shapes.push(epaint::Shape::line(
                rms.iter()
                    .enumerate()
                    .filter(|(_, p)| **p > 0.0)
                    .map(|(i, p)| to_screen * pos2(i as f32, *p as f32))
                    .collect::<Vec<_>>(),
                Stroke::new(thickness, Color32::from_additive_luminance(196)),
            ));

            ui.painter().extend(shapes);
        });

        ctx.request_repaint();
    }
}
