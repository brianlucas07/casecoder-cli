use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub fn app_dir() -> PathBuf {
    let base = if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config")
    } else {
        PathBuf::from(".")
    };

    let dir = base.join("casecoder");
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn config_file_path() -> PathBuf {
    app_dir().join("config.json")
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Config {
    #[serde(default)]
    pub model: String,
}

pub fn carregar_config() -> Config {
    let path = config_file_path();
    if !path.exists() {
        let default_config = Config::default();
        let _ = salvar_config(&default_config);
        return default_config;
    }

    match fs::read_to_string(&path) {
        Ok(conteudo) => serde_json::from_str(&conteudo).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

pub fn salvar_config(config: &Config) -> Result<(), std::io::Error> {
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    fs::write(config_file_path(), json)
}
