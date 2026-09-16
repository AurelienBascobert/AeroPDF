use std::collections::HashMap;
use std::path::PathBuf;

pub(crate) fn get_history_file_path() -> PathBuf {
    if let Ok(app_data) = std::env::var("APPDATA") {
        let dir = PathBuf::from(app_data).join("aeropdf");
        let _ = std::fs::create_dir_all(&dir);
        return dir.join("history.txt");
    }
    std::env::temp_dir().join("aeropdf_history.txt")
}

pub(crate) fn load_history() -> HashMap<String, usize> {
    let mut history = HashMap::new();
    let path = get_history_file_path();
    if let Ok(content) = std::fs::read_to_string(path) {
        for line in content.lines() {
            if let Some((page_str, path_str)) = line.split_once('|') {
                if let Ok(page) = page_str.trim().parse::<usize>() {
                    history.insert(path_str.trim().to_string(), page);
                }
            }
        }
    }
    history
}

pub(crate) fn save_history(history: &HashMap<String, usize>) {
    let path = get_history_file_path();
    let mut content = String::new();
    for (path_str, page) in history {
        content.push_str(&format!("{}|{}\n", page, path_str));
    }
    let _ = std::fs::write(path, content);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_round_trip() {
        let tmp = std::env::temp_dir().join("aeropdf_test_history.txt");

        // Write test data
        let content = "5|C:\\docs\\test.pdf\n12|C:\\docs\\other.pdf\n";
        std::fs::write(&tmp, content).unwrap();

        // Parse it
        let loaded = std::fs::read_to_string(&tmp).unwrap();
        let mut history = HashMap::new();
        for line in loaded.lines() {
            if let Some((page_str, path_str)) = line.split_once('|') {
                if let Ok(page) = page_str.trim().parse::<usize>() {
                    history.insert(path_str.trim().to_string(), page);
                }
            }
        }

        assert_eq!(history.len(), 2);
        assert_eq!(history.get(r"C:\docs\test.pdf"), Some(&5));
        assert_eq!(history.get(r"C:\docs\other.pdf"), Some(&12));

        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn test_history_malformed_lines() {
        let content = "notanumber|path.pdf\n|empty\n5|valid.pdf\n";
        let mut history = HashMap::new();
        for line in content.lines() {
            if let Some((page_str, path_str)) = line.split_once('|') {
                if let Ok(page) = page_str.trim().parse::<usize>() {
                    history.insert(path_str.trim().to_string(), page);
                }
            }
        }
        assert_eq!(history.len(), 1);
        assert_eq!(history.get("valid.pdf"), Some(&5));
    }
}
