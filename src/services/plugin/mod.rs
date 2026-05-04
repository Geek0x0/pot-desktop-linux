use crate::error::AppError;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    #[serde(rename = "plugin_type")]
    pub plugin_type: String,
    #[serde(default)]
    pub display: HashMap<String, String>,
    #[serde(default)]
    pub language: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Plugin {
    pub info: PluginInfo,
    pub path: PathBuf,
}

impl Plugin {
    pub fn main_js_path(&self) -> PathBuf {
        self.path.join("main.js")
    }

    pub fn load_script(&self) -> Result<String> {
        fs::read_to_string(self.main_js_path())
            .map_err(|e| AppError::Custom(format!("Failed to read plugin script: {}", e)))
    }
}

pub struct PluginManager {
    plugins: HashMap<String, HashMap<String, Plugin>>, // type -> name -> plugin
}

impl PluginManager {
    pub fn new() -> Self {
        let mut manager = PluginManager {
            plugins: HashMap::new(),
        };
        manager.discover_all();
        manager
    }

    fn plugins_dir() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("com.pot-app.desktop/plugins"))
    }

    pub fn discover_all(&mut self) {
        if let Some(base) = Self::plugins_dir() {
            for plugin_type in &["translate", "recognize", "tts", "collection"] {
                self.discover_type(*plugin_type, &base.join(plugin_type));
            }
        }
    }

    fn discover_type(&mut self, plugin_type: &str, dir: &Path) {
        if !dir.exists() {
            return;
        }
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        let type_plugins = self.plugins.entry(plugin_type.to_string()).or_default();

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) if n.starts_with("plugin") => n.to_string(),
                _ => continue,
            };

            let info_path = path.join("info.json");
            if !info_path.exists() {
                warn!("Plugin {} missing info.json", name);
                continue;
            }

            let info: PluginInfo = match fs::read_to_string(&info_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
            {
                Some(info) => info,
                None => {
                    warn!("Plugin {} has invalid info.json", name);
                    continue;
                }
            };

            info!("Discovered plugin: {} ({})", name, plugin_type);
            type_plugins.insert(name, Plugin { info, path });
        }
    }

    pub fn get(&self, plugin_type: &str, name: &str) -> Option<&Plugin> {
        self.plugins.get(plugin_type).and_then(|m| m.get(name))
    }

    pub fn list(&self, plugin_type: &str) -> Vec<&str> {
        self.plugins
            .get(plugin_type)
            .map(|m| m.keys().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    pub fn get_plugin_list(&self, plugin_type: &str) -> Vec<String> {
        self.plugins
            .get(plugin_type)
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Install a .potext plugin from the given file path
    pub fn install(&mut self, path: &Path) -> Result<String> {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| AppError::Custom("Invalid file path".into()))?;

        if !filename.ends_with(".potext") || !filename.starts_with("plugin") {
            return Err(AppError::Custom(
                "Plugin file must be named plugin*.potext".into(),
            ));
        }

        let plugin_name = filename.trim_end_matches(".potext");

        let file = fs::File::open(path)?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| AppError::Custom(format!("Invalid plugin archive: {}", e)))?;

        // Read info.json to determine plugin type
        let info: PluginInfo = {
            let mut info_file = archive
                .by_name("info.json")
                .map_err(|_| AppError::Custom("Plugin missing info.json".into()))?;
            let mut content = String::new();
            std::io::Read::read_to_string(&mut info_file, &mut content)?;
            serde_json::from_str(&content)?
        };

        let plugin_type = &info.plugin_type;
        let base = Self::plugins_dir()
            .ok_or_else(|| AppError::Custom("Cannot determine plugins directory".into()))?;
        let dest = base.join(plugin_type).join(plugin_name);
        fs::create_dir_all(&dest)?;

        // Canonicalize dest AFTER creating it so the path resolves fully.
        // Fail the entire extraction if canonicalization fails — a non-resolvable
        // destination means something is fundamentally wrong with the filesystem.
        let canonical_dest = dest.canonicalize().map_err(|e| {
            AppError::Custom(format!("Cannot resolve plugin destination path: {}", e))
        })?;

        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| AppError::Custom(format!("Archive read error: {}", e)))?;
            let entry_name = match entry.enclosed_name() {
                Some(name) => name.to_path_buf(),
                None => continue, // skip entries with path traversal (e.g. ../)
            };
            // Reject any path component containing ".." — belt-and-suspenders
            // since enclosed_name() already filters them.
            if entry_name
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                continue;
            }
            let entry_path = dest.join(&entry_name);
            // Ensure the parent directory exists, then canonicalize it to
            // verify it hasn't escaped via symlinks.
            if let Some(parent) = entry_path.parent() {
                fs::create_dir_all(parent)?;
                let canonical_parent = parent.canonicalize().map_err(|e| {
                    AppError::Custom(format!("Cannot resolve extraction path: {}", e))
                })?;
                if !canonical_parent.starts_with(&canonical_dest) {
                    continue; // path escaped the destination — skip
                }
            }
            if entry.is_dir() {
                fs::create_dir_all(&entry_path)?;
            } else {
                let mut outfile = fs::File::create(&entry_path)?;
                std::io::copy(&mut entry, &mut outfile)?;
            }
        }

        info!("Installed plugin: {} ({})", plugin_name, plugin_type);

        // Re-discover to pick up the new plugin
        self.discover_all();

        Ok(plugin_name.to_string())
    }

    /// Uninstall a plugin by type and name
    pub fn uninstall(&mut self, plugin_type: &str, name: &str) -> Result<()> {
        let base = Self::plugins_dir()
            .ok_or_else(|| AppError::Custom("Cannot determine plugins directory".into()))?;
        let dir = base.join(plugin_type).join(name);
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
        }
        if let Some(type_plugins) = self.plugins.get_mut(plugin_type) {
            type_plugins.remove(name);
        }
        info!("Uninstalled plugin: {} ({})", name, plugin_type);
        Ok(())
    }
}

