//! Local document storage, separate from UI and JSON validation.

use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::SystemTime,
};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub struct DocumentStore {
    directory: PathBuf,
}

impl DocumentStore {
    pub fn in_user_directory() -> io::Result<Self> {
        let home = dirs::home_dir().ok_or_else(|| io::Error::other("无法找到用户主目录"))?;
        Self::new(home.join(".json-editor").join("documents"))
    }

    pub fn new(directory: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&directory)?;
        Ok(Self { directory })
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn list(&self) -> io::Result<Vec<String>> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(&self.directory)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if name.ends_with(".json") {
                entries.push((metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH), name));
            }
        }
        entries.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
        Ok(entries.into_iter().map(|(_, name)| name).collect())
    }

    pub fn read(&self, name: &str) -> io::Result<String> {
        let path = self.existing_path(name)?;
        fs::read_to_string(path)
    }

    pub fn create(&self, source: &str) -> io::Result<String> {
        self.create_at(
            source,
            &chrono::Local::now().format("%Y-%m-%d %H-%M-%S").to_string(),
        )
    }

    fn create_at(&self, source: &str, timestamp: &str) -> io::Result<String> {
        let temp = self.write_temp(source)?;
        for suffix in 0..10_000 {
            let name = if suffix == 0 {
                format!("{timestamp}.json")
            } else {
                format!("{timestamp}-{suffix}.json")
            };
            let path = self.path(&name)?;
            // Publishing a fully written file through a hard link never overwrites
            // an existing document, including one created by another instance.
            match fs::hard_link(&temp.0, path) {
                Ok(()) => return Ok(name),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::other("同一时间的文档过多，请稍后重试"))
    }

    pub fn save(&self, name: &str, source: &str) -> io::Result<()> {
        let path = self.existing_path(name)?;
        let temp = self.write_temp(source)?;
        // The original remains intact if writing or replacement fails.
        fs::rename(&temp.0, path)
    }

    pub fn rename(&self, old_name: &str, requested: &str) -> io::Result<String> {
        let name = normalized_name(requested)?;
        let old_path = self.existing_path(old_name)?;
        if name == old_name {
            return Ok(name);
        }
        let new_path = self.path(&name)?;
        // Unlike rename(), hard_link() refuses to replace a destination.
        fs::hard_link(&old_path, &new_path)?;
        if let Err(error) = fs::remove_file(&old_path) {
            let _ = fs::remove_file(&new_path);
            return Err(error);
        }
        Ok(name)
    }

    fn path(&self, name: &str) -> io::Result<PathBuf> {
        if normalized_name(name)? != name {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "非法文档名称"));
        }
        Ok(self.directory.join(name))
    }

    fn existing_path(&self, name: &str) -> io::Result<PathBuf> {
        let path = self.path(name)?;
        if !fs::symlink_metadata(&path)?.file_type().is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "文档不是普通文件",
            ));
        }
        Ok(path)
    }

    fn write_temp(&self, source: &str) -> io::Result<TemporaryFile> {
        for _ in 0..100 {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = self
                .directory
                .join(format!(".save-{}-{sequence}.tmp", std::process::id()));
            let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            };
            let temp = TemporaryFile(path);
            file.write_all(source.as_bytes())?;
            file.sync_all()?;
            return Ok(temp);
        }
        Err(io::Error::other("无法创建临时保存文件"))
    }
}

