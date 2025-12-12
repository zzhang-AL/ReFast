//! macOS 平台实现：Spotlight 搜索（mdfind）、全局快捷键（tauri-plugin-global-shortcut）、录制/回放（rdev）。
use crate::platform::{HotkeyProvider, Recorder, SearchProvider, SearchResult};
use crate::recording::{EventType, MouseButton, RecordedEvent};
use crate::settings;
use rdev::{listen, simulate, Button, Event, EventType as RdevEvent, Key, SimulateError};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::LazyLock;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

/// Spotlight 搜索实现
pub struct SpotlightSearchProvider;

impl SpotlightSearchProvider {
    pub fn new() -> Self {
        Self
    }
}

impl SearchProvider for SpotlightSearchProvider {
    fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, String> {
        // 使用 -name 将搜索范围限定为文件名，行为更接近 Everything（避免全文内容匹配带来的噪音与性能问题）
        let output = std::process::Command::new("mdfind")
            .arg("-name")
            .arg(query)
            .output()
            .map_err(|err| format!("mdfind 执行失败: {err}"))?;
        if !output.status.success() {
            return Err(format!("mdfind 返回错误: {:?}", output.status.code()));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let results = stdout
            .lines()
            .take(limit)
            .enumerate()
            .map(|(idx, line)| SearchResult {
                path: line.to_string(),
                name: line.rsplit('/').next().unwrap_or(line).to_string(),
                score: (limit - idx) as f32,
                is_folder: std::fs::metadata(line).map(|m| m.is_dir()).unwrap_or(false),
            })
            .collect::<Vec<_>>();
        Ok(results)
    }
}

/// 全局快捷键实现（基于 tauri-plugin-global-shortcut）
pub struct GlobalShortcutProvider {
    app: AppHandle,
}

impl GlobalShortcutProvider {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl HotkeyProvider for GlobalShortcutProvider {
    fn register(&self, shortcut: &str, handler_id: &str) -> Result<(), String> {
        let app_handle = self.app.clone();
        let handler_id_owned = handler_id.to_string();
        self.app
            .global_shortcut()
            .on_shortcut(shortcut, move |app, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    let _ = app.emit("global-hotkey", handler_id_owned.clone());
                }
            })
            .map_err(|e| format!("注册全局快捷键失败: {e}"))?;
        Ok(())
    }

    fn unregister(&self, shortcut: &str) -> Result<(), String> {
        self.app
            .global_shortcut()
            .unregister(shortcut)
            .map_err(|e| format!("取消快捷键失败: {e}"))
    }

    fn unregister_all(&self) -> Result<(), String> {
        self.app
            .global_shortcut()
            .unregister_all()
            .map_err(|e| format!("取消全部快捷键失败: {e}"))
    }
}

// -------------------------
// 重复修饰键（双击）快捷键
// -------------------------

/// 重复修饰键（双击）触发后的动作。
#[derive(Debug, Clone)]
pub enum DoubleTapAction {
    /// 呼出/隐藏启动器
    ToggleLauncher,
    /// 打开应用中心（插件列表窗口）
    ShowAppCenter,
    /// 触发某个插件（通知前端）
    TriggerPlugin(String),
    /// 启动某个应用（通常是 `.app` 路径）
    LaunchApp(String),
}

/// 目前仅支持四种常见修饰键（与前端录制输出保持一致）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DoubleTapModifier {
    Ctrl,
    Alt,
    Shift,
    Meta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DoubleTapPhase {
    Idle,
    FirstPressed,
    FirstReleased,
}

#[derive(Debug, Clone)]
struct DoubleTapDetector {
    phase: DoubleTapPhase,
    last_release_time: Option<Instant>,
    other_key_pressed: bool,
}

impl Default for DoubleTapDetector {
    fn default() -> Self {
        Self {
            phase: DoubleTapPhase::Idle,
            last_release_time: None,
            other_key_pressed: false,
        }
    }
}

