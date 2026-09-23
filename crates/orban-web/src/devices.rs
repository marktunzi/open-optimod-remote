use orban_protocol::adapter::DeviceModel;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, io::Write, net::IpAddr, path::PathBuf};
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedDevice {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub terminal_port: u16,
    pub has_code: bool,
    #[serde(default)]
    pub model: DeviceModel,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditDevice {
    pub id: Option<String>,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub terminal_port: u16,
    #[serde(default)]
    pub model: DeviceModel,
}
#[derive(Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct DiskBook {
    version: u8,
    devices: Vec<SavedDevice>,
}
pub trait Vault: Send + Sync {
    fn get(&self, account: &str) -> Result<Zeroizing<String>, String>;
    fn set(&self, account: &str, code: &str) -> Result<(), String>;
    fn delete(&self, account: &str) -> Result<(), String>;
}

#[derive(Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct DiskVault {
    version: u8,
    credentials: HashMap<String, String>,
}

pub struct FileVault {
    path: PathBuf,
}

impl FileVault {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn read(&self) -> Result<HashMap<String, String>, String> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
            Err(_) => return Err("Cannot read saved access codes".into()),
        };
        if bytes.len() > 1_048_576 {
            return Err("Saved access-code file is too large".into());
        }
        let disk: DiskVault =
            serde_json::from_slice(&bytes).map_err(|_| "Saved access-code file is damaged")?;
        if disk.version != 1 || disk.credentials.len() > 256 {
            return Err("Unsupported saved access-code file".into());
        }
        Ok(disk.credentials)
    }

    fn write(&self, credentials: HashMap<String, String>) -> Result<(), String> {
        let directory = self.path.parent().ok_or("Invalid credential directory")?;
        fs::create_dir_all(directory).map_err(|_| "Cannot create the credential directory")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
                .map_err(|_| "Cannot protect the credential directory")?;
        }
        let bytes = serde_json::to_vec(&DiskVault {
            version: 1,
            credentials,
        })
        .map_err(|_| "Cannot encode saved access codes")?;
        let mut file = tempfile::NamedTempFile::new_in(directory)
            .map_err(|_| "Cannot create the credential file")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.as_file()
                .set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|_| "Cannot protect the credential file")?;
        }
        file.write_all(&bytes)
            .and_then(|_| file.as_file().sync_all())
            .map_err(|_| "Cannot write saved access codes")?;
        file.persist(&self.path)
            .map_err(|_| "Cannot save access codes")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600))
                .map_err(|_| "Cannot protect saved access codes")?;
        }
        Ok(())
    }
}

impl Vault for FileVault {
    fn get(&self, account: &str) -> Result<Zeroizing<String>, String> {
        self.read()?
            .remove(account)
            .map(Zeroizing::new)
            .ok_or("Enter the access code".into())
    }

    fn set(&self, account: &str, code: &str) -> Result<(), String> {
        let mut credentials = self.read()?;
        credentials.insert(account.into(), code.into());
        self.write(credentials)
    }

    fn delete(&self, account: &str) -> Result<(), String> {
        let mut credentials = self.read()?;
        credentials.remove(account);
        self.write(credentials)
    }
}
fn account(device: &SavedDevice) -> String {
    format!(
        "{}@{}:{}:{}",
        device.id, device.host, device.port, device.terminal_port
    )
}
fn validate(name: &str, host: &str, port: u16, terminal_port: u16) -> Result<(), String> {
    if name.trim().is_empty() || name.len() > 80 || name.chars().any(char::is_control) {
        return Err("Enter a name between 1 and 80 characters".into());
    }
    host.parse::<IpAddr>()
        .map_err(|_| "Enter a valid IP address")?;
    if port == 0 || terminal_port == 0 {
        return Err("Ports must be between 1 and 65535".into());
    }
    Ok(())
}
pub fn data_file() -> Result<PathBuf, String> {
    if let Some(dir) = std::env::var_os("OPTIMOD_DATA_DIR") {
        return Ok(PathBuf::from(dir).join("connections.json"));
    }
    let home = std::env::var_os("HOME").ok_or("User data directory is unavailable")?;
    #[cfg(target_os = "macos")]
    let dir = PathBuf::from(home).join("Library/Application Support/OpenOptimodRemote");
    #[cfg(not(target_os = "macos"))]
    let dir = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(home).join(".local/share"))
        .join("open-optimod-remote");
    Ok(dir.join("connections.json"))
}

