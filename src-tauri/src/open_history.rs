use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OpenHistoryItem {
    pub key: String,        // path or id that uniquely identifies the item
    pub last_opened: u64,   // Unix timestamp
}

static OPEN_HISTORY: LazyLock<Arc<Mutex<HashMap<String, u64>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(HashMap::new())));

pub fn get_history_file_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("open_history.json")
}

pub fn lock_history() -> Result<std::sync::MutexGuard<'static, HashMap<String, u64>>, String> {
    OPEN_HISTORY
        .lock()
        .map_err(|e| format!("Failed to lock open history: {}", e))
}

// Load history into an already-locked state (no additional locking)
pub fn load_history_into(
    state: &mut HashMap<String, u64>,
    app_data_dir: &Path,
) -> Result<(), String> {
    let history_file = get_history_file_path(app_data_dir);

    if !history_file.exists() {
        return Ok(()); // No history file, start fresh
    }

    let content = fs::read_to_string(&history_file)
        .map_err(|e| format!("Failed to read history file: {}", e))?;

    let history: HashMap<String, u64> = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse history file: {}", e))?;

    *state = history;
    Ok(())
}

// Legacy function for backward compatibility - but now uses lock_history internally
pub fn load_history(app_data_dir: &Path) -> Result<(), String> {
    let mut state = lock_history()?;
    load_history_into(&mut state, app_data_dir)
}

// Save history from a provided state (no locking)
fn save_history_internal(
    state: &HashMap<String, u64>,
    app_data_dir: &Path,
) -> Result<(), String> {
    // Create directory if it doesn't exist
    if !app_data_dir.exists() {
        fs::create_dir_all(app_data_dir)
            .map_err(|e| format!("Failed to create app data directory: {}", e))?;
    }

    let history_file = get_history_file_path(app_data_dir);

    let history_json = serde_json::to_string_pretty(state)
        .map_err(|e| format!("Failed to serialize history: {}", e))?;

    fs::write(&history_file, history_json)
        .map_err(|e| format!("Failed to write history file: {}", e))?;

    Ok(())
}

// Legacy function for backward compatibility
pub fn save_history(app_data_dir: &Path) -> Result<(), String> {
    let state = lock_history()?;
    save_history_internal(&state, app_data_dir)
}

pub fn record_open(key: String, app_data_dir: &Path) -> Result<(), String> {
    // Load history first to ensure it's up to date
    {
        let mut state = lock_history()?;
        load_history_into(&mut state, app_data_dir).ok(); // Ignore errors if file doesn't exist
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("Failed to get timestamp: {}", e))?
        .as_secs();

    let mut state = lock_history()?;
    state.insert(key, timestamp);
    drop(state);

    // Save to disk
    save_history(app_data_dir)?;

    Ok(())
}

pub fn get_last_opened(key: &str) -> Option<u64> {
    let state = lock_history().ok()?;
    state.get(key).copied()
}

pub fn get_all_history(app_data_dir: &Path) -> Result<HashMap<String, u64>, String> {
    let mut state = lock_history()?;
    load_history_into(&mut state, app_data_dir).ok(); // Ignore errors if file doesn't exist
    Ok(state.clone())
}

