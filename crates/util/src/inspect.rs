pub use derive::Inspect;

pub trait Inspector {
    fn inspect_u64(&mut self, name: &str, value: u64);
    fn inspect_str(&mut self, name: &str, value: &str);
    fn edit_u64(&mut self, name: &str, value: &mut u64);
    fn inspect_u8(&mut self, name: &str, value: u8);
    fn edit_u8(&mut self, name: &str, value: &mut u8);
    fn inspect(&mut self, name: &str, f: impl FnOnce(&mut Self));
}

pub trait Inspect {
    fn inspect(&self, name: &str, inspector: &mut impl Inspector);

    fn inspect_mut(&mut self, name: &str, inspector: &mut impl Inspector);
}

impl Inspect for u64 {
    fn inspect(&self, name: &str, inspector: &mut impl Inspector) {
        inspector.inspect_u64(name, *self)
    }

    fn inspect_mut(&mut self, name: &str, inspector: &mut impl Inspector) {
        inspector.edit_u64(name, self)
    }
}

impl Inspect for u8 {
    fn inspect(&self, name: &str, inspector: &mut impl Inspector) {
        inspector.inspect_u8(name, *self)
    }

    fn inspect_mut(&mut self, name: &str, inspector: &mut impl Inspector) {
        inspector.edit_u8(name, self)
    }
}