impl DoubleTapDetector {
    fn reset(&mut self) {
        self.phase = DoubleTapPhase::Idle;
        self.last_release_time = None;
        self.other_key_pressed = false;
    }

    fn mark_interference(&mut self) {
        if self.phase == DoubleTapPhase::FirstPressed || self.phase == DoubleTapPhase::FirstReleased {
            self.other_key_pressed = true;
        }
    }

    fn tick_timeout(&mut self, now: Instant, timeout: Duration) {
        if self.phase != DoubleTapPhase::FirstReleased {
            return;
        }
        let release_time = match self.last_release_time {
            Some(t) => t,
            None => {
                self.reset();
                return;
            }
        };
        if now.duration_since(release_time) > timeout {
            self.reset();
        }
    }

    /// 处理“目标修饰键”的 KeyPress，返回是否检测到双击。
    fn on_modifier_press(&mut self, now: Instant, timeout: Duration) -> bool {
        self.tick_timeout(now, timeout);

        match self.phase {
            DoubleTapPhase::Idle => {
                self.phase = DoubleTapPhase::FirstPressed;
                self.other_key_pressed = false;
                false
            }
            DoubleTapPhase::FirstPressed => {
                // 修饰键仍按下，忽略
                false
            }
            DoubleTapPhase::FirstReleased => {
                let within_timeout = self
                    .last_release_time
                    .map(|t| now.duration_since(t) <= timeout)
                    .unwrap_or(false);

                if within_timeout && !self.other_key_pressed {
                    self.reset();
                    true
                } else {
                    // 超时或有干扰：重置后将本次按下视为新的第一次
                    self.reset();
                    self.phase = DoubleTapPhase::FirstPressed;
                    false
                }
            }
        }
    }

    fn on_modifier_release(&mut self, now: Instant) {
        match self.phase {
            DoubleTapPhase::FirstPressed => {
                self.phase = DoubleTapPhase::FirstReleased;
                self.last_release_time = Some(now);
            }
            _ => {}
        }
    }
}

struct DoubleTapHotkeyManager {
    app: Option<AppHandle>,
    bindings: HashMap<DoubleTapModifier, DoubleTapAction>,
    detectors: HashMap<DoubleTapModifier, DoubleTapDetector>,
    timeout: Duration,
}

impl DoubleTapHotkeyManager {
    fn new() -> Self {
        Self {
            app: None,
            bindings: HashMap::new(),
            detectors: HashMap::new(),
            // 与前端录制逻辑保持一致，避免“录得出来但触发不了”
            timeout: Duration::from_millis(500),
        }
    }

    fn update_bindings(&mut self, app: AppHandle, bindings: HashMap<DoubleTapModifier, DoubleTapAction>) {
        self.app = Some(app);
        self.bindings = bindings;
        self.detectors.clear();
        for modifier in self.bindings.keys() {
            self.detectors.insert(*modifier, DoubleTapDetector::default());
        }
    }

    fn process_event(&mut self, event: &Event) -> Option<(AppHandle, DoubleTapAction)> {
        if self.detectors.is_empty() {
            return None;
        }

        let now = Instant::now();
        for det in self.detectors.values_mut() {
            det.tick_timeout(now, self.timeout);
        }

        match event.event_type {
            RdevEvent::KeyPress(key) => {
                let pressed_modifier = key_to_double_tap_modifier(key);
                let mut triggered: Option<DoubleTapModifier> = None;

                if let Some(pm) = pressed_modifier {
                    for (m, det) in self.detectors.iter_mut() {
                        if *m == pm {
                            if det.on_modifier_press(now, self.timeout) {
                                triggered = Some(pm);
                            }
                        } else {
                            // 其他任意按键（包含其它修饰键）都会打断双击序列，减少误触
                            det.mark_interference();
                        }
                    }
                } else {
                    // 非修饰键按下：视为干扰
                    for det in self.detectors.values_mut() {
                        det.mark_interference();
                    }
                }

                let modifier = triggered?;
                let app = self.app.as_ref()?.clone();
                let action = self.bindings.get(&modifier)?.clone();
                Some((app, action))
            }
            RdevEvent::KeyRelease(key) => {
                if let Some(modifier) = key_to_double_tap_modifier(key) {
                    if let Some(det) = self.detectors.get_mut(&modifier) {
                        det.on_modifier_release(now);
                    }
                }
                None
            }
            _ => None,
        }
    }
}

