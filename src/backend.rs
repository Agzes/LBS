use std::fs;
use std::thread;
use std::time::Duration;
use crate::state::SharedState;
use libc::{kill, SIGCONT, SIGSTOP};
use std::collections::HashMap;
use std::sync::Mutex;
use std::collections::HashSet;
use once_cell::sync::Lazy;
use std::process::Command;
use serde_json;

static RUNNING_LIMITERS: Lazy<Mutex<HashSet<u32>>> = Lazy::new(|| Mutex::new(HashSet::new()));
static CURRENT_FOCUSED_PID: Lazy<Mutex<u32>> = Lazy::new(|| Mutex::new(0));
static CURRENT_FOCUSED_NAME: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new(String::new()));

pub fn is_system_install() -> bool {
    if let Ok(exe) = std::env::current_exe() {
        return exe.starts_with("/usr/bin") || exe.starts_with("/usr/local/bin");
    }
    false
}

pub fn desktop_file_exists() -> bool {
    if is_system_install() { return true; }
    if let Some(data_dir) = dirs::data_local_dir() {
        let path = data_dir.join("applications/dev.agzes.lbs.desktop");
        return path.exists();
    }
    false
}

pub fn setup_desktop_file() -> bool {
    let data_dir = dirs::data_local_dir();
    let home_dir = dirs::home_dir();

    if let (Some(data), Some(home)) = (data_dir, home_dir) {
        let icons_dir = data.join("icons/hicolor/256x256/apps");
        let _ = fs::create_dir_all(&icons_dir);
        let icon_path = icons_dir.join("dev.agzes.lbs.png");
        let _ = fs::write(&icon_path, include_bytes!("../assets/logo.png"));

        
        let _ = Command::new("gtk-update-icon-cache")
            .args(["-f", "-t", &data.join("icons/hicolor").to_string_lossy()])
            .spawn();

        let bin_dir = home.join(".local/bin");
        let _ = fs::create_dir_all(&bin_dir);
        let target_bin = bin_dir.join("lbs");

        if let Ok(current_exe) = std::env::current_exe() {
            if let Ok(_) = fs::copy(&current_exe, &target_bin) {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(&target_bin, fs::Permissions::from_mode(0o755));
                }
            }
        }

        let apps_dir = data.join("applications");
        let _ = fs::create_dir_all(&apps_dir);
        let content = format!(
            "[Desktop Entry]\nName=Linux Battle Shaper\nComment=CPU Limiter with universal focus support\nExec={}\nIcon={}\nTerminal=false\nType=Application\nCategories=Utility;System;\nStartupNotify=true\n",
            target_bin.display(),
            icon_path.display()
        );
        return fs::write(apps_dir.join("dev.agzes.lbs.desktop"), content).is_ok();
    }
    false
}

pub fn sync_binary() {
    if is_system_install() { return; }
    if let (Ok(current_exe), Some(home)) = (std::env::current_exe(), dirs::home_dir()) {
        let bin_dir = home.join(".local/bin");
        let _ = fs::create_dir_all(&bin_dir);
        let target_bin = bin_dir.join("lbs");
        
        if current_exe == target_bin { return; }

        let should_copy = if !target_bin.exists() {
            true
        } else {
            match (fs::metadata(&current_exe), fs::metadata(&target_bin)) {
                (Ok(m1), Ok(m2)) => m1.len() != m2.len(),
                _ => true,
            }
        };

        if should_copy {
            if let Ok(_) = fs::copy(&current_exe, &target_bin) {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(&target_bin, fs::Permissions::from_mode(0o755));
                }
            }
        }
    }
}

pub fn add_target(pid: u32, name: &str, limit_percent: f64, state: SharedState) {
    if let Ok(mut s) = state.lock() {
        if !s.targets.iter().any(|t| t.pid == pid && t.name == name) {
            s.targets.push(crate::state::TargetProcess {
                pid,
                name: name.to_string(),
                limit_percent,
                is_active: true,
            });
            s.save();
            start_limiter(state.clone(), pid, name.to_string());
        }
    }
}

pub fn is_hyprland() -> bool {
    std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok()
}

pub fn uninstall() -> bool {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return false,
    };
    let data = match dirs::data_local_dir() {
        Some(d) => d,
        None => return false,
    };

    let bin_path = home.join(".local/bin/lbs");
    if bin_path.exists() {
        let _ = fs::remove_file(bin_path);
    }

    let desktop_path = data.join("applications/dev.agzes.lbs.desktop");
    if desktop_path.exists() {
        let _ = fs::remove_file(desktop_path);
    }

    let icon_path = data.join("icons/hicolor/256x256/apps/dev.agzes.lbs.png");
    if icon_path.exists() {
        let _ = fs::remove_file(icon_path);
    }

    if let Some(config_dir) = dirs::config_dir() {
        let env_file = config_dir.join("environment.d/10-lbs.conf");
        if env_file.exists() {
            let _ = fs::remove_file(env_file);
        }

        let lbs_config = config_dir.join("lbs");
        if lbs_config.exists() {
            let _ = fs::remove_dir_all(lbs_config);
        }
    }

    true
}

