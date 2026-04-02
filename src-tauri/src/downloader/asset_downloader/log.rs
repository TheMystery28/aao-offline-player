use crate::error::AppError;

/// Debug-only log writer.
pub(crate) struct DownloadLog {
    #[cfg(debug_assertions)]
    file: std::sync::Mutex<std::fs::File>,
}

impl DownloadLog {
    pub(crate) fn new(path: &std::path::Path) -> Result<Self, AppError> {
        // Rotate log if > 1MB
        if path.exists() {
            if let Ok(meta) = std::fs::metadata(path) {
                if meta.len() > 1_000_000 {
                    let old = path.with_extension("old.txt");
                    let _ = std::fs::rename(path, &old);
                }
            }
        }

        #[cfg(debug_assertions)]
        {
            let file = std::fs::File::create(path)
                .map_err(|e| format!("Failed to create log file: {}", e))?;
            Ok(Self {
                file: std::sync::Mutex::new(file),
            })
        }
        #[cfg(not(debug_assertions))]
        {
            let _ = path;
            Ok(Self {})
        }
    }

    #[allow(unused_variables)]
    pub(crate) fn log(&self, msg: &str) {
        #[cfg(debug_assertions)]
        {
            use std::io::Write;
            if let Ok(mut f) = self.file.lock() {
                let _ = writeln!(f, "{}", msg);
                let _ = f.flush();
            }
            println!("{}", msg);
        }
    }
}
