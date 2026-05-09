use std::path::PathBuf;

/// Default app data dir per platform.
/// macOS: ~/Library/Application Support/TaskListener
/// Windows: %APPDATA%\TaskListener
/// Linux: $XDG_DATA_HOME/tasklistener (used for dev/CI)
pub fn default_data_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME").unwrap_or_default();
        PathBuf::from(home).join("Library/Application Support/TaskListener")
    }
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var_os("APPDATA").unwrap_or_default();
        PathBuf::from(appdata).join("TaskListener")
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        let xdg = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let home = std::env::var_os("HOME").unwrap_or_default();
                PathBuf::from(home).join(".local/share")
            });
        xdg.join("tasklistener")
    }
}

pub fn default_db_path() -> PathBuf {
    default_data_dir().join("tasks.db")
}
