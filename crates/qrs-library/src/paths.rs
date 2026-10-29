//! Where settings, user data and caches live (D6): `QUICKRS_DATA` if set (tests, screenshots); else next to the
//! program (or in a `quickRS Data` folder beside it or beside quickRS.app) when a `config.json` or `userdata.json`
//! is there (a portable install); else the OS data folder
//! (`%APPDATA%\quickRS` on Windows).

use std::path::{Path, PathBuf};

/// The folder a portable install keeps its data in, next to the program (or next to quickRS.app on a Mac).
pub const PORTABLE_DIR: &str = "quickRS Data";

/// Where a portable install's data may be: next to the program, its "quickRS Data" folder, and for a macOS app
/// bundle (`X/quickRS.app/Contents/MacOS/quickrs`) the "quickRS Data" folder beside the bundle.
fn portable_candidates(exe_dir: &Path) -> Vec<PathBuf> {
    let mut out = vec![exe_dir.to_path_buf(), exe_dir.join(PORTABLE_DIR)];
    if exe_dir.ends_with("Contents/MacOS")
        && let Some(beside_app) = exe_dir.parent().and_then(Path::parent).and_then(Path::parent)
    {
        out.push(beside_app.join(PORTABLE_DIR));
    }
    out
}

#[derive(Clone, Debug)]
pub struct DataDir {
    pub root: PathBuf,
}

impl DataDir {
    pub fn locate() -> DataDir {
        if let Some(d) = std::env::var_os("QUICKRS_DATA") {
            return DataDir::at(PathBuf::from(d));
        }
        if let Some(exe_dir) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
        {
            for dir in portable_candidates(&exe_dir) {
                if dir.join("config.json").is_file() || dir.join("userdata.json").is_file() {
                    return DataDir::at(dir);
                }
            }
        }
        let root = directories::ProjectDirs::from("", "", "quickRS")
            .map(|p| p.data_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        DataDir::at(root)
    }

    pub fn at(root: PathBuf) -> DataDir {
        let _ = std::fs::create_dir_all(&root);
        DataDir { root }
    }

    pub fn config(&self) -> PathBuf {
        self.root.join("config.json")
    }

    pub fn userdata(&self) -> PathBuf {
        self.root.join("userdata.json")
    }

    pub fn cache(&self) -> PathBuf {
        self.root.join(".quickrs-cache")
    }

    pub fn index(&self) -> PathBuf {
        self.cache().join("library.json")
    }

    pub fn art(&self) -> PathBuf {
        self.cache().join("art")
    }

    /// keys.json next to the data (the Python app kept it in its folder).
    pub fn keys(&self) -> PathBuf {
        self.root.join("keys.json")
    }

    /// Lessons beyond the built-in ones (`*.json`, P13.3).
    pub fn lessons(&self) -> PathBuf {
        self.root.join("lessons")
    }
}

/// Write `text` to `path` through a temporary file, so a crash never leaves half a file.
pub fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}
