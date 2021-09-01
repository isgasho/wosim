use bitflags::bitflags;

bitflags! {
    pub struct Groups: u32 {
        const WALKABLE = 0b001;
        const CHARACTER = 0b010;
        const SENSOR = 0b100;
    }
}