pub fn cleanup_all_limiters() {
    let running = RUNNING_LIMITERS.lock().unwrap();
    for &pid in running.iter() {
        unsafe { kill(pid as i32, SIGCONT); }
    }
}

pub fn start_focus_monitor() {
    thread::spawn(move || {
        loop {
            let mut active_pid = 0;
            let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase();
            let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default().to_lowercase();

            if is_hyprland() {
                if let Ok(output) = Command::new("hyprctl").args(["activewindow", "-j"]).output() {
                    if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                        active_pid = json["pid"].as_u64().unwrap_or(0) as u32;
                    }
                }
            } 
            else if std::env::var("SWAYSOCK").is_ok() {
                if let Ok(output) = Command::new("swaymsg").args(["-t", "get_tree"]).output() {
                    if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                        fn find_focused(node: &serde_json::Value) -> u32 {
                            if node["focused"].as_bool().unwrap_or(false) {
                                return node["pid"].as_u64().unwrap_or(0) as u32;
                            }
                            if let Some(nodes) = node["nodes"].as_array() {
                                for n in nodes {
                                    let p = find_focused(n);
                                    if p != 0 { return p; }
                                }
                            }
                            if let Some(nodes) = node["floating_nodes"].as_array() {
                                for n in nodes {
                                    let p = find_focused(n);
                                    if p != 0 { return p; }
                                }
                            }
                            0
                        }
                        active_pid = find_focused(&json);
                    }
                }
            }
            else if desktop.contains("kde") && session_type == "wayland" {
                if let Ok(output) = Command::new("dbus-send")
                    .args(["--session", "--print-reply", "--dest=org.kde.KWin", "/KWin", "org.kde.KWin.activeWindow"])
                    .output() {
                    let out = String::from_utf8_lossy(&output.stdout);
                    if let Some(obj_path) = out.split("objpath \"").nth(1).and_then(|s| s.split('\"').next()) {
                        if let Ok(p_out) = Command::new("dbus-send")
                            .args(["--session", "--print-reply", "--dest=org.kde.KWin", obj_path, "org.freedesktop.DBus.Properties.Get", "string:org.kde.KWin.Window", "string:pid"])
                            .output() {
                            let p_str = String::from_utf8_lossy(&p_out.stdout);
                            if let Some(val) = p_str.split("variant          uint32 ").nth(1) {
                                active_pid = val.trim().parse::<u32>().unwrap_or(0);
                            }
                        }
                    }
                }
            }

            if active_pid == 0 {
                if let Ok(output) = Command::new("xprop").args(["-root", "_NET_ACTIVE_WINDOW"]).output() {
                    let out_str = String::from_utf8_lossy(&output.stdout);
                    if let Some(id_part) = out_str.split("window id # ").nth(1) {
                        let window_id = id_part.split(',').next().unwrap_or("").trim();
                        if !window_id.is_empty() {
                            if let Ok(pid_output) = Command::new("xprop").args(["-id", window_id, "_NET_WM_PID"]).output() {
                                let pid_str = String::from_utf8_lossy(&pid_output.stdout);
                                if let Some(p) = pid_str.split("= ").nth(1) {
                                    active_pid = p.trim().parse::<u32>().unwrap_or(0);
                                }
                            }
                        }
                    }
                }
            }

            if active_pid != 0 {
                if let Ok(mut focused_pid) = CURRENT_FOCUSED_PID.lock() {
                    *focused_pid = active_pid;
                }
                
                if let Ok(comm) = fs::read_to_string(format!("/proc/{}/comm", active_pid)) {
                    if let Ok(mut focused_name) = CURRENT_FOCUSED_NAME.lock() {
                        *focused_name = comm.trim().to_string();
                    }
                }
            }

            thread::sleep(Duration::from_millis(500));
        }
    });
}

pub fn get_cpu_usage() -> HashMap<u32, f64> {
    let mut usage = HashMap::new();
    let mut first_sample = HashMap::new();
    
    if let Ok(entries) = fs::read_dir("/proc") {
        for entry in entries.flatten() {
            if let Some(pid) = entry.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) {
                if let Ok(stat) = fs::read_to_string(format!("/proc/{}/stat", pid)) {
                    let parts: Vec<&str> = stat.split_whitespace().collect();
                    if parts.len() > 14 {
                        let utime: u64 = parts[13].parse().unwrap_or(0);
                        let stime: u64 = parts[14].parse().unwrap_or(0);
                        first_sample.insert(pid, utime + stime);
                    }
                }
            }
        }
    }

    thread::sleep(Duration::from_millis(200));

    if let Ok(entries) = fs::read_dir("/proc") {
        for entry in entries.flatten() {
            if let Some(pid) = entry.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) {
                if let Ok(stat) = fs::read_to_string(format!("/proc/{}/stat", pid)) {
                    let parts: Vec<&str> = stat.split_whitespace().collect();
                    if parts.len() > 14 {
                        let utime: u64 = parts[13].parse().unwrap_or(0);
                        let stime: u64 = parts[14].parse().unwrap_or(0);
                        let total = utime + stime;
                        if let Some(prev_total) = first_sample.get(&pid) {
                            let diff = total.saturating_sub(*prev_total);
                            usage.insert(pid, diff as f64 / 2.0); 
                        }
                    }
                }
            }
        }
    }
    usage
}

