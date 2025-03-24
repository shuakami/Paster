use std::ffi::c_void;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use serde::{Deserialize, Serialize};
use tauri::Manager;
use tokio::time::{sleep, Duration};
use windows::Win32::{
    Foundation::{HGLOBAL, HWND},
    System::{
        DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard},
        Memory::{GlobalLock, GlobalUnlock},
    },
    UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
        KEYEVENTF_UNICODE, VIRTUAL_KEY, VK_RETURN,
    },
};

/// 程序状态：包含是否暂停、快捷键信息、是否正在粘贴。
pub struct PasteState {
    pub is_paused: bool,
    pub shortcut: HotkeyConfig,
    pub is_pasting: AtomicBool, // 用于跟踪粘贴状态
}

impl PasteState {
    pub fn new() -> Self {
        Self {
            is_paused: false,
            shortcut: HotkeyConfig::default(),
            is_pasting: AtomicBool::new(false),
        }
    }
}

/// 快捷键配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub left_ctrl: bool,
    pub right_ctrl: bool,
    pub key: String,

    /// 新增字段：是否劫持系统的 Ctrl+V
    pub intercept_ctrl_v: bool,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            alt: true,
            ctrl: true,
            shift: false,
            left_ctrl: false,
            right_ctrl: false,
            key: "V".to_string(),
            intercept_ctrl_v: false,
        }
    }
}

impl HotkeyConfig {
    /// 转换为 Tauri 的加速器字符串 (如 "Alt+Control+V")。
    /// 若 intercept_ctrl_v 为 true，则无视其他组合键，直接返回 "Control+V"。
    pub fn to_tauri_accelerator(&self) -> String {
        // 如果勾选了"劫持系统 Ctrl+V"，则强制只注册 "Control+V"
        if self.intercept_ctrl_v {
            return "Control+V".to_string();
        }

        let mut parts = Vec::new();
        if self.alt {
            parts.push("Alt".to_string());
        }
        if self.ctrl {
            parts.push("Control".to_string());
        } else if self.left_ctrl {
            parts.push("ControlLeft".to_string());
        } else if self.right_ctrl {
            parts.push("ControlRight".to_string());
        }
        if self.shift {
            parts.push("Shift".to_string());
        }
        parts.push(self.key.clone());

        parts.join("+")
    }

    /// 用户可读的快捷键描述 (如 "Alt+Ctrl+V" 或 "Alt+左Ctrl+V")。
    /// 若 intercept_ctrl_v 为 true，则直接显示 "劫持系统Ctrl+V"。
    pub fn get_description(&self) -> String {
        if self.intercept_ctrl_v {
            return "系统Ctrl+V (已被劫持)".to_string();
        }

        let mut parts = Vec::new();
        if self.alt {
            parts.push("Alt".to_string());
        }
        if self.ctrl {
            parts.push("Ctrl".to_string());
        } else if self.left_ctrl {
            parts.push("左Ctrl".to_string());
        } else if self.right_ctrl {
            parts.push("右Ctrl".to_string());
        }
        if self.shift {
            parts.push("Shift".to_string());
        }
        parts.push(self.key.clone());

        parts.join("+")
    }
}

/// 打开剪贴板获取 UTF-16 内容
fn get_clipboard() -> Result<Vec<u16>, &'static str> {
    const CF_UNICODETEXT: u32 = 13;
    let mut result: Vec<u16> = vec![];

    unsafe {
        OpenClipboard(HWND(0)).or(Err("打开剪切板错误"))?;
        let hglb = GetClipboardData(CF_UNICODETEXT).map_err(|_| {
            let _ = CloseClipboard();
            "获取剪切板数据错误"
        })?;
        let locker = HGLOBAL(hglb.0 as *mut c_void);
        let raw_data = GlobalLock(locker);
        let data = raw_data as *const u16;
        let mut i = 0usize;

        loop {
            let item = *data.add(i);
            i += 1;
            if item == 0 {
                break;
            }
            // 舍弃 '\r'
            if item == 13 {
                continue;
            }
            result.push(item);
        }

        GlobalUnlock(locker).map_err(|_| {
            let _ = CloseClipboard();
            "解除剪切板锁定失败"
        })?;
        CloseClipboard().or(Err("关闭剪切板失败"))?;
    }

    Ok(result)
}