struct TemporaryFile(PathBuf);

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn normalized_name(requested: &str) -> io::Result<String> {
    if requested.chars().any(char::is_control) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "名称包含控制字符",
        ));
    }
    let requested = requested.trim();
    let stem = requested.strip_suffix(".json").unwrap_or(requested);
    let reserved = stem.split('.').next().unwrap_or("").to_ascii_uppercase();
    if stem.is_empty()
        || stem == "."
        || stem == ".."
        || stem.ends_with(['.', ' '])
        || stem
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        || matches!(reserved.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (reserved.len() == 4
            && (reserved.starts_with("COM") || reserved.starts_with("LPT"))
            && matches!(reserved.as_bytes()[3], b'1'..=b'9'))
        || stem.len() > 200
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "名称为空、过长或包含非法字符",
        ));
    }
    Ok(format!("{stem}.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(DocumentStore);

    impl Fixture {
        fn new() -> Self {
            let unique = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let directory = std::env::temp_dir().join(format!(
                "json-editor-documents-test-{}-{unique}",
                std::process::id()
            ));
            Self(DocumentStore::new(directory).unwrap())
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(self.0.directory());
        }
    }

    #[test]
    fn create_collision_save_and_reload_preserve_exact_text() {
        let fixture = Fixture::new();
        let store = &fixture.0;
        assert!(store.list().unwrap().is_empty());
        let stamp = "2026-10-13 12-34-56";
        let first = store.create_at("", stamp).unwrap();
        let second = store.create_at("{草稿", stamp).unwrap();
        assert_eq!(first, "2026-10-13 12-34-56.json");
        assert_eq!(second, "2026-10-13 12-34-56-1.json");
        assert_eq!(store.read(&first).unwrap(), "");
        assert_eq!(store.read(&second).unwrap(), "{草稿");
        let source = "{\n  \"你好\": 123456789012345678901234567890\n}\n";
        store.save(&first, source).unwrap();
        let reopened = DocumentStore::new(store.directory().to_owned()).unwrap();
        assert_eq!(reopened.read(&first).unwrap(), source);
        assert_eq!(reopened.list().unwrap().len(), 2);
        assert_eq!(fs::read_dir(store.directory()).unwrap().count(), 2);
    }

    #[test]
    fn rename_preserves_content_and_refuses_collisions() {
        let fixture = Fixture::new();
        let store = &fixture.0;
        let first = store.create_at("first", "2026-10-13 12-34-56").unwrap();
        let second = store.create_at("second", "2026-10-13 12-34-56").unwrap();
        let renamed = store.rename(&first, "  工作文档  ").unwrap();
        assert_eq!(renamed, "工作文档.json");
        assert_eq!(store.read(&renamed).unwrap(), "first");
        assert!(store.read(&first).is_err());
        assert!(store.rename(&second, &renamed).is_err());
        assert_eq!(store.read(&second).unwrap(), "second");
        assert_eq!(store.read(&renamed).unwrap(), "first");
        assert_eq!(store.rename(&renamed, &renamed).unwrap(), renamed);
    }

    #[test]
    fn rejects_unsafe_names_and_missing_documents() {
        for name in [
            "",
            ".json",
            "..",
            "../outside",
            "a/b",
            "a\\b",
            "a:",
            "a?",
            "a\n",
            "CON",
            "nul.json",
            "COM1",
            "a..json",
        ] {
            assert!(normalized_name(name).is_err(), "{name:?}");
        }
        let fixture = Fixture::new();
        assert!(fixture.0.save("missing.json", "{}").is_err());
        assert!(fixture.0.read("../outside.json").is_err());
        assert!(fixture.0.list().unwrap().is_empty());
    }

    #[test]
    fn history_excludes_temporary_files_and_directories() {
        let fixture = Fixture::new();
        fs::write(fixture.0.directory().join(".save.tmp"), "pending").unwrap();
        fs::create_dir(fixture.0.directory().join("folder.json")).unwrap();
        let name = fixture.0.create("{}").unwrap();
        assert_eq!(fixture.0.list().unwrap(), vec![name]);
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_not_documents() {
        let fixture = Fixture::new();
        fs::write(fixture.0.directory().join("source.txt"), "outside").unwrap();
        std::os::unix::fs::symlink("source.txt", fixture.0.directory().join("link.json")).unwrap();
        assert!(fixture.0.list().unwrap().is_empty());
        assert!(fixture.0.read("link.json").is_err());
        assert!(fixture.0.save("link.json", "changed").is_err());
        assert!(fixture.0.rename("link.json", "renamed").is_err());
    }
}
