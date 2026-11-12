use crate::events::TimelineData;
use base64::Engine as _;
use std::ffi::OsString;
use std::io::{self, Read};
use std::os::windows::{ffi::OsStringExt, process::CommandExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use windows_sys::Win32::System::{
    SystemInformation::GetSystemDirectoryW, Threading::CREATE_NO_WINDOW,
};

pub const MAX_FILES: usize = 5;
const COLLECTOR: &str = include_str!("collect.ps1");
const MAX_OUTPUT: u64 = 80 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(90);

pub fn read(files: &[PathBuf]) -> Result<TimelineData, String> {
    if files.len() > MAX_FILES {
        return Err(format!(
            "Bir seferde en fazla {MAX_FILES} .evtx dosyası açılabilir."
        ));
    }
    for path in files {
        if !path.is_absolute()
            || !path.is_file()
            || !path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("evtx"))
        {
            return Err(format!(
                "EVTX dosyası bulunamadı veya geçersiz: {}",
                path.display()
            ));
        }
    }

    let script: Vec<u8> = COLLECTOR
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(script);
    let paths = serde_json::to_string(files).map_err(|error| error.to_string())?;
    let mut child = Command::new(powershell()?)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-EncodedCommand",
        ])
        .arg(encoded)
        .env("BARRACUDA_TIMELINE_FILES", paths)
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Windows olay okuyucusu başlatılamadı · {error}"))?;
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let output = thread::spawn(move || bounded(stdout, MAX_OUTPUT));
    let errors = thread::spawn(move || bounded(stderr, 128 * 1024));
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if started.elapsed() < TIMEOUT => thread::sleep(Duration::from_millis(40)),
            Ok(None) => {
                break Err(
                    "Olaylar 90 saniyede okunamadı. Daha az dosya seçip tekrar dene.".into(),
                );
            }
            Err(error) => break Err(format!("Windows olay okuyucusu izlenemedi · {error}")),
        }
    };
    if status.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let output = output
        .join()
        .map_err(|_| "Olay çıktısı okunamadı.".to_owned())?;
    let errors = errors
        .join()
        .map_err(|_| "Hata çıktısı okunamadı.".to_owned())?;
    let status = status?;
    if !status.success() {
        let errors = errors.map_err(|error| error.to_string())?;
        return Err(format!(
            "Olaylar okunamadı · {}",
            String::from_utf8_lossy(&errors).trim()
        ));
    }
    let output = output.map_err(|error| format!("Olay verisi okunamadı · {error}"))?;
    let mut data: TimelineData = serde_json::from_slice(&output)
        .map_err(|error| format!("Olay verisi geçersiz · {error}"))?;
    if data.opened == 0 {
        return Err(format!(
            "Hiçbir günlük açılamadı. {}",
            data.warnings.join(" · ")
        ));
    }
    data.organize();
    Ok(data)
}

fn bounded(reader: impl Read, limit: u64) -> io::Result<Vec<u8>> {
    let mut data = Vec::new();
    reader.take(limit + 1).read_to_end(&mut data)?;
    if data.len() as u64 > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Çıktı sınırı aşıldı. Daha az dosya seç.",
        ));
    }
    Ok(data)
}

fn powershell() -> Result<PathBuf, String> {
    let mut path = [0_u16; 1024];
    // GetSystemDirectoryW writes a NUL-terminated system path into the supplied buffer.
    let size = unsafe { GetSystemDirectoryW(path.as_mut_ptr(), path.len() as u32) } as usize;
    if size == 0 || size >= path.len() {
        return Err("Windows sistem klasörü bulunamadı.".into());
    }
    let system = OsString::from_wide(&path[..size]);
    Ok(Path::new(&system).join("WindowsPowerShell/v1.0/powershell.exe"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_live_channels() {
        let data = read(&[]).unwrap();
        assert!(data.opened >= 2);
        assert!(
            data.events
                .windows(2)
                .all(|pair| pair[0].time <= pair[1].time)
        );
    }

    #[test]
    fn reads_exported_logs_when_fixtures_are_supplied() {
        let Some(input) = std::env::var_os("BARRACUDA_TIMELINE_TEST_FILES") else {
            return;
        };
        let files: Vec<_> = std::env::split_paths(&input).collect();
        let data = read(&files).unwrap();
        assert_eq!(data.opened, files.len(), "{:?}", data.warnings);
        assert!(
            data.events
                .iter()
                .any(|event| event.kind() == crate::events::Kind::Crash)
        );
        assert!(
            data.last_boot().is_some(),
            "export should contain a boot marker"
        );
        assert!(data.events.iter().enumerate().any(|(anchor, marker)| {
            marker.boot_marker()
                && data.events.iter().any(|event| {
                    event.kind() == crate::events::Kind::Crash && data.near(event, anchor)
                })
        }));
        assert!(data.events.iter().any(|event| !event.xml.is_empty()));
    }
}
