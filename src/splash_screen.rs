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
        egui::CentralPanel::default().show(ctx, |ui| {
            if !self.view_did_load {
                self.view_did_load();
                self.view_did_load = true
            }
            self.receive_message();
            ui.image(egui::include_image!(
                "../resources/valor_splash.png"
            ));
            ui.add_space(2.0);
            ui.label(format!("{}", self.message));
        });
    }
}

impl SplashScreen {
    fn view_did_load(&mut self) {
        let sender = self.sender.clone();
        let _ = thread::spawn(move || {
            let mut updater = Updater {
                message_sender: sender,
                launcher_update_file: None
            };
            updater.start_update_check();
        });
    }

    fn receive_message(&mut self) {
        if let Ok(message) = self.receiver.try_recv() {
            self.message = message;
        }
    }
}