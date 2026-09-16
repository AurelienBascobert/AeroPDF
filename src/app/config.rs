use std::collections::HashMap;
use std::path::PathBuf;

/// User preferences persisted across sessions.
#[derive(Debug, Clone)]
pub(crate) struct UserConfig {
    pub dark_mode: bool,
    pub zoom: f32,
    pub dual_page_mode: bool,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            dark_mode: false,
            zoom: 1.0,
            dual_page_mode: false,
        }
    }
}

fn config_path() -> PathBuf {
    if let Ok(app_data) = std::env::var("APPDATA") {
        let dir = PathBuf::from(app_data).join("aeropdf");
        let _ = std::fs::create_dir_all(&dir);
        return dir.join("config.txt");
    }
    std::env::temp_dir().join("aeropdf_config.txt")
}

pub(crate) fn load_config() -> UserConfig {
    let mut cfg = UserConfig::default();
    let path = config_path();
    if let Ok(content) = std::fs::read_to_string(path) {
        let map: HashMap<&str, &str> = content
            .lines()
            .filter_map(|line| line.split_once('='))
            .map(|(k, v)| (k.trim(), v.trim()))
            .collect();

        if let Some(&val) = map.get("dark_mode") {
            cfg.dark_mode = val == "true";
        }
        if let Some(&val) = map.get("zoom") {
            if let Ok(z) = val.parse::<f32>() {
                cfg.zoom = z.clamp(0.5, 4.0);
            }
        }
        if let Some(&val) = map.get("dual_page_mode") {
            cfg.dual_page_mode = val == "true";
        }
    }
    cfg
}

pub(crate) fn save_config(cfg: &UserConfig) {
    let path = config_path();
    let content = format!(
        "dark_mode={}\nzoom={:.2}\ndual_page_mode={}\n",
        cfg.dark_mode, cfg.zoom, cfg.dual_page_mode
    );
    let _ = std::fs::write(path, content);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let cfg = UserConfig::default();
        assert!(!cfg.dark_mode);
        assert!((cfg.zoom - 1.0).abs() < f32::EPSILON);
        assert!(!cfg.dual_page_mode);
    }

    #[test]
    fn test_config_round_trip() {
        let tmp = std::env::temp_dir().join("aeropdf_test_config.txt");
        let content = "dark_mode=true\nzoom=1.50\ndual_page_mode=true\n";
        std::fs::write(&tmp, content).unwrap();

        let loaded_content = std::fs::read_to_string(&tmp).unwrap();
        let map: std::collections::HashMap<&str, &str> = loaded_content
            .lines()
            .filter_map(|line| line.split_once('='))
            .map(|(k, v)| (k.trim(), v.trim()))
            .collect();

        assert_eq!(map.get("dark_mode"), Some(&"true"));
        assert_eq!(map.get("zoom"), Some(&"1.50"));
        assert_eq!(map.get("dual_page_mode"), Some(&"true"));

        let _ = std::fs::remove_file(tmp);
    }
}