pub fn list_grouped_processes() -> Vec<(String, Vec<(u32, f64, bool)>, f64)> {
    let cpu_map = get_cpu_usage();
    let mut groups: HashMap<String, Vec<(u32, f64, bool)>> = HashMap::new();
    
    if let Ok(entries) = fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let pid_str = entry.file_name();
            if let Some(pid) = pid_str.to_str().and_then(|s| s.parse::<u32>().ok()) {
                let comm_path = entry.path().join("comm");
                if let Ok(comm) = fs::read_to_string(comm_path) {
                    let name = comm.trim().to_string();
                    let cpu = *cpu_map.get(&pid).unwrap_or(&0.0);
                    let is_kernel = fs::read_link(format!("/proc/{}/exe", pid)).is_err();
                    groups.entry(name).or_default().push((pid, cpu, is_kernel));
                }
            }
        }
    }

    let mut result: Vec<(String, Vec<(u32, f64, bool)>, f64)> = groups
        .into_iter()
        .map(|(name, pids)| {
            let total_cpu: f64 = pids.iter().map(|(_, c, _)| c).sum();
            (name, pids, total_cpu)
        })
        .collect();

    result.sort_by(|a, b| {
        let a_is_system = a.1.iter().any(|(p, _, _)| *p < 1000);
        let b_is_system = b.1.iter().any(|(p, _, _)| *p < 1000);

        if a_is_system != b_is_system {
            return a_is_system.cmp(&b_is_system);
        }
        b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal)
    });
    result
}

pub fn start_process_scanner(state: SharedState) {
    let state_m = state.clone();
    thread::spawn(move || {
        loop {
            let proc_list = list_grouped_processes();
            {
                let mut s = state_m.lock().unwrap();
                s.available_processes = proc_list.clone();
            }
            
            let targets = {
                let s = state_m.lock().unwrap();
                s.targets.clone()
            };

            for target in &targets {
                if target.pid == 0 && target.is_active {
                    for (name, pids, _) in &proc_list {
                        if name == &target.name {
                            for (pid, _, _) in pids {
                                start_limiter(state_m.clone(), *pid, name.clone());
                            }
                        }
                    }
                } else if target.is_active {
                    start_limiter(state_m.clone(), target.pid, target.name.clone());
                }
            }

            thread::sleep(Duration::from_secs(2));
        }
    });
}

pub fn start_limiter(state: SharedState, pid: u32, name: String) {
    {
        let mut running = RUNNING_LIMITERS.lock().unwrap();
        if running.contains(&pid) { return; }
        running.insert(pid);
    }

    let state_c = state.clone();
    thread::spawn(move || {
        loop {
            let (limit_percent, awake_cycle_ms, is_active, unlimit_at_focus) = {
                let s = state_c.lock().unwrap();
                let target = s.targets.iter().find(|t| 
                    (t.pid == pid) || (t.pid == 0 && t.name == name)
                );
                match target {
                    Some(t) => (t.limit_percent, s.awake_cycle_ms, t.is_active, s.unlimit_at_focus),
                    None => {
                        unsafe { kill(pid as i32, SIGCONT); }
                        RUNNING_LIMITERS.lock().unwrap().remove(&pid);
                        return;
                    }
                }
            };

            let is_focused = {
                if unlimit_at_focus {
                    let focused_p = CURRENT_FOCUSED_PID.lock().unwrap();
                    let focused_n = CURRENT_FOCUSED_NAME.lock().unwrap();
                    *focused_p == pid || *focused_n == name
                } else {
                    false
                }
            };

            if !is_active || is_focused {
                unsafe { kill(pid as i32, SIGCONT); }
                thread::sleep(Duration::from_millis(500));
                continue;
            }
            
            if unsafe { kill(pid as i32, 0) } != 0 {
                RUNNING_LIMITERS.lock().unwrap().remove(&pid);
                return;
            }

            if limit_percent <= 0.0 {
                unsafe { kill(pid as i32, SIGCONT); }
                thread::sleep(Duration::from_millis(500));
                continue;
            }

            let cycle_us = (awake_cycle_ms as f64) * 1000.0;
            let sleep_us = (cycle_us * limit_percent) / 100.0;
            let awake_us = if cycle_us > sleep_us { cycle_us - sleep_us } else { 0.0 };

            if awake_us > 0.0 {
                unsafe { kill(pid as i32, SIGCONT); }
                thread::sleep(Duration::from_micros(awake_us as u64));
            }

            if sleep_us > 0.0 {
                unsafe { kill(pid as i32, SIGSTOP); }
                thread::sleep(Duration::from_micros(sleep_us as u64));
            }
        }
    });
}
