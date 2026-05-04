use crate::error::AppError;
use keyring::{Entry, Error as KeyringError};
use log::{info, warn};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

// Keep com.pot-app.desktop to share config with the Tauri version.
// The desktop file and icons use com.pot-app.pot-gtk for packaging.
const CONFIG_DIR_NAME: &str = "com.pot-app.desktop";

pub const APP_ID: &str = "com.pot-app.desktop";
const CONFIG_FILE: &str = "config.json";
const SECRET_FILE: &str = "secrets.json";
const SECRET_FIELDS: &[&str] = &[
    "api_key", "auth_key", "secret", "key", "appkey", "password", "token",
];
pub(crate) const DEFAULT_TRANSLATE_SERVICES: &[&str] = &["google", "bing"];

fn is_secret_field(field: &str) -> bool {
    SECRET_FIELDS.contains(&field) || field.ends_with("_token")
}

fn default_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(CONFIG_DIR_NAME)
}

struct SecretStore {
    path: PathBuf,
}

impl SecretStore {
    fn new(config_dir: PathBuf) -> Self {
        Self {
            path: config_dir.join(SECRET_FILE),
        }
    }

    fn entry_id(config_key: &str, field: &str) -> String {
        format!("{}::{}", config_key, field)
    }

    fn entry(config_key: &str, field: &str) -> keyring::Result<Entry> {
        Entry::new(APP_ID, &Self::entry_id(config_key, field))
    }

    fn get(&self, config_key: &str, field: &str) -> Option<String> {
        match Self::entry(config_key, field).and_then(|entry| entry.get_password()) {
            Ok(secret) => return Some(secret),
            Err(KeyringError::NoEntry) => {}
            Err(error) => warn!(
                "Failed to load secret '{}' from keyring: {}",
                Self::entry_id(config_key, field),
                error
            ),
        }

        self.read_local_store()
            .get(&Self::entry_id(config_key, field))
            .and_then(|value| value.as_str())
            .map(ToString::to_string)
    }

    fn set(&self, config_key: &str, field: &str, secret: &str) {
        if secret.is_empty() {
            self.delete(config_key, field);
            return;
        }

        match Self::entry(config_key, field).and_then(|entry| entry.set_password(secret)) {
            Ok(()) => {
                self.delete_local_secret(&Self::entry_id(config_key, field));
                return;
            }
            Err(error) => warn!(
                "Failed to store secret '{}' in keyring: {}. Falling back to local secret store.",
                Self::entry_id(config_key, field),
                error
            ),
        }

        self.write_local_secret(&Self::entry_id(config_key, field), secret);
    }

    fn delete(&self, config_key: &str, field: &str) {
        match Self::entry(config_key, field).and_then(|entry| entry.delete_password()) {
            Ok(()) | Err(KeyringError::NoEntry) => {}
            Err(error) => warn!(
                "Failed to delete secret '{}' from keyring: {}",
                Self::entry_id(config_key, field),
                error
            ),
        }

        self.delete_local_secret(&Self::entry_id(config_key, field));
    }

