use crate::error::{AppError, AppResult};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub fn write_atomic(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::Message("输出路径缺少父目录".into()))?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
    let mut file = fs::File::create(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        AppError::Io(error)
    })
}

pub fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> AppResult<()> {
    write_atomic(path, serde_json::to_string_pretty(value)?.as_bytes())
}

pub fn sha256_file(path: &Path) -> AppResult<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Extends the watcher fingerprint using only the bytes appended since the
/// previous cursor. This is deliberately a hash chain (not a whole-file
/// SHA-256) so an incremental poll never has to reread a long session file.
pub fn extend_hash_chain(previous: &str, appended: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(previous.as_bytes());
    hasher.update(appended);
    format!("sha256-chain:{}", hex::encode(hasher.finalize()))
}

/// Produces a stable comparison key without requiring the path to exist.
/// Existing paths are canonicalized first; Windows keys are case-insensitive.
pub fn normalize_path_key(path: &Path) -> String {
    let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let mut value = resolved.to_string_lossy().replace('\\', "/");
    while value.ends_with('/') && value.len() > 3 {
        value.pop();
    }
    #[cfg(windows)]
    value.make_ascii_lowercase();
    value
}

pub fn copy_directory(source: &Path, destination: &Path) -> AppResult<()> {
    if !source.is_dir() {
        return Err(AppError::Message("源任务包目录不存在".into()));
    }
    fs::create_dir_all(destination)?;
    for entry in walkdir::WalkDir::new(source).follow_links(false) {
        let entry = entry.map_err(|error| AppError::Message(format!("遍历目录失败：{error}")))?;
        let relative = entry
            .path()
            .strip_prefix(source)
            .map_err(|_| AppError::Message("无法计算相对路径".into()))?;
        let target = destination.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

pub fn create_zip(source: &Path, destination: &Path) -> AppResult<()> {
    let parent = destination
        .parent()
        .ok_or_else(|| AppError::Message("Zip 输出路径缺少父目录".into()))?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".{}.zip.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> AppResult<()> {
        let file = fs::File::create(&temporary)?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);
        for entry in walkdir::WalkDir::new(source)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let relative = entry
                .path()
                .strip_prefix(source)
                .map_err(|_| AppError::Message("无法计算 Zip 相对路径".into()))?;
            let name = relative.to_string_lossy().replace('\\', "/");
            zip.start_file(name, options)?;
            let mut input = fs::File::open(entry.path())?;
            std::io::copy(&mut input, &mut zip)?;
        }
        zip.finish()?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    fs::rename(&temporary, destination)?;
    Ok(())
}

pub fn extract_zip(source: &Path, destination: &Path) -> AppResult<PathBuf> {
    fs::create_dir_all(destination)?;
    let file = fs::File::open(source)?;
    let mut archive = zip::ZipArchive::new(file)?;
    for index in 0..archive.len() {
        let mut item = archive.by_index(index)?;
        let enclosed = item
            .enclosed_name()
            .ok_or_else(|| AppError::Message("Zip 包含不安全路径".into()))?
            .to_owned();
        let target = destination.join(enclosed);
        if item.is_dir() {
            fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut output = fs::File::create(target)?;
            std::io::copy(&mut item, &mut output)?;
        }
    }
    if destination.join("manifest.json").exists() {
        return Ok(destination.to_path_buf());
    }
    let children = fs::read_dir(destination)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .collect::<Vec<_>>();
    if children.len() == 1 && children[0].path().join("manifest.json").exists() {
        Ok(children[0].path())
    } else {
        Err(AppError::Message("Zip 根目录中未找到 manifest.json".into()))
    }
}

pub fn is_within(child: &Path, parent: &Path) -> bool {
    match (child.canonicalize(), parent.canonicalize()) {
        (Ok(child), Ok(parent)) => child.starts_with(parent),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sha256_is_real() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("value.txt");
        fs::write(&path, b"agentpack").unwrap();
        assert_eq!(
            sha256_file(&path).unwrap(),
            "26909bd12978c4916e6ab0f87843ffd6bf11e02dbbdb2cc9c2a468210d7d8980"
        );
    }

    #[test]
    fn normalizes_windows_path_case_and_separators() {
        #[cfg(windows)]
        assert_eq!(
            normalize_path_key(Path::new("C:\\Work\\Continuum\\")),
            "c:/work/continuum"
        );
        #[cfg(not(windows))]
        assert_eq!(
            normalize_path_key(Path::new("/tmp/continuum/")),
            "/tmp/continuum"
        );
    }
}
