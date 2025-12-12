//! 平台能力抽象：搜索 / 快捷键 / 录制-回放
//! Windows 与 macOS 分别实现，命令层只依赖这些接口。
use crate::recording::RecordedEvent;

/// 通用搜索结果
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub path: String,
    pub name: String,
    pub score: f32,
    pub is_folder: bool,
}

pub trait SearchProvider: Send + Sync {
    fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, String>;
}

/// 全局快捷键能力
pub trait HotkeyProvider: Send + Sync {
    /// 注册快捷键，例如 "Command+Space" / "Ctrl+Alt+K"
    fn register(&self, shortcut: &str, handler_id: &str) -> Result<(), String>;
    /// 取消注册
    fn unregister(&self, shortcut: &str) -> Result<(), String>;
    /// 清除全部
    fn unregister_all(&self) -> Result<(), String>;
}

/// 录制与回放
pub trait Recorder: Send + Sync {
    fn start(&self) -> Result<(), String>;
    fn stop(&self) -> Result<Vec<RecordedEvent>, String>;
    fn play(&self, events: &[RecordedEvent], speed: f32) -> Result<(), String>;
}

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;