    fn read_local_store(&self) -> serde_json::Map<String, Value> {
        if !self.path.exists() {
            return serde_json::Map::new();
        }

        match fs::read_to_string(&self.path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|error| {
                warn!("Failed to parse local secret store: {}", error);
                serde_json::Map::new()
            }),
            Err(error) => {
                warn!("Failed to read local secret store: {}", error);
                serde_json::Map::new()
            }
        }
    }

    fn write_local_store(&self, data: &serde_json::Map<String, Value>) {
        if data.is_empty() {
            let _ = fs::remove_file(&self.path);
            return;
        }

        match serde_json::to_string_pretty(data) {
            Ok(content) => {
                if let Err(error) = fs::write(&self.path, content) {
                    warn!("Failed to save local secret store: {}", error);
                    return;
                }
                if let Err(error) = fs::set_permissions(
                    &self.path,
                    std::os::unix::fs::PermissionsExt::from_mode(0o600),
                ) {
                    warn!("Failed to set local secret store permissions: {}", error);
                }
            }
            Err(error) => warn!("Failed to serialize local secret store: {}", error),
        }
    }

    fn write_local_secret(&self, id: &str, secret: &str) {
        let mut data = self.read_local_store();
        data.insert(id.to_string(), Value::String(secret.to_string()));
        self.write_local_store(&data);
    }

    fn delete_local_secret(&self, id: &str) {
        let mut data = self.read_local_store();
        if data.remove(id).is_some() {
            self.write_local_store(&data);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceCategory {
    Translate,
    Recognize,
    Tts,
    Collection,
}

impl ServiceCategory {
    pub fn list_key(&self) -> &'static str {
        match self {
            ServiceCategory::Translate => "translate_service_list",
            ServiceCategory::Recognize => "recognize_service_list",
            ServiceCategory::Tts => "tts_service_list",
            ServiceCategory::Collection => "collection_service_list",
        }
    }
}

fn append_missing_default_translate_services(service_list: &mut Vec<String>) {
    for service_name in DEFAULT_TRANSLATE_SERVICES {
        let already_present = service_list.iter().any(|instance_key| {
            instance_key
                .split('@')
                .next()
                .unwrap_or(instance_key.as_str())
                == *service_name
        });

        if !already_present {
            service_list.push(service_name.to_string());
        }
    }
}

pub struct ConfigStore {
    path: PathBuf,
    data: serde_json::Map<String, Value>,
}

impl ConfigStore {
    pub fn new_in_dir(config_dir: PathBuf) -> Self {
        let path = config_dir.join(CONFIG_FILE);

        fs::create_dir_all(&config_dir).ok();
        info!("Config path: {:?}", path);

        let data = if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
                    warn!("Failed to parse config: {}", e);
                    serde_json::Map::new()
                }),
                Err(e) => {
                    warn!("Failed to read config: {}", e);
                    serde_json::Map::new()
                }
            }
        } else {
            info!("Config not found, creating new");
            serde_json::Map::new()
        };

        Self { path, data }
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        self.data.get(key).cloned()
    }

    pub fn set<T: serde::Serialize>(&mut self, key: &str, value: T) {
        self.data.insert(key.to_string(), json!(value));
        self.save();
    }

    pub fn remove(&mut self, key: &str) {
        if self.data.remove(key).is_some() {
            self.save();
        }
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    #[allow(dead_code)]
    pub fn reload(&mut self) {
        if let Ok(content) = fs::read_to_string(&self.path) {
            if let Ok(data) = serde_json::from_str(&content) {
                self.data = data;
            }
        }
    }

    fn save(&self) {
        if let Ok(content) = serde_json::to_string_pretty(&self.data) {
            if let Err(e) = fs::write(&self.path, &content) {
                warn!("Failed to save config: {}", e);
            } else if let Err(e) = fs::set_permissions(
                &self.path,
                std::os::unix::fs::PermissionsExt::from_mode(0o600),
            ) {
                warn!("Failed to set config permissions: {}", e);
            }
        }
    }

    pub fn config_dir(&self) -> PathBuf {
        self.path
            .parent()
            .expect("config path always has a parent")
            .to_path_buf()
    }
}

fn lock_store(store: &Mutex<ConfigStore>) -> std::sync::MutexGuard<'_, ConfigStore> {
    store.lock().unwrap_or_else(|e| e.into_inner())
}

pub struct AppConfig {
    pub store: Mutex<ConfigStore>,
}

impl AppConfig {
    pub fn new() -> Self {
        Self::new_in_dir(default_config_dir())
    }

    pub fn new_in_dir(config_dir: PathBuf) -> Self {
        let store = ConfigStore::new_in_dir(config_dir);
        let app_config = Self {
            store: Mutex::new(store),
        };
        app_config.migrate_plaintext_secrets();
        let _ = app_config.check_service_available();
        app_config
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        let value = lock_store(&self.store).get(key)?;
        Some(self.hydrate_secret_fields(key, value))
    }

    pub fn set<T: serde::Serialize>(&self, key: &str, value: T) {
        let value = self.sanitize_secret_fields(key, json!(value));
        lock_store(&self.store).set(key, value);
    }

    pub fn remove(&self, key: &str) {
        let secret_store = self.secret_store();
        for field in SECRET_FIELDS {
            secret_store.delete(key, field);
        }
        lock_store(&self.store).remove(key);
    }

    pub fn is_first_run(&self) -> bool {
        lock_store(&self.store).is_empty()
    }

    #[allow(dead_code)]
    pub fn reload(&self) {
        lock_store(&self.store).reload();
    }

    pub fn config_dir(&self) -> PathBuf {
        lock_store(&self.store).config_dir()
    }

    fn secret_store(&self) -> SecretStore {
        SecretStore::new(self.config_dir())
    }

    fn sanitize_secret_fields(&self, key: &str, value: Value) -> Value {
        if value.is_null() {
            let secret_store = self.secret_store();
            for field in SECRET_FIELDS {
                secret_store.delete(key, field);
            }
            return value;
        }

        let mut object = match value {
            Value::Object(object) => object,
            other => return other,
        };

        let secret_store = self.secret_store();
        let sensitive_fields: Vec<String> = object
            .keys()
            .filter(|field| is_secret_field(field))
            .cloned()
            .collect();

        for field in sensitive_fields {
            let value = object.remove(&field).unwrap_or(Value::Null);
            match value.as_str().map(str::trim) {
                Some(secret) if !secret.is_empty() => secret_store.set(key, &field, secret),
                _ => secret_store.delete(key, &field),
            }
        }

        Value::Object(object)
    }