#[cfg(feature = "plugin")]
pub fn execute_plugin_script(
    script: &str,
    plugin_type: &str,
) -> std::result::Result<String, String> {
    use boa_engine::{Context, Source};

    let mut context = Context::default();
    let source = Source::from_bytes(script);
    let result = context
        .eval(source)
        .map_err(|e| format!("Plugin script error: {:?}", e))?;

    let output = result
        .to_string(&mut context)
        .map_err(|e| format!("Plugin result conversion error: {:?}", e))?;

    Ok(output.to_std_string_escaped())
}

/// Execute a plugin with input variables injected as global scope.
/// The plugin's main.js should define a function matching the plugin type
/// (e.g., `translate(text, from, to)` for translate plugins)
/// and assign the result to a global `result` variable.
#[cfg(feature = "plugin")]
pub fn execute_plugin_with_input(
    plugin: &Plugin,
    args: &str,
) -> std::result::Result<String, String> {
    let script_content = plugin.load_script().map_err(|e| e.to_string())?;
    let wrapped = format!("(function() {{\n{}\nreturn result;\n}})()", script_content);
    execute_plugin_script(&wrapped, &plugin.info.plugin_type)
}

#[cfg(feature = "plugin")]
mod adapters {
    use super::*;
    use crate::services::types::{ServiceError, TranslateRequest, TranslateResult};
    use serde_json::Value;
    use std::sync::Arc;

    pub struct PluginTranslateService {
        plugin: Arc<Plugin>,
    }

    impl PluginTranslateService {
        pub fn new(plugin: Arc<Plugin>) -> Self {
            Self { plugin }
        }
    }

    #[async_trait::async_trait]
    impl crate::services::translate::TranslateService for PluginTranslateService {
        fn name(&self) -> &str {
            &self.plugin.info.name
        }

        async fn translate(
            &self,
            req: TranslateRequest,
        ) -> std::result::Result<TranslateResult, ServiceError> {
            let js_args = serde_json::json!({
                "text": req.text,
                "from": req.from,
                "to": req.to,
                "config": req.config,
            })
            .to_string();

            let output = super::execute_plugin_with_input(&self.plugin, &js_args).map_err(|e| {
                ServiceError {
                    service: self.name().to_string(),
                    message: e,
                }
            })?;

            Ok(TranslateResult::Text(output))
        }

        fn default_config(&self) -> Value {
            Value::Null
        }
    }

    pub fn create_translate_adapters(
        manager: &PluginManager,
    ) -> Vec<Arc<dyn crate::services::translate::TranslateService>> {
        manager
            .list("translate")
            .iter()
            .filter_map(|name| {
                manager.get("translate", name).map(|p| {
                    Arc::new(PluginTranslateService::new(Arc::new(p.clone())))
                        as Arc<dyn crate::services::translate::TranslateService>
                })
            })
            .collect()
    }
}

#[cfg(feature = "plugin")]
pub use adapters::create_translate_adapters;

#[cfg(not(feature = "plugin"))]
pub fn create_translate_adapters(
    _manager: &PluginManager,
) -> Vec<std::sync::Arc<dyn crate::services::translate::TranslateService>> {
    Vec::new()
}