fn key_to_double_tap_modifier(key: Key) -> Option<DoubleTapModifier> {
    match key {
        Key::ControlLeft | Key::ControlRight => Some(DoubleTapModifier::Ctrl),
        Key::ShiftLeft | Key::ShiftRight => Some(DoubleTapModifier::Shift),
        Key::Alt | Key::AltGr => Some(DoubleTapModifier::Alt),
        Key::MetaLeft | Key::MetaRight => Some(DoubleTapModifier::Meta),
        _ => None,
    }
}

fn execute_double_tap_action(app: AppHandle, action: DoubleTapAction) {
    match action {
        DoubleTapAction::ToggleLauncher => {
            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = crate::commands::toggle_launcher(app_handle);
            });
        }
        DoubleTapAction::ShowAppCenter => {
            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = crate::commands::show_plugin_list_window(app_handle).await;
            });
        }
        DoubleTapAction::TriggerPlugin(plugin_id) => {
            let _ = app.emit("plugin-hotkey-triggered", plugin_id);
        }
        DoubleTapAction::LaunchApp(app_path) => {
            tauri::async_runtime::spawn(async move {
                let info = crate::app_search::AppInfo {
                    name: String::new(),
                    path: app_path,
                    icon: None,
                    description: None,
                    name_pinyin: None,
                    name_pinyin_initials: None,
                };
                let _ = crate::app_search::windows::launch_app(&info);
            });
        }
    }
}

/// 录制实现：使用 rdev 监听全局事件
pub struct RdevRecorder {
    state: Arc<Mutex<RecordingState>>,
    double_tap: Arc<Mutex<DoubleTapHotkeyManager>>,
}

static MAC_RECORDER: LazyLock<RdevRecorder> = LazyLock::new(|| RdevRecorder::new());

/// macOS：是否已启用（或曾经启用过）“双击修饰键”快捷键。
/// <p>
/// 用途：避免在用户未使用该功能时无谓启动 rdev 全局监听器（减少权限打扰）。
/// </p>
static MACOS_DOUBLE_TAP_ACTIVE: AtomicBool = AtomicBool::new(false);

#[derive(Default)]
struct RecordingState {
    is_recording: bool,
    events: Vec<RecordedEvent>,
    start: Option<std::time::Instant>,
}

impl RdevRecorder {
    pub fn new() -> Self {
        let recorder = Self {
            state: Arc::new(Mutex::new(RecordingState::default())),
            double_tap: Arc::new(Mutex::new(DoubleTapHotkeyManager::new())),
        };
        start_rdev_listener(recorder.state.clone(), recorder.double_tap.clone());
        recorder
    }

    /// 更新“双击修饰键”绑定。
    /// <p>
    /// 仅在用户配置了该类快捷键时由命令层调用；若 bindings 为空则清空绑定并关闭检测。
    /// </p>
    pub fn update_double_tap_bindings(
        &self,
        app: AppHandle,
        bindings: HashMap<DoubleTapModifier, DoubleTapAction>,
    ) {
        if let Ok(mut guard) = self.double_tap.lock() {
            guard.update_bindings(app, bindings);
        }
    }
}

impl Recorder for RdevRecorder {
    fn start(&self) -> Result<(), String> {
        let mut guard = self.state.lock().map_err(|e| e.to_string())?;
        guard.is_recording = true;
        guard.events.clear();
        guard.start = Some(std::time::Instant::now());
        Ok(())
    }

