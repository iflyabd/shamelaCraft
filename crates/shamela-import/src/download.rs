//! Background downloads with progress + cancellation (std only).
//!
//! Each job runs on its own thread streaming `ureq`, reporting through
//! shared atomics the UI polls. No async runtime needed.

use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};

/// Shared progress handle polled by the interface.
pub struct DlProgress {
    pub written: AtomicU64,
    pub total: AtomicU64,
    pub finished: AtomicBool,
    pub failed: Mutex<Option<String>>,
    pub cancel: AtomicBool,
}

impl DlProgress {
    /// Fresh handle, wrapped for sharing.
    pub fn shared() -> Arc<Self> {
        Arc::new(Self {
            written: AtomicU64::new(0),
            total: AtomicU64::new(0),
            finished: AtomicBool::new(false),
            failed: Mutex::new(None),
            cancel: AtomicBool::new(false),
        })
    }

    /// 0.0..=1.0 fraction, or 0.0 when the total is unknown.
    pub fn fraction(&self) -> f32 {
        let total = self.total.load(Ordering::SeqCst);
        if total == 0 {
            return 0.0;
        }
        (self.written.load(Ordering::SeqCst) as f32 / total as f32).clamp(0.0, 1.0)
    }

    /// Human short status.
    pub fn status(&self) -> String {
        if let Some(err) = self.failed.lock().map(|g| g.clone()).unwrap_or(None) {
            return format!("failed: {err}");
        }
        if self.finished.load(Ordering::SeqCst) {
            return String::from("done");
        }
        let written = self.written.load(Ordering::SeqCst) as f64 / 1_048_576.0;
        let total = self.total.load(Ordering::SeqCst);
        if total == 0 {
            return format!("{written:.1} MB");
        }
        format!("{written:.1} / {:.1} MB", total as f64 / 1_048_576.0)
    }
}

/// Spawn a background download thread.
pub fn start_download(url: String, dest: PathBuf, prog: Arc<DlProgress>) {
    std::thread::spawn(move || {
        let result = download_inner(&url, &dest, &prog);
        if let Err(message) = result {
            if let Ok(mut slot) = prog.failed.lock() {
                *slot = Some(message);
            }
        }
        prog.finished.store(true, Ordering::SeqCst);
    });
}

fn download_inner(url: &str, dest: &Path, prog: &DlProgress) -> Result<(), String> {
    use std::io::Read;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    let response = ureq::get(url).call().map_err(|e| format!("GET: {e}"))?;
    if let Some(length) = response.header("Content-Length") {
        if let Ok(total) = length.parse::<u64>() {
            prog.total.store(total, Ordering::SeqCst);
        }
    }
    let mut reader = response.into_reader();
    let mut file = std::fs::File::create(dest).map_err(|e| format!("create: {e}"))?;
    let mut buf = vec![0u8; 65536];
    loop {
        if prog.cancel.load(Ordering::SeqCst) {
            drop(file);
            let _ = std::fs::remove_file(dest);
            return Err(String::from("cancelled"));
        }
        let count = reader.read(&mut buf).map_err(|e| format!("read: {e}"))?;
        if count == 0 {
            break;
        }
        use std::io::Write;
        file.write_all(&buf[..count])
            .map_err(|e| format!("write: {e}"))?;
        prog.written.fetch_add(count as u64, Ordering::SeqCst);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_math() {
        let prog = DlProgress::shared();
        assert_eq!(prog.fraction(), 0.0);
        prog.total.store(100, Ordering::SeqCst);
        prog.written.store(25, Ordering::SeqCst);
        assert!((prog.fraction() - 0.25).abs() < 0.001);
        assert!(prog.status().contains("MB"));
        prog.finished.store(true, Ordering::SeqCst);
        assert_eq!(prog.status(), "done");
    }
}
