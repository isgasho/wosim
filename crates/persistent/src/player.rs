use derive::DbVec;
use protocol::PlayerSlots;

#[derive(DbVec)]
pub struct Player {
    pub slots: PlayerSlots,
}