    fn stop(&self) -> Result<Vec<RecordedEvent>, String> {
        let mut guard = self.state.lock().map_err(|e| e.to_string())?;
        guard.is_recording = false;
        guard.start = None;
        Ok(guard.events.clone())
    }

    fn play(&self, events: &[RecordedEvent], speed: f32) -> Result<(), String> {
        // 简单顺序回放，按 time_offset_ms 差值控制节奏
        let mut last_ms = 0u64;
        for evt in events {
            let delay_ms = ((evt.time_offset_ms.saturating_sub(last_ms) as f32) / speed) as u64;
            if delay_ms > 0 {
                thread::sleep(Duration::from_millis(delay_ms));
            }
            send_event(&evt.event_type, evt.x, evt.y)?;
            last_ms = evt.time_offset_ms;
        }
        Ok(())
    }
}

/// 启动录制（macOS）。
pub fn start_recording() -> Result<(), String> {
    MAC_RECORDER.start()
}

/// 停止录制（macOS）。
pub fn stop_recording() -> Result<Vec<RecordedEvent>, String> {
    MAC_RECORDER.stop()
}

/// 重建注册 macOS 全局快捷键（包含“双击修饰键”）。
/// <p>
/// 说明：为减少状态复杂度，这里采用“先全部取消，再全部注册”的策略。
/// </p>
pub fn register_macos_hotkeys(app: &tauri::AppHandle, app_data_dir: &Path) -> Result<(), String> {
    use std::collections::{HashMap as StdHashMap, HashSet};

    let settings = settings::load_settings(app_data_dir)?;

    // 重建全部快捷键：简单可靠，避免维护增量状态（同时也避免遗留无效旧快捷键）
    app.global_shortcut().unregister_all().ok();

    let mut used: HashSet<String> = HashSet::new();
    let mut double_bindings: StdHashMap<DoubleTapModifier, DoubleTapAction> = StdHashMap::new();

    fn normalize_double_modifier(name: &str) -> Option<DoubleTapModifier> {
        match name {
            "Meta" | "Command" | "Cmd" => Some(DoubleTapModifier::Meta),
            "Ctrl" | "Control" => Some(DoubleTapModifier::Ctrl),
            "Alt" | "Option" => Some(DoubleTapModifier::Alt),
            "Shift" => Some(DoubleTapModifier::Shift),
            _ => None,
        }
    }

    /// 判断该配置是否为“双击修饰键”（例如 Ctrl+Ctrl / Meta+Meta）。
    /// <p>
    /// 前端录制逻辑会将该类型配置保存为：
    /// - modifiers: ["Ctrl", "Ctrl"]
    /// - key: "Ctrl"
    /// </p>
    fn parse_double_modifier(config: &settings::HotkeyConfig) -> Option<DoubleTapModifier> {
        if config.modifiers.len() != 2 {
            return None;
        }
        let m0 = normalize_double_modifier(config.modifiers.get(0)?.as_str())?;
        let m1 = normalize_double_modifier(config.modifiers.get(1)?.as_str())?;
        let k = normalize_double_modifier(config.key.as_str())?;
        if m0 == m1 && m0 == k {
            Some(m0)
        } else {
            None
        }
    }

    fn insert_double_binding(
        map: &mut StdHashMap<DoubleTapModifier, DoubleTapAction>,
        modifier: DoubleTapModifier,
        action: DoubleTapAction,
        who: &str,
    ) {
        if map.contains_key(&modifier) {
            eprintln!(
                "[macOS] 检测到重复修饰键快捷键冲突，已跳过: {} -> {:?}",
                who, modifier
            );
            return;
        }
        map.insert(modifier, action);
    }

    // 1) 启动器快捷键
    match settings.hotkey.as_ref() {
        Some(cfg) => {
            if let Some(modifier) = parse_double_modifier(cfg) {
                insert_double_binding(
                    &mut double_bindings,
                    modifier,
                    DoubleTapAction::ToggleLauncher,
                    "launcher",
                );
            } else {
                let launcher_shortcut = hotkey_config_to_shortcut_string(cfg).unwrap_or_else(|e| {
                    eprintln!("[macOS] 启动器快捷键配置无效，将使用默认 Command+Space: {}", e);
                    "Command+Space".to_string()
                });

                if used.insert(launcher_shortcut.clone()) {
                    app.global_shortcut()
                        .on_shortcut(launcher_shortcut.as_str(), move |app, _sc, event| {
                            if event.state == ShortcutState::Pressed {
                                let app_handle = app.clone();
                                tauri::async_runtime::spawn(async move {
                                    let _ = crate::commands::toggle_launcher(app_handle);
                                });
                            }
                        })
                        .map_err(|e| format!("注册启动器快捷键失败: {e}"))?;
                }
            }
        }
        None => {
            // 兼容：无配置时保留默认 Command+Space
            let launcher_shortcut = "Command+Space".to_string();
            if used.insert(launcher_shortcut.clone()) {
                app.global_shortcut()
                    .on_shortcut(launcher_shortcut.as_str(), move |app, _sc, event| {
                        if event.state == ShortcutState::Pressed {
                            let app_handle = app.clone();
                            tauri::async_runtime::spawn(async move {
                                let _ = crate::commands::toggle_launcher(app_handle);
                            });
                        }
                    })
                    .map_err(|e| format!("注册启动器快捷键失败: {e}"))?;
            }
        }
    }

    // 2) 应用中心快捷键
    if let Some(cfg) = settings.app_center_hotkey.as_ref() {
        if let Some(modifier) = parse_double_modifier(cfg) {
            insert_double_binding(
                &mut double_bindings,
                modifier,
                DoubleTapAction::ShowAppCenter,
                "app-center",
            );
        } else {
            match hotkey_config_to_shortcut_string(cfg) {
                Ok(shortcut) => {
                    if used.insert(shortcut.clone()) {
                        app.global_shortcut()
                            .on_shortcut(shortcut.as_str(), move |app, _sc, event| {
                                if event.state == ShortcutState::Pressed {
                                    let app_handle = app.clone();
                                    tauri::async_runtime::spawn(async move {
                                        let _ = crate::commands::show_plugin_list_window(app_handle).await;
                                    });
                                }
                            })
                            .map_err(|e| format!("注册应用中心快捷键失败: {e}"))?;
                    }
                }
                Err(e) => {
                    eprintln!("[macOS] 应用中心快捷键配置无效，已跳过: {}", e);
                }
            }
        }
    }

    // 3) 插件快捷键
    for (plugin_id, cfg) in settings.plugin_hotkeys.iter() {
        if let Some(modifier) = parse_double_modifier(cfg) {
            insert_double_binding(
                &mut double_bindings,
                modifier,
                DoubleTapAction::TriggerPlugin(plugin_id.clone()),
                plugin_id,
            );
            continue;
        }

        let shortcut = match hotkey_config_to_shortcut_string(cfg) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[macOS] 插件快捷键配置无效，已跳过 ({}): {}", plugin_id, e);
                continue;
            }
        };

        if !used.insert(shortcut.clone()) {
            eprintln!(
                "[macOS] 插件快捷键与已注册快捷键冲突，已跳过: {} -> {}",
                plugin_id, shortcut
            );
            continue;
        }

        let plugin_id_owned = plugin_id.clone();
        app.global_shortcut()
            .on_shortcut(shortcut.as_str(), move |app, _sc, event| {
                if event.state == ShortcutState::Pressed {
                    let _ = app.emit("plugin-hotkey-triggered", plugin_id_owned.clone());
                }
            })
            .map_err(|e| format!("注册插件快捷键失败 ({}): {}", plugin_id, e))?;
    }

    // 4) 应用快捷键（用 app:<path> 作为语义 ID，与 Windows 保持一致）
    for (app_path, cfg) in settings.app_hotkeys.iter() {
        if let Some(modifier) = parse_double_modifier(cfg) {
            insert_double_binding(
                &mut double_bindings,
                modifier,
                DoubleTapAction::LaunchApp(app_path.clone()),
                app_path,
            );
            continue;
        }

        let shortcut = match hotkey_config_to_shortcut_string(cfg) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[macOS] 应用快捷键配置无效，已跳过 ({}): {}", app_path, e);
                continue;
            }
        };

        if !used.insert(shortcut.clone()) {
            eprintln!(
                "[macOS] 应用快捷键与已注册快捷键冲突，已跳过: {} -> {}",
                app_path, shortcut
            );
            continue;
        }

        let app_path_owned = app_path.clone();
        app.global_shortcut()
            .on_shortcut(shortcut.as_str(), move |_app, _sc, event| {
                if event.state == ShortcutState::Pressed {
                    let info = crate::app_search::AppInfo {
                        name: String::new(),
                        path: app_path_owned.clone(),
                        icon: None,
                        description: None,
                        name_pinyin: None,
                        name_pinyin_initials: None,
                    };
                    let _ = crate::app_search::windows::launch_app(&info);
                }
            })
            .map_err(|e| format!("注册应用快捷键失败 ({}): {}", app_path, e))?;
    }

    // 5) 双击修饰键快捷键（Ctrl+Ctrl / Meta+Meta ...）
    // 说明：该类型无法用 Accelerator 字符串表达，需依赖 rdev 全局监听。
    let need_double_tap = !double_bindings.is_empty();
    let was_double_tap_active = MACOS_DOUBLE_TAP_ACTIVE.load(Ordering::Relaxed);
    if need_double_tap || was_double_tap_active {
        let _ = LazyLock::force(&MAC_RECORDER);
        MAC_RECORDER.update_double_tap_bindings(app.clone(), double_bindings);
        MACOS_DOUBLE_TAP_ACTIVE.store(need_double_tap, Ordering::Relaxed);
    }

    Ok(())
}

