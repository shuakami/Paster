#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod commands;

use std::sync::Mutex;
use auto_launch::AutoLaunchBuilder;
use tauri::{
    CustomMenuItem, GlobalShortcutManager, Manager, SystemTray, SystemTrayEvent, SystemTrayMenu,
    SystemTrayMenuItem,
};
use commands::{paste, toggle_pause, get_shortcut, update_shortcut, restart_app, PasteState, HotkeyConfig};

/// 记录当前全局快捷键，以便下次更新或注销
struct GlobalShortcutState {
    registered_shortcut: Option<String>,
}

impl GlobalShortcutState {
    fn new() -> Self {
        Self {
            registered_shortcut: None,
        }
    }
}

/// 注册全局快捷键
pub fn register_global_shortcut(
    app_handle: tauri::AppHandle,
    config: &HotkeyConfig,
) -> Result<(), String> {
    let shortcut_state = app_handle.state::<Mutex<GlobalShortcutState>>();
    let mut locked_state = shortcut_state.lock().unwrap();

    // 根据当前配置生成要注册的加速器字符串
    let accelerator = config.to_tauri_accelerator();

    // 如果已注册过其他快捷键，则先注销
    if let Some(old_accel) = &locked_state.registered_shortcut {
        let _ = app_handle.global_shortcut_manager().unregister(old_accel);
    }

    let app_handle_clone = app_handle.clone();
    let paste_handler = move || {
        #[cfg(debug_assertions)]
        println!("全局快捷键被触发");
        
        let state = app_handle_clone.state::<Mutex<PasteState>>();
        let locked = state.lock().unwrap();
        if !locked.is_paused {
            let window = app_handle_clone.get_window("main").unwrap();
            let _ = window.emit("trigger-paste", ());
        } else {
            #[cfg(debug_assertions)]
            println!("应用已暂停，忽略快捷键");
        }
    };

    match app_handle
        .global_shortcut_manager()
        .register(&accelerator, paste_handler)
    {
        Ok(_) => {
            locked_state.registered_shortcut = Some(accelerator.clone());
            #[cfg(debug_assertions)]
            println!("全局快捷键 \"{}\" 已注册成功", accelerator);
            
            Ok(())
        }
        Err(e) => {
            #[cfg(debug_assertions)]
            println!("全局快捷键 \"{}\" 注册失败: {}", accelerator, e);
            
            Err(e.to_string())
        }
    }
}

/// 启动时从本地配置文件读取快捷键信息
fn load_shortcut_config(app_handle: &tauri::AppHandle) -> HotkeyConfig {
    use tauri::api::path::{BaseDirectory, resolve_path};
    use std::fs;

    let default = HotkeyConfig::default();

    let store_path = match resolve_path(
        &app_handle.config(),
        app_handle.package_info(),
        &app_handle.env(),
        "shortcut_config.json",
        Some(BaseDirectory::AppConfig),
    ) {
        Ok(path) => path,
        Err(e) => {
            #[cfg(debug_assertions)]
            eprintln!("获取app_config_dir失败: {}", e);
            
            return default;
        }
    };

    if !store_path.exists() {
        // 文件不存在就返回默认
        return default;
    }

    let content = match fs::read_to_string(&store_path) {
        Ok(s) => s,
        Err(e) => {
            #[cfg(debug_assertions)]
            eprintln!("读取配置文件失败: {}", e);
            
            return default;
        }
    };

    let config = match serde_json::from_str::<HotkeyConfig>(&content) {
        Ok(cfg) => cfg,
        Err(e) => {
            #[cfg(debug_assertions)]
            eprintln!("解析JSON失败: {}", e);
            
            return default;
        }
    };

    #[cfg(debug_assertions)]
    println!("已从 {} 读取快捷键配置: {:?}", store_path.display(), config);
    
    config
}

#[tokio::main]
async fn main() {
    let auto_start = AutoLaunchBuilder::new()
        .set_app_name("Paster")
        .set_app_path(std::env::current_exe().unwrap().to_str().unwrap())
        .set_args("--silent")
        .build()
        .unwrap();

    // 创建托盘
    let quit = CustomMenuItem::new("quit".to_string(), "退出");
    let show = CustomMenuItem::new("show".to_string(), "显示窗口");
    let pause = CustomMenuItem::new("pause".to_string(), "暂停");
    let tray_menu = SystemTrayMenu::new()
        .add_item(show)
        .add_item(pause)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(quit);
    let tray = SystemTray::new().with_menu(tray_menu);

    tauri::Builder::default()
        // 管理状态：PasteState & GlobalShortcutState
        .manage(Mutex::new(PasteState::new()))
        .manage(Mutex::new(GlobalShortcutState::new()))
        .system_tray(tray)
        .on_system_tray_event(|app, event| match event {
            // 左键单击：显示/隐藏窗口
            SystemTrayEvent::LeftClick { .. } => {
                let window = app.get_window("main").unwrap();
                if window.is_visible().unwrap() {
                    let _ = window.hide();
                } else {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            // 菜单点击
            SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
                "quit" => {
                    std::process::exit(0);
                }
                "show" => {
                    let window = app.get_window("main").unwrap();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
                "pause" => {
                    let state = app.state::<Mutex<PasteState>>();
                    let mut locked = state.lock().unwrap();
                    locked.is_paused = !locked.is_paused;

                    // 修改菜单项文字
                    let tray_handle = app.tray_handle();
                    if locked.is_paused {
                        let _ = tray_handle.get_item("pause").set_title("继续");
                    } else {
                        let _ = tray_handle.get_item("pause").set_title("暂停");
                    }
                }
                _ => {}
            },
            _ => {}
        })
        .setup(move |app| {
            // 1. 启动时先从文件读取快捷键，写入PasteState
            {
                let config = load_shortcut_config(&app.app_handle());
                let state = app.state::<Mutex<PasteState>>();
                let mut locked = state.lock().unwrap();
                locked.shortcut = config;
            }

            // 2. 注册全局快捷键
            {
                let state = app.state::<Mutex<PasteState>>();
                let config = {
                    let locked = state.lock().unwrap();
                    locked.shortcut.clone()
                };
                register_global_shortcut(app.app_handle().clone(), &config).ok();
            }

            // 3. 关闭主窗口时隐藏而非退出
            let window = app.get_window("main").unwrap();
            let window_clone = window.clone();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window_clone.hide();
                }
            });

            // 4. 设置开机自启
            if !auto_start.is_enabled().unwrap() {
                let _ = auto_start.enable();
            }
            
            // 5. 处理静默启动参数
            let matches = app.get_cli_matches().unwrap();
            let is_silent = matches.args.get("silent").and_then(|arg| arg.value.as_bool()).unwrap_or(false);
            
            // 如果启动参数包含 --silent，则隐藏窗口
            if is_silent {
                #[cfg(debug_assertions)]
                println!("以静默模式启动");
                
                let _ = window.hide();
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            paste,
            toggle_pause,
            get_shortcut,
            update_shortcut,
            restart_app
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
