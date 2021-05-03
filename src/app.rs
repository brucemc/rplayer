use eframe::{egui, epi};
use egui::{containers::*, *};
use std::thread;
use std::sync::mpsc;
mod pipeline;
use gst::ElementExtManual;
use std::time::Duration;
use std::collections::VecDeque;

static RMS_SIZE: usize = 250;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
pub struct PlayerApp {
    file_name: String,
    gstreamer_pipeline: Option<gst::Pipeline>,
    sender : mpsc::SyncSender<f64>,
    receiver : std::sync::mpsc::Receiver<f64>,
    rms: VecDeque<f64>,
}

impl Default for PlayerApp {
    fn default() -> Self {

        let (sender, receiver) = mpsc::sync_channel(220);
        let mut rms : VecDeque<f64> = VecDeque::with_capacity(RMS_SIZE);
        for _ in 0..RMS_SIZE {
            rms.push_back(0.0);
        }

        Self {
            file_name: r##"resources/youve_got_speed.mp3"##.to_owned(),
            gstreamer_pipeline: Option::None,
            sender,
            receiver,
            rms,
        }
    }
}

impl epi::App for PlayerApp {
    fn name(&self) -> &str {
        "gplayer"
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
            gstreamer_pipeline,
            sender,
            receiver,
            rms,
        } = self;

        match receiver.recv_timeout(Duration::from_millis(10)) {
            Ok(r) => {
//                println!("{:?} rms = {}", std::thread::current().id(),r);
                rms.push_back(r);
                if rms.len() > RMS_SIZE {
                    rms.pop_front();
                }
            },
            _ => (),
        }
        egui::TopPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {

                let btn = match gstreamer_pipeline {
                    Some(p) => {
                        if p.get_current_state() != gst::State::Playing {
                            "\u{25B6}"
                        } else {
                            "\u{23f8}"
                        }
                    }
                    _ => { "\u{25B6}" }
                };

                if ui.button(btn).clicked() {
                    match gstreamer_pipeline {
                        Some(p) => {
                            if p.get_current_state() == gst::State::Paused {
                                p.set_state(gst::State::Playing).unwrap();
                                println!("{:?} Pipeline playing", thread::current().id());
                            }
                            else {
                                p.set_state(gst::State::Paused).unwrap();
                                println!("{:?} Pipeline paused", thread::current().id());
                            }
                        }
                        _ => {
                            *gstreamer_pipeline = match pipeline::create(file_name, sender.clone()) {
                                Ok(p) => {
                                    match p.set_state(gst::State::Playing) {
                                        Ok(_) => {
                                            println!("{:?} Pipeline playing {}", thread::current().id(), file_name);
                                            Option::Some(p)
                                        }
                                        _ => {
                                            println!("Error: could not play");
                                            Option::None
                                        }
                                    }
                                }
                                Err(e) => {
                                    println!("Error: {}", e);
                                    Option::None
                                }
                            }
                        }
                    }
                }
                if ui.button("\u{23F9}").clicked() {
                    match gstreamer_pipeline {
                        Some(p) => {
                            p.set_state(gst::State::Null).unwrap();
                            println!("{:?} Pipeline stopped", thread::current().id());
                        }
                        _ => (),
                    }
                    *gstreamer_pipeline = Option::None;
                }
            });
            ui.horizontal(|ui| {
                ui.label("File: ");
                ui.text_edit_singleline(file_name);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("RMS");
            ui.separator();

            let desired_size = ui.available_width() * vec2(1.0, 0.35);
            let (_id, rect) = ui.allocate_space(desired_size);

            let to_screen =
                emath::RectTransform::from_to(Rect::from_x_y_ranges(0.0..=RMS_SIZE as f32, 0.52..=0.55), rect);

            let mut shapes = vec![];

            let points : Vec<Pos2> = (0..RMS_SIZE)
                .filter(|i| {
                    rms[*i] > 0.0
                })
                .map(|i| {
                    to_screen * pos2(i as f32, rms[i] as f32)
                }).collect();

            let thickness = 4.0;
            shapes.push(epaint::Shape::line(
                points,
                Stroke::new(thickness, Color32::from_additive_luminance(196)),
            ));
            ui.painter().extend(shapes);
        });

        ctx.request_repaint();

    }
}


