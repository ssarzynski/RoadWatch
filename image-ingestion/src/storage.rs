use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct LocalEvidenceStore {
    quarantine_root: PathBuf,
    sanitized_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredObject {
    /// Opaque key relative to the configured trust-zone root.
    pub object_key: String,
}

impl LocalEvidenceStore {
    pub fn new(quarantine_root: impl Into<PathBuf>, sanitized_root: impl Into<PathBuf>) -> io::Result<Self> {
        let store = Self {
            quarantine_root: quarantine_root.into(),
            sanitized_root: sanitized_root.into(),
        };
        ensure_private_dir(&store.quarantine_root)?;
        ensure_private_dir(&store.sanitized_root)?;
        Ok(store)
    }

    /// Stores hostile original bytes under a server-generated random name.
    /// No user filename is accepted by this API.
    pub fn put_quarantine(&self, bytes: &[u8]) -> io::Result<StoredObject> {
        let key = Uuid::new_v4().to_string();
        atomic_write(&self.quarantine_root, &key, bytes)?;
        Ok(StoredObject { object_key: key })
    }

    /// Stores a sanitized derivative using its lowercase SHA-256 as content identity.
    pub fn put_sanitized(&self, sha256: &str, bytes: &[u8]) -> io::Result<StoredObject> {
        if !valid_sha256_hex(sha256) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid sanitized SHA-256"));
        }
        let key = sha256.to_ascii_lowercase();
        let final_path = self.sanitized_root.join(&key);
        if final_path.exists() {
            return Ok(StoredObject { object_key: key });
        }
        atomic_write(&self.sanitized_root, &key, bytes)?;
        Ok(StoredObject { object_key: key })
    }

    pub fn list_quarantine(&self) -> io::Result<Vec<(String, std::time::SystemTime)>> {
        list_objects(&self.quarantine_root, valid_uuid_key)
    }

    pub fn list_sanitized(&self) -> io::Result<Vec<(String, std::time::SystemTime)>> {
        list_objects(&self.sanitized_root, valid_sha256_hex)
    }

    pub fn delete_sanitized(&self, object_key: &str) -> io::Result<()> {
        if !valid_sha256_hex(object_key) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid sanitized key"));
        }
        match fs::remove_file(self.sanitized_root.join(object_key)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }

    pub fn delete_quarantine(&self, object_key: &str) -> io::Result<()> {
        if !valid_uuid_key(object_key) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid quarantine key"));
        }
        match fs::remove_file(self.quarantine_root.join(object_key)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }
}


fn list_objects(root: &Path, validator: fn(&str) -> bool) -> io::Result<Vec<(String, std::time::SystemTime)>> {
    let mut objects = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else { continue };
        if !validator(&name) {
            continue;
        }
        let modified = entry.metadata()?.modified()?;
        objects.push((name, modified));
    }
    Ok(objects)
}

fn atomic_write(root: &Path, key: &str, bytes: &[u8]) -> io::Result<()> {
    let tmp_name = format!(".{}.{}.tmp", key, Uuid::new_v4());
    let tmp = root.join(tmp_name);
    let final_path = root.join(key);

    let result = (|| {
        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut file = opts.open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, &final_path)?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn ensure_private_dir(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn valid_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn valid_uuid_key(value: &str) -> bool {
    Uuid::parse_str(value).is_ok() && !value.contains('/') && !value.contains('\\')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_roots() -> (PathBuf, PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("roadwatch-storage-test-{}", Uuid::new_v4()));
        (base.join("quarantine"), base.join("sanitized"), base)
    }

    #[test]
    fn quarantine_key_is_server_generated_and_opaque() {
        let (q, s, base) = temp_roots();
        let store = LocalEvidenceStore::new(&q, &s).unwrap();
        let object = store.put_quarantine(b"hostile-original").unwrap();
        assert!(Uuid::parse_str(&object.object_key).is_ok());
        assert_eq!(fs::read(q.join(&object.object_key)).unwrap(), b"hostile-original");
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn sanitized_key_requires_sha256_shape() {
        let (q, s, base) = temp_roots();
        let store = LocalEvidenceStore::new(&q, &s).unwrap();
        assert!(store.put_sanitized("../escape", b"x").is_err());
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn sanitized_storage_is_content_addressed() {
        let (q, s, base) = temp_roots();
        let store = LocalEvidenceStore::new(&q, &s).unwrap();
        let hash = "a".repeat(64);
        let object = store.put_sanitized(&hash, b"clean").unwrap();
        assert_eq!(object.object_key, hash);
        assert_eq!(fs::read(s.join(&object.object_key)).unwrap(), b"clean");
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn quarantine_delete_rejects_path_traversal() {
        let (q, s, base) = temp_roots();
        let store = LocalEvidenceStore::new(&q, &s).unwrap();
        assert!(store.delete_quarantine("../../etc/passwd").is_err());
        fs::remove_dir_all(base).unwrap();
    }
}
