use crate::error::AppError;
use crate::recording::{RecordingMeta, RecordingState};
use crate::replay::ReplayState;
use std::sync::{LazyLock, Mutex};

static RECORDING_STATE: LazyLock<Mutex<RecordingState>> = LazyLock::new(|| Mutex::new(RecordingState::new()));

static REPLAY_STATE: LazyLock<Mutex<ReplayState>> = LazyLock::new(|| Mutex::new(ReplayState::new()));

#[tauri::command]
pub fn start_recording() -> Result<(), String> {
    #[cfg(not(target_os = "windows"))]
    {
        return Err(AppError::PlatformNotSupported(
            "Recording is only supported on Windows".to_string(),
        )
        .to_string());
    }

    let mut state = RECORDING_STATE.lock().map_err(|e| e.to_string())?;
    
    if state.is_recording {
        return Err("Already recording".to_string());
    }

    state.start();
    
    // TODO: Install Windows hooks here
    Ok(())
}

#[tauri::command]
pub fn stop_recording() -> Result<String, String> {
    #[cfg(not(target_os = "windows"))]
    {
        return Err(AppError::PlatformNotSupported(
            "Recording is only supported on Windows".to_string(),
        )
        .to_string());
    }

    let mut state = RECORDING_STATE.lock().map_err(|e| e.to_string())?;
    
    if !state.is_recording {
        return Err("Not currently recording".to_string());
    }

    state.stop();
    
    // TODO: Uninstall Windows hooks here
    // TODO: Save events to JSON file
    // For now, return a placeholder path
    Ok("recordings/recording_placeholder.json".to_string())
}

#[tauri::command]
pub fn list_recordings() -> Result<Vec<RecordingMeta>, String> {
    // TODO: Scan recordings directory and return metadata
    // For now, return empty list
    Ok(vec![])
}

#[tauri::command]
pub fn play_recording(path: String, speed: f32) -> Result<(), String> {
    #[cfg(not(target_os = "windows"))]
    {
        return Err(AppError::PlatformNotSupported(
            "Replay is only supported on Windows".to_string(),
        )
        .to_string());
    }

    let mut state = REPLAY_STATE.lock().map_err(|e| e.to_string())?;
    
    if state.is_playing {
        return Err("Already playing".to_string());
    }

    state.load_recording(&path)?;
    state.start(speed);
    
    // TODO: Start replay task here
    Ok(())
}

#[tauri::command]
pub fn stop_playback() -> Result<(), String> {
    let mut state = REPLAY_STATE.lock().map_err(|e| e.to_string())?;
    
    if !state.is_playing {
        return Err("Not currently playing".to_string());
    }

    state.stop();
    
    // TODO: Stop replay task here
    Ok(())
}