fn hotkey_config_to_shortcut_string(config: &settings::HotkeyConfig) -> Result<String, String> {
    // 双击修饰键（如 Ctrl+Ctrl / Meta+Meta）无法用 Accelerator 字符串表达，这里显式拒绝，
    // 避免给用户造成“保存成功但永远不触发”的错觉。
    if config.modifiers.len() == 2
        && config.modifiers[0] == config.modifiers[1]
        && config.modifiers[0] == config.key
    {
        return Err(format!(
            "macOS 暂不支持重复修饰键快捷键（例如 {}+{}）",
            config.modifiers[0], config.modifiers[1]
        ));
    }

    let mut has_command = false;
    let mut has_control = false;
    let mut has_alt = false;
    let mut has_shift = false;

    for m in &config.modifiers {
        match m.as_str() {
            "Meta" | "Command" | "Cmd" => has_command = true,
            "Ctrl" | "Control" => has_control = true,
            "Alt" | "Option" => has_alt = true,
            "Shift" => has_shift = true,
            other => {
                return Err(format!("无法识别的修饰键: {}", other));
            }
        }
    }

    let mut parts: Vec<String> = Vec::new();
    if has_command {
        parts.push("Command".to_string());
    }
    if has_control {
        parts.push("Control".to_string());
    }
    if has_alt {
        parts.push("Alt".to_string());
    }
    if has_shift {
        parts.push("Shift".to_string());
    }

    if parts.is_empty() {
        return Err("快捷键必须包含至少一个修饰键".to_string());
    }

    let key = config.key.trim();
    if key.is_empty() {
        return Err("快捷键按键不能为空".to_string());
    }

    parts.push(key.to_string());
    Ok(parts.join("+"))
}

