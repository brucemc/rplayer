use eframe::{egui, epi};
use egui::{containers::*, *};
use std::sync::mpsc;
mod pipeline;
use std::time::Duration;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
pub struct PlayerApp {
    file_name: String,
    status_message: String,
    pipeline: Option<pipeline::Pipeline>,
    mpsc_sender: mpsc::SyncSender<Vec<f64>>,
    mpsc_receiver: std::sync::mpsc::Receiver<Vec<f64>>,
    spectrum_data: Vec<f64>,
    spectrum_data_peak: Vec<f64>,
    spectrum_lines: Vec<Shape>,
}

impl Default for PlayerApp {
    fn default() -> Self {
        let (mpsc_sender, mpsc_receiver) = mpsc::sync_channel(22000);
        let mut spectrum_data: Vec<f64> = Vec::with_capacity(pipeline::FFT_SIZE / 2);
        let mut spectrum_data_peak: Vec<f64> = Vec::with_capacity(pipeline::FFT_SIZE / 2);
        for _ in 0..pipeline::FFT_SIZE / 2 {
            spectrum_data.push(0.0);
            spectrum_data_peak.push(0.0);
        }

        Self {
            file_name: r##"resources/youve_got_speed.mp3"##.to_owned(),
            status_message: "".to_owned(),
            pipeline: Option::None,
            mpsc_sender,
            mpsc_receiver,
            spectrum_data,
            spectrum_data_peak,
            spectrum_lines: vec![],
        }
    }
}

impl PlayerApp {
    fn update_fft(&mut self, fft_data: &Vec<f64>) {
        let fft_it = fft_data.iter().skip(1).rev().enumerate();
        for (i, fft_val) in fft_it {
            if self.spectrum_data[i] < *fft_val {
                self.spectrum_data[i] = *fft_val;
            } else {
                self.spectrum_data[i] = self.spectrum_data[i] * 0.97;
            }
            if self.spectrum_data_peak[i] < *fft_val {
                self.spectrum_data_peak[i] = *fft_val * 1.05;
            } else {
                self.spectrum_data_peak[i] = self.spectrum_data_peak[i] * 0.998;
            }
        }
    }

    fn update_stopped(&mut self) {
        for i in 0..pipeline::FFT_SIZE / 2 {
            self.spectrum_data[i] = self.spectrum_data[i] * 0.94;
            self.spectrum_data_peak[i] = self.spectrum_data_peak[i] * 0.94;
        }
    }

    fn draw_spectrum_lines(&mut self, ui: &mut Ui) {
        let desired_size = vec2(ui.available_size().x, ui.available_size().y / 1.0);
        let (_id, rect) = ui.allocate_space(desired_size);

        let to_screen = emath::RectTransform::from_to(
            Rect::from_x_y_ranges(1.0..=pipeline::FFT_SIZE as f32 / 2.0, 100.0..=0.0),
            rect,
        );
        self.spectrum_lines.clear();
        let thickness = desired_size.x / pipeline::FFT_SIZE as f32 * 1.9;
        self.spectrum_lines.push(epaint::Shape::line(
            self.spectrum_data
                .iter()
                .enumerate()
                .filter(|(_, p)| **p > 0.0)
                .map(|(i, p)| to_screen * pos2(i as f32, *p as f32))
                .collect::<Vec<_>>(),
            Stroke::new(thickness, Color32::from_additive_luminance(196)),
        ));
        self.spectrum_lines.push(epaint::Shape::line(
            self.spectrum_data_peak
                .iter()
                .enumerate()
                .filter(|(_, p)| **p > 0.0)
                .map(|(i, p)| to_screen * pos2(i as f32, *p as f32))
                .collect::<Vec<_>>(),
            Stroke::new(thickness, Color32::RED),
        ));
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
    fn update(&mut self, ctx: &egui::CtxRef, _frame: &mut epi::Frame<'_>) {
        egui::TopPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Play / Pause Button
                let btn_txt = self
                    .pipeline
                    .as_ref()
                    .filter(|p| p.get_current_state() == gst::State::Playing)
                    .map_or("\u{25B6}", |_| "\u{23f8}");

                if ui.button(btn_txt).clicked() {
                    if let Some(p) = &self.pipeline {
                        p.play_pause()
                            .map_err(|err| {
                                self.status_message = format!("Error: {}", err);
                                self.pipeline = Option::None;
                            })
                            .ok();
                    } else {
                        pipeline::Pipeline::new(&self.file_name, self.mpsc_sender.clone())
                            .map_err(|err| {
                                self.status_message =
                                    format!("Error: could not create pipeline. {}", err);
                                self.pipeline = Option::None;
                            })
                            .and_then(|p| {
                                p.play()
                                    .map_err(|err| {
                                        self.status_message =
                                            format!("Error: could not play. {}", err);
                                        self.pipeline = Option::None;
                                    })
                                    .and_then(|_| {
                                        self.pipeline = Option::Some(p);
                                        Ok(())
                                    })
                            })
                            .ok();
                    }
                }

                // Stop button
                if ui.button("\u{23F9}").clicked() {
                    if let Some(p) = &self.pipeline {
                        p.stop()
                            .map_err(|err| {
                                self.status_message = format!("Error: could not stop. {}", err);
                            })
                            .ok();
                    }
                    self.pipeline = Option::None;
                }
            });

            ui.horizontal(|ui| {
                ui.label("File: ");
                ui.text_edit_singleline(&mut self.file_name);
                ui.visuals_mut().override_text_color = Some(egui::Color32::RED);
                ui.label(self.status_message.clone());
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Ok(fft_data) = self.mpsc_receiver.recv_timeout(Duration::from_millis(1)) {
                self.status_message = "".to_string();
                self.update_fft(&fft_data);
                self.draw_spectrum_lines(ui);
            } else {
                if let Some(p) = &self.pipeline {
                    if p.get_current_state() != gst::State::Playing {
                        self.update_stopped();
                        self.draw_spectrum_lines(ui);
                    }
                } else {
                    self.update_stopped();
                    self.draw_spectrum_lines(ui);
                }
            }
            ui.painter().extend(self.spectrum_lines.clone());
        });
        ctx.request_repaint();
    }
}
