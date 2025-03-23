use std::ffi::c_void;
use std::sync::Mutex;
use serde::{Deserialize, Serialize};
use tauri::Manager;
use tokio::time::{sleep, Duration};
use windows::Win32::{
    Foundation::HGLOBAL,
    System::{
        DataExchange::CloseClipboard,
        Memory::{GlobalLock, GlobalUnlock},
    },
    UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
        KEYEVENTF_UNICODE, VIRTUAL_KEY, VK_RETURN,
    },
};
use windows::Win32::{
    Foundation::HWND,
    System::DataExchange::{GetClipboardData, OpenClipboard},
};

// 用于控制程序是否暂停
pub struct PasteState {
    pub is_paused: bool,
    pub shortcut: HotkeyConfig,
}

impl PasteState {
    pub fn new() -> Self {
        Self { 
            is_paused: false,
            shortcut: HotkeyConfig::default(),
        }
    }
}

// 快捷键配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub left_ctrl: bool,
    pub right_ctrl: bool,
    pub key: String,
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
        }
    }
}

impl HotkeyConfig {
    // 转换为Tauri快捷键格式
    pub fn to_tauri_accelerator(&self) -> String {
        let mut parts = Vec::new();
        
        if self.alt {
            parts.push("Alt".to_string());
        }
        
        if self.ctrl {
            parts.push("Ctrl".to_string());
        } else if self.left_ctrl {
            parts.push("CtrL".to_string());
        } else if self.right_ctrl {
            parts.push("CtrR".to_string());
        }
        
        if self.shift {
            parts.push("Shift".to_string());
        }
        
        parts.push(self.key.clone());
        
        parts.join("+")
    }
    
    // 获取快捷键描述
    pub fn get_description(&self) -> String {
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

fn get_clipboard() -> Result<Vec<u16>, &'static str> {
    const CF_UNICODETEXT: u32 = 13;
    let mut result: Vec<u16> = vec![];

    //参考 https://learn.microsoft.com/zh-cn/windows/win32/dataxchg/using-the-clipboard#pasting-information-from-the-clipboard
    unsafe {
        OpenClipboard(HWND(0)).or(Err("打开剪切板错误"))?;
        let hglb = GetClipboardData(CF_UNICODETEXT).map_err(|_| {
            if let Err(_) = CloseClipboard() {
                return "关闭剪切板失败";
            }
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
            if item == 13 {
                //舍弃'\r'
                continue;
            }
            result.push(item);
        }

        GlobalUnlock(locker).map_err(|_| {
            if let Err(_) = CloseClipboard() {
                return "关闭剪切板失败";
            }
            "解除剪切板锁定失败"
        })?;
        CloseClipboard().or(Err("关闭剪切板失败"))?;
    }
    return Ok(result);
}

#[tauri::command]
pub async fn paste(stand: u32, float: u32, app_handle: tauri::AppHandle) -> Result<(), &'static str> {
    // 检查是否暂停状态，在await之前检查完成并释放锁
    let is_paused = {
        let state = app_handle.state::<Mutex<PasteState>>();
        let state = state.lock().unwrap();
        state.is_paused
    };
    
    if is_paused {
        return Err("功能已暂停");
    }
    
    let utf16_units: Vec<u16> = get_clipboard()?;
    for item in utf16_units {
        if item == 10 {
            //必须特别处理回车
            let input = [
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        //参考 https://learn.microsoft.com/zh-cn/windows/win32/api/winuser/ns-winuser-keybdinput
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
            let input = [INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: item,
                        dwFlags: KEYEVENTF_UNICODE,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            }];
            unsafe {
                SendInput(&input, std::mem::size_of::<INPUT>() as i32);
            }
        };

        let random = rand::random::<u32>();
        sleep(Duration::from_millis((stand + random % float) as u64)).await;
    }

    return Ok(());
}

#[tauri::command]
pub fn toggle_pause(app_handle: tauri::AppHandle) -> bool {
    let state = app_handle.state::<Mutex<PasteState>>();
    let mut state = state.lock().unwrap();
    state.is_paused = !state.is_paused;
    state.is_paused
}

#[tauri::command]
pub fn get_shortcut(app_handle: tauri::AppHandle) -> HotkeyConfig {
    let state = app_handle.state::<Mutex<PasteState>>();
    let state = state.lock().unwrap();
    state.shortcut.clone()
}

#[tauri::command]
pub fn update_shortcut(config: HotkeyConfig, app_handle: tauri::AppHandle) -> Result<String, String> {
    let state = app_handle.state::<Mutex<PasteState>>();
    let mut state = state.lock().unwrap();
    
    // 保存新配置
    state.shortcut = config.clone();
    
    // 生成描述
    let description = config.get_description();
    
    // 通知主线程更新快捷键
    app_handle.emit_all("update-hotkey", config).map_err(|e| e.to_string())?;
    
    Ok(description)
}
