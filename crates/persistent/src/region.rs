use database::DatabaseRef;

pub struct Region {
    pub npcs: database::Vec<usize>,
}

impl Region {
    pub fn new(database: DatabaseRef) -> Self {
        Self {
            npcs: database::Vec::new(database),
        }
    }

    pub fn serialize(&mut self, mut writer: impl std::io::Write) -> std::io::Result<()> {
        self.npcs.serialize(&mut writer)?;
        Ok(())
    }

    pub fn deserialize(
        mut reader: impl std::io::Read,
        database: DatabaseRef,
    ) -> std::io::Result<Self> {
        let npcs = database::Vec::deserialize(&mut reader, database)?;
        Ok(Self { npcs })
    }
}