pub fn credential_file() -> Result<PathBuf, String> {
    Ok(data_file()?.with_file_name("credentials.json"))
}
pub struct Book {
    path: PathBuf,
    devices: Vec<SavedDevice>,
    vault: Box<dyn Vault>,
}
impl Book {
    pub fn open(path: PathBuf, vault: Box<dyn Vault>) -> Result<Self, String> {
        let (devices, migrate) = match fs::read(&path) {
            Ok(bytes) => {
                if bytes.len() > 1_048_576 {
                    return Err("Connection file is too large".into());
                }
                let disk: DiskBook = serde_json::from_slice(&bytes)
                    .map_err(|_| "Connection file is damaged; it has not been overwritten")?;
                if !matches!(disk.version, 1 | 2) || disk.devices.len() > 256 {
                    return Err("Unsupported connection file".into());
                }
                for (i, d) in disk.devices.iter().enumerate() {
                    validate(&d.name, &d.host, d.port, d.terminal_port)?;
                    Uuid::parse_str(&d.id).map_err(|_| "Invalid connection identifier")?;
                    if disk.devices[..i].iter().any(|x| x.id == d.id) {
                        return Err("Duplicate connection identifier".into());
                    }
                }
                (disk.devices, disk.version == 1)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Vec::new(), false),
            Err(_) => return Err("Cannot read the connection file".into()),
        };
        let mut book = Self {
            path,
            devices,
            vault,
        };
        if migrate {
            book.persist(book.devices.clone())?;
        }
        Ok(book)
    }
    pub fn list(&self) -> Vec<SavedDevice> {
        self.devices.clone()
    }
    pub fn find(&self, id: &str) -> Result<SavedDevice, String> {
        self.devices
            .iter()
            .find(|d| d.id == id)
            .cloned()
            .ok_or("This connection no longer exists".into())
    }
    fn persist(&mut self, devices: Vec<SavedDevice>) -> Result<(), String> {
        let dir = self.path.parent().ok_or("Invalid data directory")?;
        fs::create_dir_all(dir).map_err(|_| "Cannot create the data directory")?;
        let mut file = tempfile::NamedTempFile::new_in(dir)
            .map_err(|_| "Cannot create the connection file")?;
        let bytes = serde_json::to_vec_pretty(&DiskBook {
            version: 2,
            devices: devices.clone(),
        })
        .map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|_| file.as_file().sync_all())
            .map_err(|_| "Cannot write the connection file")?;
        file.persist(&self.path)
            .map_err(|_| "Cannot save the connection file")?;
        self.devices = devices;
        Ok(())
    }
    pub fn save(&mut self, edit: EditDevice) -> Result<SavedDevice, String> {
        let host = edit
            .host
            .trim()
            .parse::<IpAddr>()
            .map_err(|_| "Enter a valid IP address")?
            .to_string();
        validate(&edit.name, &host, edit.port, edit.terminal_port)?;
        let old = edit.id.as_deref().map(|id| self.find(id)).transpose()?;
        if old.is_none() && self.devices.len() == 256 {
            return Err("Up to 256 connections can be saved".into());
        }
        let mut device = SavedDevice {
            id: old
                .as_ref()
                .map(|d| d.id.clone())
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            name: edit.name.trim().into(),
            host,
            port: edit.port,
            terminal_port: edit.terminal_port,
            has_code: old.as_ref().is_some_and(|d| d.has_code),
            model: edit.model,
        };
        if let Some(old) = old
            .as_ref()
            .filter(|d| account(d) != account(&device) && d.has_code)
        {
            self.vault.delete(&account(old))?;
            device.has_code = false;
        }
        let mut devices = self.devices.clone();
        if let Some(i) = devices.iter().position(|d| d.id == device.id) {
            devices[i] = device.clone();
        } else {
            devices.push(device.clone());
        }
        self.persist(devices)?;
        Ok(device)
    }
    pub fn set_code(&mut self, id: &str, code: Option<Zeroizing<String>>) -> Result<(), String> {
        let device = self.find(id)?;
        match &code {
            Some(code) => {
                let _ = Zeroizing::new(orban_protocol::auth::login_request(code)?);
                self.vault.set(&account(&device), code)?;
            }
            None => self.vault.delete(&account(&device))?,
        }
        let mut devices = self.devices.clone();
        devices.iter_mut().find(|d| d.id == id).unwrap().has_code = code.is_some();
        self.persist(devices)
    }
    pub fn code(&mut self, id: &str) -> Result<Zeroizing<String>, String> {
        let device = self.find(id)?;
        if !device.has_code {
            return Err("Enter the access code".into());
        }
        match self.vault.get(&account(&device)) {
            Ok(code) => Ok(code),
            Err(error) => {
                let mut devices = self.devices.clone();
                devices
                    .iter_mut()
                    .find(|entry| entry.id == id)
                    .unwrap()
                    .has_code = false;
                self.persist(devices)?;
                Err(error)
            }
        }
    }
    pub fn delete(&mut self, id: &str) -> Result<(), String> {
        let device = self.find(id)?;
        if device.has_code {
            self.vault.delete(&account(&device))?;
        }
        self.persist(
            self.devices
                .iter()
                .filter(|d| d.id != id)
                .cloned()
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orban_protocol::adapter::DeviceModel;
    use std::sync::Mutex;
    #[derive(Default)]
    struct MemoryVault(Mutex<std::collections::HashMap<String, String>>);
    impl Vault for MemoryVault {
        fn get(&self, id: &str) -> Result<Zeroizing<String>, String> {
            self.0
                .lock()
                .unwrap()
                .get(id)
                .cloned()
                .map(Zeroizing::new)
                .ok_or("Missing".into())
        }
        fn set(&self, id: &str, code: &str) -> Result<(), String> {
            self.0.lock().unwrap().insert(id.into(), code.into());
            Ok(())
        }
        fn delete(&self, id: &str) -> Result<(), String> {
            self.0.lock().unwrap().remove(id);
            Ok(())
        }
    }
    fn edit(name: &str, host: &str) -> EditDevice {
        EditDevice {
            id: None,
            name: name.into(),
            host: host.into(),
            port: 6201,
            terminal_port: 23,
            model: DeviceModel::Auto,
        }
    }
    #[test]
    fn independent_connections_survive_restart_and_secrets_never_enter_json() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("connections.json");
        let mut book = Book::open(path.clone(), Box::<MemoryVault>::default()).unwrap();
        let a = book.save(edit("Studio", "203.0.113.10")).unwrap();
        let b = book.save(edit("Reserve", "203.0.113.11")).unwrap();
        book.set_code(&a.id, Some(Zeroizing::new("TESTCODE9876".into())))
            .unwrap();
        assert_eq!(&**book.code(&a.id).unwrap(), "TESTCODE9876");
        assert!(book.code(&b.id).is_err());
        let disk = fs::read_to_string(&path).unwrap();
        assert!(!disk.contains("TESTCODE9876"));
        let mut reloaded = Book::open(path, Box::<MemoryVault>::default()).unwrap();
        assert_eq!(reloaded.list().len(), 2);
        assert!(reloaded.list()[0].has_code);
        assert!(reloaded.code(&a.id).is_err());
        assert!(!reloaded.list()[0].has_code);
        assert_eq!(reloaded.find(&b.id).unwrap().host, "203.0.113.11");
        book.delete(&b.id).unwrap();
        assert_eq!(book.list().len(), 1);
    }
    #[test]
    fn changing_endpoint_forgets_code_but_renaming_keeps_it() {
        let temp = tempfile::tempdir().unwrap();
        let mut book =
            Book::open(temp.path().join("c.json"), Box::<MemoryVault>::default()).unwrap();
        let a = book.save(edit("Studio", "203.0.113.10")).unwrap();
        book.set_code(&a.id, Some(Zeroizing::new("TESTCODE9876".into())))
            .unwrap();
        let mut e = edit("New name", "203.0.113.10");
        e.id = Some(a.id.clone());
        assert!(book.save(e).unwrap().has_code);
        let mut e = edit("New name", "203.0.113.11");
        e.id = Some(a.id.clone());
        assert!(!book.save(e).unwrap().has_code);
        assert!(book.code(&a.id).is_err());
    }
    #[test]
    fn invalid_edits_and_corrupt_files_are_not_silently_saved() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("c.json");
        let mut book = Book::open(path.clone(), Box::<MemoryVault>::default()).unwrap();
        assert!(book.save(edit("", "203.0.113.10")).is_err());
        assert!(book.save(edit("X", "foreign.invalid")).is_err());
        let mut e = edit("X", "203.0.113.10");
        e.port = 0;
        assert!(book.save(e).is_err());
        assert!(!path.exists());
        fs::write(&path, b"damaged").unwrap();
        assert!(Book::open(path.clone(), Box::<MemoryVault>::default()).is_err());
        assert_eq!(fs::read(path).unwrap(), b"damaged");
    }

    #[test]
    fn file_vault_survives_restart_and_deletes_credentials() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("private").join("credentials.json");
        let vault = FileVault::new(path.clone());
        vault.set("studio", "TESTCODE9876").unwrap();
        drop(vault);

        let reloaded = FileVault::new(path);
        assert_eq!(&**reloaded.get("studio").unwrap(), "TESTCODE9876");
        reloaded.delete("studio").unwrap();
        assert!(reloaded.get("studio").is_err());
    }

    #[test]
    fn version_one_connections_migrate_to_auto_model_without_losing_identity() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("connections.json");
        let id = Uuid::new_v4().to_string();
        fs::write(
            &path,
            format!(
                r#"{{"version":1,"devices":[{{"id":"{id}","name":"Studio","host":"192.0.2.10","port":6201,"terminal_port":23,"has_code":false}}]}}"#
            ),
        )
        .unwrap();

        let book = Book::open(path.clone(), Box::<MemoryVault>::default()).unwrap();
        let device = book.find(&id).unwrap();
        assert_eq!(device.model, DeviceModel::Auto);
        let migrated: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(migrated["version"], 2);
        assert_eq!(migrated["devices"][0]["model"], "auto");
    }

    #[test]
    fn each_saved_connection_keeps_its_own_selected_model() {
        let temp = tempfile::tempdir().unwrap();
        let mut book =
            Book::open(temp.path().join("c.json"), Box::<MemoryVault>::default()).unwrap();
        let mut first = edit("FM", "203.0.113.20");
        first.model = DeviceModel::Optimod5500i;
        let mut second = edit("HD", "203.0.113.21");
        second.model = DeviceModel::Optimod8500;

        assert_eq!(book.save(first).unwrap().model, DeviceModel::Optimod5500i);
        assert_eq!(book.save(second).unwrap().model, DeviceModel::Optimod8500);
    }

    #[cfg(unix)]
    #[test]
    fn file_vault_uses_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("private");
        let path = directory.join("credentials.json");
        let vault = FileVault::new(path.clone());
        vault.set("studio", "TESTCODE9876").unwrap();

        assert_eq!(
            fs::metadata(directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