fn send_event(event: &EventType, x: Option<i32>, y: Option<i32>) -> Result<(), String> {
    let ev = match event {
        EventType::MouseMove => {
            if let (Some(x), Some(y)) = (x, y) {
                RdevEvent::MouseMove {
                    x: x as f64,
                    y: y as f64,
                }
            } else {
                return Err("缺少鼠标坐标".to_string());
            }
        }
        EventType::MouseDown { button } => RdevEvent::ButtonPress(to_button(*button)),
        EventType::MouseUp { button } => RdevEvent::ButtonRelease(to_button(*button)),
        EventType::MouseWheel { delta } => RdevEvent::Wheel {
            delta_x: 0,
            delta_y: *delta as i64,
        },
        EventType::KeyDown { vk_code } => RdevEvent::KeyPress(to_key(*vk_code)),
        EventType::KeyUp { vk_code } => RdevEvent::KeyRelease(to_key(*vk_code)),
    };

    simulate(&ev).map_err(|e| match e {
        SimulateError => "发送事件失败".to_string(),
    })?;
    // 给系统一点时间处理事件
    thread::sleep(Duration::from_millis(2));
    Ok(())
}

fn to_button(btn: MouseButton) -> Button {
    match btn {
        MouseButton::Left => Button::Left,
        MouseButton::Right => Button::Right,
        MouseButton::Middle => Button::Middle,
    }
}

