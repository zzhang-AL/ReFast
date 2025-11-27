use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppInfo {
    pub name: String,
    pub path: String,
    pub icon: Option<String>,
    pub description: Option<String>,
}

#[cfg(target_os = "windows")]
pub mod windows {
    use super::*;
    use std::env;

    // Windows-specific implementation
    pub fn scan_start_menu() -> Result<Vec<AppInfo>, String> {
        let mut apps = Vec::new();

        // Common start menu paths - only scan user's start menu for speed
        let start_menu_paths = vec![
            env::var("APPDATA")
                .ok()
                .map(|p| PathBuf::from(p).join("Microsoft/Windows/Start Menu/Programs")),
            // Skip PROGRAMDATA for now - it's slower and less commonly used
            // env::var("PROGRAMDATA")
            //     .ok()
            //     .map(|p| PathBuf::from(p).join("Microsoft/Windows/Start Menu/Programs")),
        ];

        for start_menu_path in start_menu_paths.into_iter().flatten() {
            if start_menu_path.exists() {
                // Start scanning from depth 0, limit to 2 levels for speed
                if let Err(_) = scan_directory(&start_menu_path, &mut apps, 0) {
                    // Continue on error
                }
            }
        }

        // Remove duplicates based on name
        apps.sort_by(|a, b| a.name.cmp(&b.name));
        apps.dedup_by(|a, b| a.name == b.name);

        Ok(apps)
    }

    fn scan_directory(dir: &Path, apps: &mut Vec<AppInfo>, depth: usize) -> Result<(), String> {
        // Limit recursion depth to avoid scanning too deep (reduced to 2 for speed)
        const MAX_DEPTH: usize = 2;
        if depth > MAX_DEPTH {
            return Ok(());
        }
        
        // Limit total number of apps to avoid memory issues (reduced for speed)
        const MAX_APPS: usize = 500;
        if apps.len() >= MAX_APPS {
            return Ok(());
        }

        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => return Ok(()), // Skip directories we can't read
        };

        for entry in entries {
            if apps.len() >= MAX_APPS {
                break;
            }
            
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue, // Skip entries we can't read
            };
            let path = entry.path();

            if path.is_dir() {
                // Recursively scan subdirectories
                if let Err(_) = scan_directory(&path, apps, depth + 1) {
                    // Continue on error
                }
            } else if path.extension().and_then(|s| s.to_str()) == Some("lnk") {
                // Skip .lnk parsing for now - it's too slow with PowerShell
                // Just use the .lnk filename as the app name
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    apps.push(AppInfo {
                        name: name.to_string(),
                        path: path.to_string_lossy().to_string(),
                        icon: None,
                        description: None,
                    });
                }
                // TODO: Optionally parse .lnk files in background for better results
                // if let Ok(app_info) = parse_lnk_file(&path) {
                //     apps.push(app_info);
                // }
            } else if path.extension().and_then(|s| s.to_str()) == Some("exe") {
                // Direct executable
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    apps.push(AppInfo {
                        name: name.to_string(),
                        path: path.to_string_lossy().to_string(),
                        icon: None,
                        description: None,
                    });
                }
            }
        }

        Ok(())
    }

    fn parse_lnk_file(lnk_path: &Path) -> Result<AppInfo, String> {
        // Use PowerShell to resolve .lnk file target
        let path_str = lnk_path.to_string_lossy().replace('\'', "''"); // Escape single quotes for PowerShell
        let ps_command = format!(
            r#"$shell = New-Object -ComObject WScript.Shell; $shortcut = $shell.CreateShortcut('{}'); $shortcut.TargetPath"#,
            path_str
        );

        // Add timeout to PowerShell command to avoid hanging
        let output = Command::new("powershell")
            .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps_command])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .map_err(|e| format!("Failed to execute PowerShell: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "Failed to parse .lnk file: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let target_path = String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_string();

        if target_path.is_empty() {
            return Err("Empty target path".to_string());
        }

        // Check if target exists (it might be a relative path)
        let target = if Path::new(&target_path).exists() {
            target_path
        } else {
            // Try to resolve relative to the .lnk file's directory
            if let Some(parent) = lnk_path.parent() {
                let resolved = parent.join(&target_path);
                if resolved.exists() {
                    resolved.to_string_lossy().to_string()
                } else {
                    target_path // Return as-is, might be a system path
                }
            } else {
                target_path
            }
        };

        let name = lnk_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_string();

        Ok(AppInfo {
            name,
            path: target,
            icon: None,
            description: None,
        })
    }

    pub fn search_apps(query: &str, apps: &[AppInfo]) -> Vec<AppInfo> {
        if query.is_empty() {
            return apps.to_vec();
        }

        let query_lower = query.to_lowercase();
        let mut results: Vec<(AppInfo, i32)> = apps
            .iter()
            .filter_map(|app| {
                let name_lower = app.name.to_lowercase();
                let path_lower = app.path.to_lowercase();

                let mut score = 0;

                // Exact match gets highest score
                if name_lower == query_lower {
                    score += 1000;
                } else if name_lower.starts_with(&query_lower) {
                    score += 500;
                } else if name_lower.contains(&query_lower) {
                    score += 100;
                }

                // Path match gets lower score
                if path_lower.contains(&query_lower) {
                    score += 10;
                }

                if score > 0 {
                    Some((app.clone(), score))
                } else {
                    None
                }
            })
            .collect();

        // Sort by score (descending)
        results.sort_by(|a, b| b.1.cmp(&a.1));

        results.into_iter().map(|(app, _)| app).collect()
    }

    pub fn launch_app(app: &AppInfo) -> Result<(), String> {
        let path = Path::new(&app.path);
        
        if !path.exists() {
            return Err(format!("Application not found: {}", app.path));
        }

        // Use Windows ShellExecute or Command
        Command::new(&app.path)
            .spawn()
            .map_err(|e| format!("Failed to launch application: {}", e))?;

        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
pub mod windows {
    use super::*;

    pub fn scan_start_menu() -> Result<Vec<AppInfo>, String> {
        Err("App search is only supported on Windows".to_string())
    }

    pub fn search_apps(_query: &str, _apps: &[AppInfo]) -> Vec<AppInfo> {
        vec![]
    }

    pub fn launch_app(_app: &AppInfo) -> Result<(), String> {
        Err("App launch is only supported on Windows".to_string())
    }
}

