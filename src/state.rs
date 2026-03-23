use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub const APP_ID: &str = "dev.agzes.lbs";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TargetProcess {
    pub pid: u32,
    pub name: String,
    pub limit_percent: f64,
    pub is_active: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppState {
    #[serde(skip)]
    pub targets: Vec<TargetProcess>,
    pub awake_cycle_ms: u32,
    pub auto_start: bool,
    pub unlimit_at_focus: bool,
    pub close_to_tray: bool,
    pub allow_limit_99: bool,
    pub check_updates: bool,
    pub last_run_version: Option<String>,
    pub desktop_installed: bool,
    pub shown_warning: bool,
    
    #[serde(skip)]
    pub available_processes: Vec<(String, Vec<(u32, f64, bool)>, f64)>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            targets: vec![],
            awake_cycle_ms: 100,
            auto_start: false,
            unlimit_at_focus: true,
            close_to_tray: true,
            allow_limit_99: false,
            check_updates: true,
            last_run_version: None,
            desktop_installed: false,
            shown_warning: false,
            available_processes: vec![],
        }
    }
}

pub type SharedState = Arc<Mutex<AppState>>;

impl AppState {
    fn get_config_path() -> Option<PathBuf> {
        let mut path = dirs::config_dir()?;
        path.push("lbs");
        if !path.exists() {
            let _ = fs::create_dir_all(&path);
        }
        path.push("config.json");
        Some(path)
    }

    pub fn load() -> Self {
        if let Some(path) = Self::get_config_path() {
            if let Ok(data) = fs::read_to_string(path) {
                if let Ok(state) = serde_json::from_str::<AppState>(&data) {
                    return state;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        if let Some(path) = Self::get_config_path() {
            if let Ok(data) = serde_json::to_string_pretty(self) {
                let _ = fs::write(path, data);
            }
        }
    }
}