fn to_key(vk: u32) -> Key {
    // 粗略映射，常用键位覆盖；复杂映射可按需扩展
    match vk {
        0x41 => Key::KeyA,
        0x42 => Key::KeyB,
        0x43 => Key::KeyC,
        0x44 => Key::KeyD,
        0x45 => Key::KeyE,
        0x46 => Key::KeyF,
        0x47 => Key::KeyG,
        0x48 => Key::KeyH,
        0x49 => Key::KeyI,
        0x4A => Key::KeyJ,
        0x4B => Key::KeyK,
        0x4C => Key::KeyL,
        0x4D => Key::KeyM,
        0x4E => Key::KeyN,
        0x4F => Key::KeyO,
        0x50 => Key::KeyP,
        0x51 => Key::KeyQ,
        0x52 => Key::KeyR,
        0x53 => Key::KeyS,
        0x54 => Key::KeyT,
        0x55 => Key::KeyU,
        0x56 => Key::KeyV,
        0x57 => Key::KeyW,
        0x58 => Key::KeyX,
        0x59 => Key::KeyY,
        0x5A => Key::KeyZ,
        0x30 => Key::Num0,
        0x31 => Key::Num1,
        0x32 => Key::Num2,
        0x33 => Key::Num3,
        0x34 => Key::Num4,
        0x35 => Key::Num5,
        0x36 => Key::Num6,
        0x37 => Key::Num7,
        0x38 => Key::Num8,
        0x39 => Key::Num9,
        0x20 => Key::Space,
        0x0D => Key::Return,
        0x1B => Key::Escape,
        0x08 => Key::Backspace,
        0x2E => Key::Delete,
        0x28 => Key::DownArrow,
        0x26 => Key::UpArrow,
        0x25 => Key::LeftArrow,
        0x27 => Key::RightArrow,
        0x10 => Key::ShiftLeft,
        0x11 => Key::ControlLeft,
        0x12 => Key::Alt,
        _ => Key::Unknown(0),
    }
}

/// 录制监听入口：调用方需在后台启动一个线程 listen，写入 state
pub fn start_rdev_listener(
    state: Arc<Mutex<RecordingState>>,
    double_tap: Arc<Mutex<DoubleTapHotkeyManager>>,
) {
    thread::spawn(move || {
        let res = listen(move |event: Event| {
            // 1) 双击修饰键检测：不依赖录制状态
            let triggered = {
                let mut guard = match double_tap.lock() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                guard.process_event(&event)
            };
            if let Some((app, action)) = triggered {
                execute_double_tap_action(app, action);
            }

            // 2) 录制：仅在录制状态下写入事件
            let mut guard = match state.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            if !guard.is_recording {
                return;
            }
            let offset = guard
                .start
                .map(|s| s.elapsed().as_millis() as u64)
                .unwrap_or(0);
            if let Some(rec) = map_event(event, offset) {
                guard.events.push(rec);
            }
        });

        if let Err(e) = res {
            eprintln!("[macOS] rdev 监听启动失败（可能缺少“输入监控/辅助功能”权限）: {:?}", e);
        }
    });
}

