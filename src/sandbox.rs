
use crate::sandbox::FileSystemType::RawFolder;

pub struct Sandbox {
    home_path: String,
    home_type: FileSystemType,
    isolate_proc: bool,
    directories: Vec<String>,
    mounts: Vec<Mount>,
    symlinks: Vec<Symlink>,
}

pub struct Symlink{
    to: String,
    from: String,
}


pub enum FileSystemType{
    RawFolder,
    GoCryptFS,
    CryFS
}


struct Mount {
    from: String,
    to: String,
    readonly: bool,
}

impl Mount {
    pub fn new(from: String, to: String,readonly: bool) -> Mount {
        Mount{from, to, readonly}
    }
}
