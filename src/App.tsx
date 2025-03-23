import {
  Body1Stronger,
  Button,
  Checkbox,
  Dropdown,
  FluentProvider,
  Input,
  InputOnChangeData,
  Label,
  Option,
  Spinner,
  Switch,
  webDarkTheme,
  webLightTheme,
} from '@fluentui/react-components'
import { invoke } from '@tauri-apps/api'
import { listen } from '@tauri-apps/api/event'
import { useEffect, useRef, useState } from 'react'

// 快捷键配置接口
interface HotkeyConfig {
  alt: boolean;
  ctrl: boolean;
  shift: boolean;
  left_ctrl: boolean;
  right_ctrl: boolean;
  key: string;
}

export default function App() {
  const [theme, setTheme] = useState(webLightTheme)
  const [errMsg, setErrMsg] = useState('')
  const [stand, setStand] = useState('10')
  const lastStand = useRef('10')
  const [float, setFloat] = useState('5')
  const lastFloat = useRef('5')
  const [counter, setCounter] = useState(-1)
  const [buttonDisabled, setButtonDisabled] = useState(false)
  const [isPaused, setIsPaused] = useState(false)
  const [shortcutText, setShortcutText] = useState('Alt+Ctrl+V')
  
  // 快捷键设置状态
  const [hotkeyConfig, setHotkeyConfig] = useState<HotkeyConfig>({
    alt: true,
    ctrl: true,
    shift: false,
    left_ctrl: false,
    right_ctrl: false,
    key: 'V'
  })
  const [showHotkeySettings, setShowHotkeySettings] = useState(false)

  const onChange = (
    set: React.Dispatch<React.SetStateAction<string>>,
    _event: React.ChangeEvent<HTMLInputElement>,
    data: InputOnChangeData
  ) => {
    set(data.value)
  }
  
  const onBlur = (
    current: string,
    set: React.Dispatch<React.SetStateAction<string>>,
    last: React.MutableRefObject<string>
  ) => {
    if (/^[1-9]\d{0,5}$/.test(current)) {
      last.current = current
    } else {
      set(last.current)
    }
  }
  
  const onClick = () => {
    triggerPaste()
  }

  const triggerPaste = () => {
    if (isPaused) {
      setErrMsg('功能已暂停')
      return
    }

    setButtonDisabled(true)
    setCounter(3)
    const interval = setInterval(() => {
      setCounter((counter) => {
        if (counter == 1) {
          clearInterval(interval)
          ;(async () => {
            try {
              await invoke('paste', {
                stand: parseInt(lastStand.current),
                float: parseInt(lastFloat.current),
              })
              setErrMsg('')
            } catch (e) {
              setErrMsg(e as string)
            }
            setButtonDisabled(false)
            setCounter(-1)
          })()
          return 0
        }
        return counter - 1
      })
    }, 1000)
  }

  const togglePause = async () => {
    try {
      const newPauseState = await invoke('toggle_pause') as boolean
      setIsPaused(newPauseState)
      setErrMsg(newPauseState ? '功能已暂停' : '')
    } catch (e) {
      console.error(e)
    }
  }
  
  const toggleHotkeySettings = () => {
    setShowHotkeySettings(!showHotkeySettings)
  }
  
  const updateHotkey = async () => {
    try {
      const description = await invoke('update_shortcut', { config: hotkeyConfig }) as string
      setShortcutText(description)
      setShowHotkeySettings(false)
      setErrMsg('')
    } catch (e) {
      setErrMsg(`更新快捷键失败: ${e}`)
    }
  }
  
  const handleHotkeyChange = (field: keyof HotkeyConfig, value: boolean | string) => {
    const updatedConfig = { ...hotkeyConfig, [field]: value }
    
    // 处理互斥选项：默认Ctrl与左右Ctrl互斥
    if (field === 'ctrl' && value === true) {
      updatedConfig.left_ctrl = false
      updatedConfig.right_ctrl = false
    } else if ((field === 'left_ctrl' || field === 'right_ctrl') && value === true) {
      updatedConfig.ctrl = false
      if (field === 'left_ctrl') updatedConfig.right_ctrl = false
      if (field === 'right_ctrl') updatedConfig.left_ctrl = false
    }
    
    setHotkeyConfig(updatedConfig)
  }
  
  // 获取当前快捷键配置
  const fetchHotkeyConfig = async () => {
    try {
      const config = await invoke('get_shortcut') as HotkeyConfig
      setHotkeyConfig(config)
      
      // 更新显示文本
      const description = await invoke('update_shortcut', { config }) as string
      setShortcutText(description)
    } catch (e) {
      console.error('获取快捷键配置失败:', e)
    }
  }

  useEffect(() => {
    const mediaQueryList = window.matchMedia('(prefers-color-scheme: dark)')
    setTheme(mediaQueryList.matches ? webDarkTheme : webLightTheme)
    const listener = (event: MediaQueryListEvent) => {
      setTheme(event.matches ? webDarkTheme : webLightTheme)
    }
    mediaQueryList.addEventListener('change', listener)
    
    // 获取初始快捷键配置
    fetchHotkeyConfig()
    
    // 监听来自全局快捷键的粘贴请求
    const unlisten = listen('trigger-paste', () => {
      triggerPaste()
    })
    
    return () => {
      mediaQueryList.removeEventListener('change', listener)
      unlisten.then(unlistenFn => unlistenFn())
    }
  }, [isPaused])

  const keyOptions = [
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M',
    'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
    'F1', 'F2', 'F3', 'F4', 'F5', 'F6', 'F7', 'F8', 'F9', 'F10', 'F11', 'F12'
  ]

  return (
    <FluentProvider
      style={{
        width: '100%',
        height: '100%',
      }}
      theme={theme}>
      <div
        style={{
          width: '100%',
          height: '100%',
          display: 'flex',
          justifyContent: 'center',
          alignItems: 'center',
        }}>
        <div
          style={{
            display: 'flex',
            padding: 15,
            flexDirection: 'column',
            justifyContent: 'center',
            alignItems: 'center',
          }}>
          {errMsg == '' ? (
            <Body1Stronger>
              单击按钮后, 将在3S后开始, 延迟(ms)范围为[基本延迟,
              基本延迟+浮动值]<br/>
              全局快捷键: {shortcutText}
            </Body1Stronger>
          ) : (
            <Body1Stronger style={{ color: '#d13438' }}>{errMsg}</Body1Stronger>
          )}

          <div
            style={{
              display: 'flex',
              flexDirection: 'column',
              justifyContent: 'space-around',
              alignItems: 'flex-end',
              marginBlock: 20,
            }}>
            <div style={{ marginBottom: 10 }}>
              <Label weight="semibold">基本延迟:</Label>
              <Input
                value={stand}
                size="small"
                style={{ marginLeft: 8, width: 60 }}
                onChange={onChange.bind(null, setStand)}
                onBlur={onBlur.bind(null, stand, setStand, lastStand)}
              />
            </div>
            <div style={{ marginBottom: 10 }}>
              <Label weight="semibold">浮动值:</Label>
              <Input
                value={float}
                size="small"
                style={{ marginLeft: 8, width: 60 }}
                onChange={onChange.bind(null, setFloat)}
                onBlur={onBlur.bind(null, float, setFloat, lastFloat)}
              />
            </div>
            <div style={{ marginBottom: 10, display: 'flex', alignItems: 'center' }}>
              <Label weight="semibold">暂停功能:</Label>
              <Switch 
                checked={isPaused}
                onChange={togglePause}
                style={{ marginLeft: 8 }}
              />
            </div>

            {/* 快捷键设置按钮 */}
            <div style={{ marginBottom: 10, display: 'flex', alignItems: 'center' }}>
              <Button 
                onClick={toggleHotkeySettings}
                size="small"
              >
                {showHotkeySettings ? '隐藏快捷键设置' : '自定义快捷键'}
              </Button>
            </div>
            
            {/* 快捷键设置面板 */}
            {showHotkeySettings && (
              <div style={{ 
                marginTop: 10, 
                marginBottom: 10, 
                display: 'flex', 
                flexDirection: 'column',
                padding: 10,
                border: '1px solid #ccc',
                borderRadius: 4,
                width: '100%'
              }}>
                <div style={{ marginBottom: 10 }}>
                  <Checkbox 
                    label="Alt"
                    checked={hotkeyConfig.alt}
                    onChange={(_, data) => handleHotkeyChange('alt', data.checked)}
                  />
                </div>
                
                <div style={{ marginBottom: 10 }}>
                  <Checkbox 
                    label="Ctrl (通用)"
                    checked={hotkeyConfig.ctrl}
                    onChange={(_, data) => handleHotkeyChange('ctrl', data.checked)}
                  />
                </div>
                
                <div style={{ marginBottom: 10 }}>
                  <Checkbox 
                    label="左Ctrl"
                    checked={hotkeyConfig.left_ctrl}
                    onChange={(_, data) => handleHotkeyChange('left_ctrl', data.checked)}
                  />
                </div>
                
                <div style={{ marginBottom: 10 }}>
                  <Checkbox 
                    label="右Ctrl"
                    checked={hotkeyConfig.right_ctrl}
                    onChange={(_, data) => handleHotkeyChange('right_ctrl', data.checked)}
                  />
                </div>
                
                <div style={{ marginBottom: 10 }}>
                  <Checkbox 
                    label="Shift"
                    checked={hotkeyConfig.shift}
                    onChange={(_, data) => handleHotkeyChange('shift', data.checked)}
                  />
                </div>
                
                <div style={{ marginBottom: 10 }}>
                  <Label>按键:</Label>
                  <Dropdown
                    value={hotkeyConfig.key}
                    onOptionSelect={(_, data) => 
                      data.optionValue && handleHotkeyChange('key', data.optionValue.toString())
                    }
                  >
                    {keyOptions.map(key => (
                      <Option key={key} value={key}>
                        {key}
                      </Option>
                    ))}
                  </Dropdown>
                </div>
                
                <div style={{ display: 'flex', justifyContent: 'flex-end', marginTop: 10 }}>
                  <Button 
                    appearance="primary"
                    onClick={updateHotkey}
                  >
                    保存快捷键
                  </Button>
                </div>
              </div>
            )}
          </div>
          <Button
            appearance="primary"
            disabled={buttonDisabled}
            onClick={onClick}>
            {counter == -1 ? (
              '粘贴'
            ) : counter == 0 ? (
              <Spinner size="tiny" />
            ) : (
              counter
            )}
          </Button>
        </div>
      </div>
    </FluentProvider>
  )
}