fn map_event(event: Event, offset_ms: u64) -> Option<RecordedEvent> {
    match event.event_type {
        RdevEvent::MouseMove { x, y } => Some(RecordedEvent {
            event_type: EventType::MouseMove,
            x: Some(x as i32),
            y: Some(y as i32),
            time_offset_ms: offset_ms,
        }),
        RdevEvent::ButtonPress(btn) => Some(RecordedEvent {
            event_type: EventType::MouseDown {
                button: from_button(btn),
            },
            x: None,
            y: None,
            time_offset_ms: offset_ms,
        }),
        RdevEvent::ButtonRelease(btn) => Some(RecordedEvent {
            event_type: EventType::MouseUp {
                button: from_button(btn),
            },
            x: None,
            y: None,
            time_offset_ms: offset_ms,
        }),
        RdevEvent::Wheel { delta_x: _, delta_y } => Some(RecordedEvent {
            event_type: EventType::MouseWheel {
                delta: delta_y as i32,
            },
            x: None,
            y: None,
            time_offset_ms: offset_ms,
        }),
        RdevEvent::KeyPress(key) => Some(RecordedEvent {
            event_type: EventType::KeyDown {
                vk_code: from_key(key),
            },
            x: None,
            y: None,
            time_offset_ms: offset_ms,
        }),
        RdevEvent::KeyRelease(key) => Some(RecordedEvent {
            event_type: EventType::KeyUp {
                vk_code: from_key(key),
            },
            x: None,
            y: None,
            time_offset_ms: offset_ms,
        }),
        _ => None,
    }
}

fn from_button(btn: Button) -> MouseButton {
    match btn {
        Button::Left => MouseButton::Left,
        Button::Right => MouseButton::Right,
        Button::Middle => MouseButton::Middle,
        _ => MouseButton::Left,
    }
}

fn from_key(key: Key) -> u32 {
    match key {
        Key::KeyA => 0x41,
        Key::KeyB => 0x42,
        Key::KeyC => 0x43,
        Key::KeyD => 0x44,
        Key::KeyE => 0x45,
        Key::KeyF => 0x46,
        Key::KeyG => 0x47,
        Key::KeyH => 0x48,
        Key::KeyI => 0x49,
        Key::KeyJ => 0x4A,
        Key::KeyK => 0x4B,
        Key::KeyL => 0x4C,
        Key::KeyM => 0x4D,
        Key::KeyN => 0x4E,
        Key::KeyO => 0x4F,
        Key::KeyP => 0x50,
        Key::KeyQ => 0x51,
        Key::KeyR => 0x52,
        Key::KeyS => 0x53,
        Key::KeyT => 0x54,
        Key::KeyU => 0x55,
        Key::KeyV => 0x56,
        Key::KeyW => 0x57,
        Key::KeyX => 0x58,
        Key::KeyY => 0x59,
        Key::KeyZ => 0x5A,
        Key::Num0 => 0x30,
        Key::Num1 => 0x31,
        Key::Num2 => 0x32,
        Key::Num3 => 0x33,
        Key::Num4 => 0x34,
        Key::Num5 => 0x35,
        Key::Num6 => 0x36,
        Key::Num7 => 0x37,
        Key::Num8 => 0x38,
        Key::Num9 => 0x39,
        Key::Space => 0x20,
        Key::Return => 0x0D,
        Key::Escape => 0x1B,
        Key::Backspace => 0x08,
        Key::Delete => 0x2E,
        Key::LeftArrow => 0x25,
        Key::RightArrow => 0x27,
        Key::UpArrow => 0x26,
        Key::DownArrow => 0x28,
        Key::ShiftLeft => 0x10,
        Key::ControlLeft => 0x11,
        Key::Alt => 0x12,
        _ => 0,
    }
}
