#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod commands;

use auto_launch::AutoLaunchBuilder;
use commands::{get_shortcut, paste, toggle_pause, update_shortcut, HotkeyConfig, PasteState};
use std::{env, sync::Mutex};
use tauri::{
    CustomMenuItem, GlobalShortcutManager, Manager, SystemTray, SystemTrayEvent, SystemTrayMenu, SystemTrayMenuItem,
};

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

#[tokio::main]
async fn main() {
    let auto_start = AutoLaunchBuilder::new()
        .set_app_name("Paster")
        .set_app_path(&env::current_exe().unwrap().to_str().unwrap())
        .build()
        .unwrap();

    // 创建系统托盘菜单
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
        .manage(Mutex::new(PasteState::new()))
        .manage(Mutex::new(GlobalShortcutState::new()))
        .system_tray(tray)
        .on_system_tray_event(|app, event| match event {
            SystemTrayEvent::LeftClick { .. } => {
                let window = app.get_window("main").unwrap();
                if window.is_visible().unwrap() {
                    window.hide().unwrap();
                } else {
                    window.show().unwrap();
                    window.set_focus().unwrap();
                }
            }
            SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
                "quit" => {
                    std::process::exit(0);
                }
                "show" => {
                    let window = app.get_window("main").unwrap();
                    window.show().unwrap();
                    window.set_focus().unwrap();
                }
                "pause" => {
                    let state = app.state::<Mutex<PasteState>>();
                    let mut state = state.lock().unwrap();
                    state.is_paused = !state.is_paused;
                    
                    // 更新菜单项文本
                    let tray_handle = app.tray_handle();
                    if state.is_paused {
                        tray_handle
                            .get_item("pause")
                            .set_title("继续")
                            .unwrap();
                    } else {
                        tray_handle
                            .get_item("pause")
                            .set_title("暂停")
                            .unwrap();
                    }
                }
                _ => {}
            },
            _ => {}
        })
        .setup(move |app| {
            // 获取初始快捷键设置
            let paste_state = app.state::<Mutex<PasteState>>();
            let shortcut_config = {
                let state = paste_state.lock().unwrap();
                state.shortcut.clone()
            };
            
            // 注册全局快捷键
            register_global_shortcut(app.app_handle(), &shortcut_config)?;
            
            // 监听快捷键更新事件
            let app_handle = app.app_handle();
            app.listen_global("update-hotkey", move |event| {
                if let Some(payload) = event.payload() {
                    if let Ok(config) = serde_json::from_str::<HotkeyConfig>(payload) {
                        // 注册新的快捷键
                        let _ = register_global_shortcut(app_handle.clone(), &config);
                    }
                }
            });

            // 设置窗口关闭行为（隐藏而不是退出）
            {
                let window = app.get_window("main").unwrap();
                let window_handle = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        window_handle.hide().unwrap();
                    }
                });
            }

            // 尝试启用开机自启动
            if !auto_start.is_enabled().unwrap() {
                let _ = auto_start.enable();
            }
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![paste, toggle_pause, get_shortcut, update_shortcut])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn register_global_shortcut(app_handle: tauri::AppHandle, config: &HotkeyConfig) -> Result<(), String> {
    // 获取快捷键管理器状态
    let shortcut_state = app_handle.state::<Mutex<GlobalShortcutState>>();
    let mut shortcut_state = shortcut_state.lock().unwrap();
    
    // 获取快捷键字符串
    let accelerator = config.to_tauri_accelerator();
    
    // 如果已注册其他快捷键，则先注销
    if let Some(old_accelerator) = &shortcut_state.registered_shortcut {
        let _ = app_handle.global_shortcut_manager().unregister(old_accelerator);
    }
    
    // 克隆app_handle用于闭包
    let app_handle_clone = app_handle.clone();
    let paste_handler = move || {
        let state = app_handle_clone.state::<Mutex<PasteState>>();
        let is_paused = {
            let state = state.lock().unwrap();
            state.is_paused
        };
        
        if !is_paused {
            let window = app_handle_clone.get_window("main").unwrap();
            window.emit("trigger-paste", ()).unwrap();
        }
    };
    
    match app_handle.global_shortcut_manager().register(&accelerator, paste_handler) {
        Ok(_) => {
            shortcut_state.registered_shortcut = Some(accelerator);
            Ok(())
        },
        Err(e) => Err(e.to_string())
    }
}
