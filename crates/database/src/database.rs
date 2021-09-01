use std::{
    fs::OpenOptions,
    io::{self, Seek, SeekFrom},
    path::Path,
};

use crate::{file::File, object::Object, raw::RawDatabase, reference::DatabaseRef};

pub struct Database {
    file: File,
    database: DatabaseRef,
}

impl Database {
    pub fn open<T: Object>(path: impl AsRef<Path>) -> io::Result<(Self, T)> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        let (raw, header) = RawDatabase::open(file, &T::format())?;
        let database = DatabaseRef::new(raw);
        let file = File::from_header(header, database.clone());
        let content = T::deserialize(&mut file.read(), database.clone())?;
        Ok((Self { file, database }, content))
    }

    pub fn create<T: Object>(
        path: impl AsRef<Path>,
        constructor: impl FnOnce(DatabaseRef) -> T,
    ) -> io::Result<(Self, T)> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)?;
        let raw = RawDatabase::create(file, &T::format())?;
        let database = DatabaseRef::new(raw);
        let file = File::new(database.clone());
        let content = constructor(database.clone());
        Ok((Self { file, database }, content))
    }

    pub fn snapshot<T: Object>(&mut self, content: &mut T) -> io::Result<()> {
        let mut writer = self.file.write();
        content.serialize(&mut writer)?;
        let size = writer.seek(SeekFrom::Current(0))?;
        writer.set_len(size);
        drop(writer);
        self.database.snapshot(self.file.header())
    }
}

impl Drop for Database {
    fn drop(&mut self) {
        self.database.lock().close()
    }
}
