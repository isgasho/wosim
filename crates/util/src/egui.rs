use egui::Ui;

use crate::inspect::Inspector;

impl Inspector for Ui {
    fn inspect_u64(&mut self, name: &str, value: u64) {
        self.label(format!("{}: {}", name, value));
    }

    fn edit_u64(&mut self, name: &str, value: &mut u64) {
        self.inspect_u64(name, *value)
    }

    fn inspect_u8(&mut self, name: &str, value: u8) {
        self.inspect_u64(name, value as u64);
    }

    fn edit_u8(&mut self, name: &str, value: &mut u8) {
        self.inspect_u8(name, *value);
    }

    fn inspect(&mut self, name: &str, f: impl FnOnce(&mut Self)) {
        self.collapsing(name, |ui| f(ui));
    }

    fn inspect_str(&mut self, name: &str, value: &str) {
        self.label(format!("{}: {}", name, value));
    }
}