/// 粘贴命令：读取剪贴板，逐字符发送到前台
#[tauri::command]
pub async fn paste(stand: u32, float: u32, app_handle: tauri::AppHandle) -> Result<(), &'static str> {
    #[cfg(debug_assertions)]
    println!("paste函数被调用：stand={}, float={}", stand, float);

    // 获取状态
    let state = app_handle.state::<Mutex<PasteState>>();

    // 1. 是否暂停
    let is_paused = {
        let locked = state.lock().unwrap();
        locked.is_paused
    };
    if is_paused {
        #[cfg(debug_assertions)]
        println!("函数退出：功能已暂停");
        
        return Err("功能已暂停");
    }

    // 2. 是否已经在粘贴
    {
        let locked = state.lock().unwrap();
        let is_pasting = locked.is_pasting.load(Ordering::SeqCst);

        if is_pasting {
            #[cfg(debug_assertions)]
            println!("已经在粘贴中，停止粘贴过程");
            
            locked.is_pasting.store(false, Ordering::SeqCst);
            return Ok(());
        } else {
            locked.is_pasting.store(true, Ordering::SeqCst);
        }
    }

    // 3. 读取剪贴板内容
    let utf16_units = get_clipboard()?;
    
    #[cfg(debug_assertions)]
    println!("剪贴板内容长度：{}", utf16_units.len());

    // 4. 逐字符发送
    let mut i = 0;
    for ch in utf16_units {
        // 每次循环前检查是否中断
        {
            let locked = state.lock().unwrap();
            if !locked.is_pasting.load(Ordering::SeqCst) {
                #[cfg(debug_assertions)]
                println!("粘贴被中断，在第{}个字符处停止", i);
                
                locked.is_pasting.store(false, Ordering::SeqCst);
                return Ok(());
            }
        }

        if ch == 10 {
            // 回车
            let input = [
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VK_RETURN,
                            wScan: 0,
                            dwFlags: KEYBD_EVENT_FLAGS(0),
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                },
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VK_RETURN,
                            wScan: 0,
                            dwFlags: KEYEVENTF_KEYUP,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                },
            ];
            unsafe {
                SendInput(&input, std::mem::size_of::<INPUT>() as i32);
            }
        } else {
            // 普通字符
            let input = [
                // 按下
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VIRTUAL_KEY(0),
                            wScan: ch,
                            dwFlags: KEYEVENTF_UNICODE,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                },
                // 抬起
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VIRTUAL_KEY(0),
                            wScan: ch,
                            dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                },
            ];
            unsafe {
                SendInput(&input, std::mem::size_of::<INPUT>() as i32);
            }
        }

        let random = rand::random::<u32>();
        let delay = stand + random % float;
        sleep(Duration::from_millis(delay as u64)).await;
        i += 1;
    }

    // 5. 粘贴结束，重置状态
    {
        let locked = state.lock().unwrap();
        locked.is_pasting.store(false, Ordering::SeqCst);
    }
    #[cfg(debug_assertions)]
    println!("paste函数成功完成");
    Ok(())
}

/// 切换暂停状态
#[tauri::command]
pub fn toggle_pause(app_handle: tauri::AppHandle) -> bool {
    let state = app_handle.state::<Mutex<PasteState>>();
    let mut locked = state.lock().unwrap();
    locked.is_paused = !locked.is_paused;
    locked.is_paused
}

/// 获取当前快捷键配置
#[tauri::command]
pub fn get_shortcut(app_handle: tauri::AppHandle) -> HotkeyConfig {
    let state = app_handle.state::<Mutex<PasteState>>();
    let locked = state.lock().unwrap();
    locked.shortcut.clone()
}

/// 更新快捷键（并尝试重新注册全局快捷键），并将新配置持久化到本地
#[tauri::command]
pub fn update_shortcut(config: HotkeyConfig, app_handle: tauri::AppHandle) -> Result<String, String> {
    // 1. 验证快捷键是否合法
    if !(config.alt || config.ctrl || config.shift || config.left_ctrl || config.right_ctrl) && !config.intercept_ctrl_v {
        return Err("至少需要选择一个修饰键（Alt/Ctrl/Shift)".to_string());
    }

    let state = app_handle.state::<Mutex<PasteState>>();
    {
        let mut locked = state.lock().unwrap();
        locked.shortcut = config.clone();
    }

    // 2. 保存到配置文件
    if let Err(e) = save_shortcut_config(&app_handle, &config) {
        #[cfg(debug_assertions)]
        eprintln!("保存配置失败: {}", e);
    }

    // 3. 注册全局快捷键
    match crate::register_global_shortcut(app_handle.clone(), &config) {
        Ok(_) => {},
        Err(e) => return Err(format!("{}。可能需要重启应用才能生效。", e)),
    }

    Ok(config.get_description())
}

/// 重启应用
#[tauri::command]
pub fn restart_app(app_handle: tauri::AppHandle) {
    std::thread::spawn(move || {
        let _ = app_handle.restart();
    });
}

/// 保存快捷键配置到本地文件
fn save_shortcut_config(app_handle: &tauri::AppHandle, config: &HotkeyConfig) -> Result<(), String> {
    use tauri::api::path::{BaseDirectory, resolve_path};
    use std::fs::{self, File};
    use std::io::Write;

    let store_path = match resolve_path(
        &app_handle.config(),
        app_handle.package_info(),
        &app_handle.env(),
        "shortcut_config.json",
        Some(BaseDirectory::AppConfig),
    ) {
        Ok(path) => path,
        Err(e) => return Err(format!("获取app_config_dir失败: {}", e)),
    };

    // 确保目录存在
    if let Some(parent) = store_path.parent() {
        if !parent.exists() {
            if let Err(e) = fs::create_dir_all(parent) {
                #[cfg(debug_assertions)]
                eprintln!("创建目录失败: {}", e);
                
                return Err(format!("创建目录失败: {}", e));
            }
        }
    }

    // 序列化配置
    let json = match serde_json::to_string_pretty(config) {
        Ok(j) => j,
        Err(e) => return Err(format!("序列化JSON失败: {}", e)),
    };

    // 写入文件
    let mut file = match File::create(&store_path) {
        Ok(f) => f,
        Err(e) => return Err(format!("创建文件失败: {}", e)),
    };

    if let Err(e) = file.write_all(json.as_bytes()) {
        #[cfg(debug_assertions)]
        eprintln!("写入文件失败: {}", e);
        
        return Err(format!("写入文件失败: {}", e));
    }

    #[cfg(debug_assertions)]
    println!("已将快捷键配置保存到: {}", store_path.display());
    
    Ok(())
}
