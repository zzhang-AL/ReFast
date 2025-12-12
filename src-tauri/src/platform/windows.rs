//! Windows 平台实现占位，保持编译通过。
//! Windows 仍使用现有模块（everything_search/hotkey_handler/hooks/replay）。

use crate::platform::{HotkeyProvider, Recorder, SearchProvider, SearchResult};
use crate::recording::RecordedEvent;

pub struct WindowsSearchProvider;
impl WindowsSearchProvider {
    pub fn new() -> Self {
        Self
    }
}
impl SearchProvider for WindowsSearchProvider {
    fn search(&self, _query: &str, _limit: usize) -> Result<Vec<SearchResult>, String> {
        Err("Windows 平台请直接使用 everything_search 模块".to_string())
    }
}

pub struct WindowsHotkeyProvider;
impl WindowsHotkeyProvider {
    pub fn new() -> Self {
        Self
    }
}
impl HotkeyProvider for WindowsHotkeyProvider {
    fn register(&self, _shortcut: &str, _handler_id: &str) -> Result<(), String> {
        Err("Windows 平台请使用 hotkey_handler 模块".to_string())
    }
    fn unregister(&self, _shortcut: &str) -> Result<(), String> {
        Err("Windows 平台请使用 hotkey_handler 模块".to_string())
    }
    fn unregister_all(&self) -> Result<(), String> {
        Err("Windows 平台请使用 hotkey_handler 模块".to_string())
    }
}

pub struct WindowsRecorder;
impl WindowsRecorder {
    pub fn new() -> Self {
        Self
    }
}
impl Recorder for WindowsRecorder {
    fn start(&self) -> Result<(), String> {
        Err("Windows 平台请使用 hooks 模块录制".to_string())
    }
    fn stop(&self) -> Result<Vec<RecordedEvent>, String> {
        Err("Windows 平台请使用 hooks 模块录制".to_string())
    }
    fn play(&self, _events: &[RecordedEvent], _speed: f32) -> Result<(), String> {
        Err("Windows 平台请使用 replay 模块回放".to_string())
    }
}
