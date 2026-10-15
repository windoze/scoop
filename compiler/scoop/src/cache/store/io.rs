use std::fs::{File, OpenOptions};
use std::path::Path;

pub(crate) fn open_build_lock(path: &Path) -> std::io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
}