    fn hydrate_secret_fields(&self, key: &str, value: Value) -> Value {
        let mut object = match value {
            Value::Object(object) => object,
            other => return other,
        };

        let secret_store = self.secret_store();
        for field in SECRET_FIELDS {
            if let Some(secret) = secret_store.get(key, field) {
                object.insert((*field).to_string(), Value::String(secret));
            }
        }

        Value::Object(object)
    }

    fn migrate_plaintext_secrets(&self) {
        let keys: Vec<String> = {
            let store = lock_store(&self.store);
            store.data.keys().cloned().collect()
        };

        for key in keys {
            let Some(original_value) = lock_store(&self.store).get(&key) else {
                continue;
            };

            let sanitized_value = self.sanitize_secret_fields(&key, original_value.clone());
            if sanitized_value != original_value {
                lock_store(&self.store).set(&key, sanitized_value);
            }
        }
    }

    pub fn generate_instance_key(service_name: &str) -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let rand_part = format!("{:x}", now.as_nanos() % 0xFFFF_FFFF);
        let id = &rand_part[rand_part.len().saturating_sub(8)..];
        format!("{}@{}", service_name, id)
    }

    pub fn get_service_list(&self, category: ServiceCategory) -> Vec<String> {
        self.get(category.list_key())
            .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
            .unwrap_or_default()
    }

    pub fn effective_service_list(&self, category: ServiceCategory) -> Vec<String> {
        let mut service_list = self.get_service_list(category);
        if matches!(category, ServiceCategory::Translate) {
            append_missing_default_translate_services(&mut service_list);
        }
        service_list
    }

    pub fn set_service_list(&self, category: ServiceCategory, list: &[String]) {
        self.set(category.list_key(), list);
    }

    pub fn get_plugin_list(&self, plugin_type: &str) -> Vec<String> {
        let config_dir = self.config_dir();
        let plugin_dir = config_dir.join("plugins").join(plugin_type);

        let mut plugin_list = Vec::new();
        if plugin_dir.exists() {
            if let Ok(read_dir) = fs::read_dir(&plugin_dir) {
                for entry in read_dir.flatten() {
                    if entry.path().is_dir() {
                        if let Some(name) = entry.file_name().to_str() {
                            if name.starts_with("plugin") {
                                plugin_list.push(name.to_string());
                            } else {
                                let _ = fs::remove_dir_all(entry.path());
                            }
                        }
                    }
                }
            }
        }
        plugin_list
    }

    pub fn check_service_available(&self) -> std::result::Result<(), AppError> {
        let builtin_translate: Vec<&str> = vec![
            "openai", "baidu", "bing", "deepl", "google", "lingva", "youdao",
        ];
        let builtin_recognize: Vec<&str> = vec!["tesseract"];
        let builtin_tts: Vec<&str> = vec!["lingva_tts"];
        let builtin_collection: Vec<&str> = vec!["anki"];

        let plugin_translate = self.get_plugin_list("translate");
        let plugin_recognize = self.get_plugin_list("recognize");
        let plugin_tts = self.get_plugin_list("tts");
        let plugin_collection = self.get_plugin_list("collection");

        if let Some(list) = self.get("translate_service_list") {
            let list: Vec<String> = serde_json::from_value(list)?;
            self.prune_unavailable(
                list,
                &builtin_translate,
                &plugin_translate,
                "translate_service_list",
            );
        }
        if let Some(list) = self.get("recognize_service_list") {
            let list: Vec<String> = serde_json::from_value(list)?;
            self.prune_unavailable(
                list,
                &builtin_recognize,
                &plugin_recognize,
                "recognize_service_list",
            );
        }
        if let Some(list) = self.get("tts_service_list") {
            let list: Vec<String> = serde_json::from_value(list)?;
            self.prune_unavailable(list, &builtin_tts, &plugin_tts, "tts_service_list");
        }
        if let Some(list) = self.get("collection_service_list") {
            let list: Vec<String> = serde_json::from_value(list)?;
            self.prune_unavailable(
                list,
                &builtin_collection,
                &plugin_collection,
                "collection_service_list",
            );
        }
        Ok(())
    }

    fn prune_unavailable(
        &self,
        list: Vec<String>,
        builtin: &[&str],
        plugins: &[String],
        key: &str,
    ) {
        let origin_len = list.len();
        let new_list: Vec<String> = list
            .into_iter()
            .filter(|service| {
                let name = service.split('@').next().unwrap_or("");
                let config_exists =
                    !service.contains('@') || self.has_service_instance_config(service);
                if !config_exists {
                    return false;
                }
                if name.starts_with("plugin") {
                    plugins.iter().any(|p| p == name)
                } else {
                    builtin.contains(&name)
                }
            })
            .collect();
        if new_list.len() != origin_len {
            self.set(key, new_list);
        }
    }

    fn has_service_instance_config(&self, instance_key: &str) -> bool {
        lock_store(&self.store)
            .get(instance_key)
            .and_then(|value| value.as_object().cloned())
            .is_some()
    }
}
