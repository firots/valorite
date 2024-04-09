use std::{sync::mpsc::{self, Receiver, Sender}, thread};
use crate::{constants::*, updater::Updater};
use eframe::egui;

pub struct SplashScreen {
    message: String,
    view_did_load: bool,
    receiver: Receiver<String>,
    sender: Sender<String>
}

impl Default for SplashScreen {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            message: CHECKING_FOR_UPDATES.to_owned(),
            view_did_load: false,
            sender: tx,
            receiver: rx
        }
    }
}

impl eframe::App for SplashScreen {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.view_did_load {
            self.view_did_load(ctx);
            self.view_did_load = true
        }

        self.receive_message();
        if self.message != HIDE_WINDOW_MESSAGE {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.image(egui::include_image!(
                    "../resources/valor_splash.png"
                ));
                ui.add_space(2.0);
                ui.label(format!("{}", self.message));
            });
        } 
    }
}

impl SplashScreen {
    fn view_did_load(&mut self, ctx: &egui::Context) {
        let sender = self.sender.clone();
        let ctx = ctx.clone();
        let _ = thread::spawn(move || {
            let mut updater = Updater {
                sender,
                launcher_update_file: None,
                ctx
            };
            updater.start();
        });
    }

    fn receive_message(&mut self) {
        if let Ok(message) = self.receiver.try_recv() {
            self.message = message;
        }
    }
}