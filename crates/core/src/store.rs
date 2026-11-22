// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Synced before the rename, so a power cut never leaves a renamed but empty file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".tmp.{}.{n}", std::process::id()));
    let tmp = PathBuf::from(tmp);
    let res = (|| {
        let mut f = File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)
    })();
    if res.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    res
}

pub fn aside_name(path: &Path, suffix: &str) -> PathBuf {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let base = format!("{}{suffix}.{secs}", path.as_os_str().to_string_lossy());
    let mut name = PathBuf::from(&base);
    let mut n = 1;
    while name.exists() {
        name = PathBuf::from(format!("{base}-{n}"));
        n += 1;
    }
    name
}

pub fn set_aside(path: &Path, suffix: &str) -> io::Result<PathBuf> {
    let to = aside_name(path, suffix);
    // A hard link refuses an existing target; the rename after it is then only a remove.
    match std::fs::hard_link(path, &to) {
        Ok(()) => std::fs::remove_file(path).map(|_| to),
        Err(_) => {
            std::fs::copy(path, &to)?;
            std::fs::remove_file(path).map(|_| to)
        }
    }
}

pub fn copy_aside(path: &Path, suffix: &str) -> io::Result<PathBuf> {
    let to = aside_name(path, suffix);
    let mut src = File::open(path)?;
    let mut dst = OpenOptions::new().write(true).create_new(true).open(&to)?;
    io::copy(&mut src, &mut dst)?;
    dst.sync_all()?;
    Ok(to)
}

pub struct Lock(#[allow(dead_code)] File);

impl Lock {
    pub fn try_acquire(dir: &Path) -> Option<Lock> {
        std::fs::create_dir_all(dir).ok()?;
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.join("store.lock"))
            .ok()?;
        f.try_lock().ok()?;
        Some(Lock(f))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../_temp/store-tests")
            .join(name);
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn concurrent_writers_leave_one_whole_file() {
        let d = scratch("concurrent");
        let path = d.join("history.bin");
        let payloads: Vec<Vec<u8>> = (0..8u8).map(|i| vec![i; 256 * 1024]).collect();
        std::thread::scope(|s| {
            for p in &payloads {
                let path = &path;
                s.spawn(move || {
                    for _ in 0..20 {
                        // A rename onto a target another writer is renaming may fail on Windows; never corrupt.
                        let _ = write_atomic(path, p);
                    }
                });
            }
        });
        let got = std::fs::read(&path).unwrap();
        assert!(payloads.contains(&got), "file is not any writer's payload");
        let left: Vec<_> = std::fs::read_dir(&d).unwrap().flatten().collect();
        assert_eq!(left.len(), 1, "temp files left behind");
    }

    #[test]
    fn second_lock_on_a_store_is_refused() {
        let d = scratch("lock");
        let a = Lock::try_acquire(&d);
        assert!(a.is_some());
        assert!(Lock::try_acquire(&d).is_none());
        drop(a);
        assert!(Lock::try_acquire(&d).is_some());
    }
}
