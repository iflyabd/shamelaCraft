//! Pure-Rust archive extraction (ZIP / TAR / TAR.GZ / RAR) for book collections.
//!
//! Downloaded collection archives are unpacked locally, then every `.bok`
//! inside is collected for import. Path traversal is rejected for all
//! formats (`zip`/`tar` enforce their own rules; RAR entries are sanitized
//! by hand). RAR filenames fall back to Windows-1256 decoding, matching the
//! legacy Arabic tooling.

use crate::{err, Result};
use std::path::{Path, PathBuf};

/// Archive kind by file name, if natively supported.
pub fn archive_kind(name: &str) -> Option<&'static str> {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".zip") {
        Some("zip")
    } else if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
        Some("targz")
    } else if lower.ends_with(".tar") {
        Some("tar")
    } else if lower.ends_with(".rar") {
        Some("rar")
    } else {
        None
    }
}

/// True for directly importable books or natively extractable archives.
pub fn is_importable(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".bok") || archive_kind(&lower).is_some()
}

/// Extract `path` into `dest`, returning every `.bok` discovered inside.
pub fn extract_archive(path: &Path, dest: &Path) -> Result<Vec<PathBuf>> {
    let name = path
        .file_name()
        .and_then(|part| part.to_str())
        .unwrap_or("");
    match archive_kind(name) {
        Some("zip") => extract_zip(path, dest),
        Some("targz") => extract_tar_gz(path, dest),
        Some("tar") => extract_tar(path, dest),
        Some("rar") => extract_rar(path, dest),
        _ => Err(err(format!("unsupported archive: {name}"))),
    }
}

fn collect_bok(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let entries = std::fs::read_dir(dir).map_err(|error| err(format!("list dir: {error}")))?;
    for entry in entries {
        let path = entry
            .map_err(|error| err(format!("list dir: {error}")))?
            .path();
        if path.is_dir() {
            collect_bok(&path, out)?;
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("bok"))
        {
            out.push(path);
        }
    }
    Ok(())
}

fn extract_zip(path: &Path, dest: &Path) -> Result<Vec<PathBuf>> {
    let file = std::fs::File::open(path).map_err(|error| err(format!("open zip: {error}")))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|error| err(format!("read zip: {error}")))?;
    archive
        .extract(dest)
        .map_err(|error| err(format!("extract zip: {error}")))?;
    let mut out = Vec::new();
    collect_bok(dest, &mut out)?;
    Ok(out)
}

fn extract_tar_gz(path: &Path, dest: &Path) -> Result<Vec<PathBuf>> {
    let file = std::fs::File::open(path).map_err(|error| err(format!("open archive: {error}")))?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    archive
        .unpack(dest)
        .map_err(|error| err(format!("extract tar.gz: {error}")))?;
    let mut out = Vec::new();
    collect_bok(dest, &mut out)?;
    Ok(out)
}

fn extract_tar(path: &Path, dest: &Path) -> Result<Vec<PathBuf>> {
    let file = std::fs::File::open(path).map_err(|error| err(format!("open archive: {error}")))?;
    let mut archive = tar::Archive::new(file);
    archive
        .unpack(dest)
        .map_err(|error| err(format!("extract tar: {error}")))?;
    let mut out = Vec::new();
    collect_bok(dest, &mut out)?;
    Ok(out)
}

fn leak_error(message: String) -> rars::Error {
    rars::Error::WriterFailure(Box::leak(message.into_boxed_str()))
}

/// Reject absolute paths and `..` escapes; normalize `\` separators.
fn sanitize_entry(raw: &str) -> Option<PathBuf> {
    if Path::new(raw).is_absolute() {
        return None;
    }
    let mut out = PathBuf::new();
    for part in raw.replace('\\', "/").split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return None;
        }
        out.push(part);
    }
    if out.as_os_str().is_empty() {
        return None;
    }
    Some(out)
}

fn decode_name(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec())
        .unwrap_or_else(|_| encoding_rs::WINDOWS_1256.decode(bytes).0.into_owned())
}

fn extract_rar(path: &Path, dest: &Path) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dest).map_err(|error| err(format!("mkdir: {error}")))?;
    let file = std::fs::File::open(path).map_err(|error| err(format!("open rar: {error}")))?;
    let archive = rars::ArchiveReader::read_reader(file)
        .map_err(|error| err(format!("read rar: {error}")))?;
    archive
        .extract_to(None, |meta| {
            let name = decode_name(meta.name_bytes());
            let Some(safe) = sanitize_entry(&name) else {
                return Err(leak_error(format!("unsafe entry path: {name}")));
            };
            let target = dest.join(safe);
            if meta.is_directory {
                std::fs::create_dir_all(&target)
                    .map_err(|error| leak_error(format!("mkdir: {error}")))?;
                return Ok(Box::new(std::io::sink()) as Box<dyn std::io::Write>);
            }
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| leak_error(format!("mkdir: {error}")))?;
            }
            let file = std::fs::File::create(&target)
                .map_err(|error| leak_error(format!("create file: {error}")))?;
            Ok(Box::new(file) as Box<dyn std::io::Write>)
        })
        .map_err(|error| err(format!("extract rar: {error}")))?;
    let mut out = Vec::new();
    collect_bok(dest, &mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn kinds() {
        assert_eq!(archive_kind("a.ZIP"), Some("zip"));
        assert_eq!(archive_kind("a.tar.gz"), Some("targz"));
        assert_eq!(archive_kind("a.tgz"), Some("targz"));
        assert_eq!(archive_kind("a.tar"), Some("tar"));
        assert_eq!(archive_kind("a.RAR"), Some("rar"));
        assert_eq!(archive_kind("a.bok"), None);
        assert_eq!(archive_kind("a.iso"), None);
        assert!(is_importable("a.bok"));
        assert!(is_importable("a.rar"));
        assert!(!is_importable("a.iso"));
    }

    #[test]
    fn sanitizer() {
        assert!(sanitize_entry("/abs/path").is_none());
        assert!(sanitize_entry("../escape").is_none());
        assert!(sanitize_entry("a/../../b").is_none());
        assert_eq!(
            sanitize_entry("dir\\book.bok"),
            Some(PathBuf::from("dir/book.bok"))
        );
    }

    #[test]
    fn zip_round_trip_collects_bok() {
        let dir = std::env::temp_dir().join(format!("sc-extract-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let zip_path = dir.join("test.zip");
        {
            let file = std::fs::File::create(&zip_path).unwrap();
            let mut writer = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            writer.start_file("nested/book.bok", options).unwrap();
            writer.write_all(b"fake").unwrap();
            writer.start_file("notes.txt", options).unwrap();
            writer.write_all(b"hi").unwrap();
            writer.finish().unwrap();
        }
        let out = extract_archive(&zip_path, &dir.join("unpacked")).unwrap();
        assert_eq!(out.len(), 1);
        assert!(out[0].ends_with("nested/book.bok"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
