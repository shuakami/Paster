import React, { useEffect, useRef, useState } from 'react';
import './App.css';
import { invoke } from '@tauri-apps/api/tauri';
import { listen } from '@tauri-apps/api/event';
import { 
  Button, 
  Checkbox,
  CssBaseline,
  TextField,
  Typography,
  Box,
  FormControlLabel,
  Stack,
  Switch,
  MenuItem,
  Select,
  FormControl,
  InputLabel,
  ThemeProvider,
  createTheme,
  CircularProgress,
  Paper
} from '@mui/material';

// 快捷键配置接口
interface HotkeyConfig {
  alt: boolean;
  ctrl: boolean;
  shift: boolean;
  left_ctrl: boolean;
  right_ctrl: boolean;
  key: string;
  intercept_ctrl_v: boolean; // 新增，用于是否劫持系统Ctrl+V
}

export default function App() {
  const [darkMode, setDarkMode] = useState(window.matchMedia('(prefers-color-scheme: dark)').matches);
  const theme = createTheme({
    palette: {
      mode: darkMode ? 'dark' : 'light',
    },
  });

  // 错误/提示信息
  const [errMsg, setErrMsg] = useState('');
  // 用户设置的两项延迟参数
  const [stand, setStand] = useState('10');
  const lastStand = useRef('10');
  const [float, setFloat] = useState('5');
  const lastFloat = useRef('5');
  // 倒计时
  const [counter, setCounter] = useState(-1);
  // 粘贴按钮禁用状态
  const [buttonDisabled, setButtonDisabled] = useState(false);
  // 是否暂停粘贴功能
  const [isPaused, setIsPaused] = useState(false);
  // 当前快捷键在界面显示的文本
  const [shortcutText, setShortcutText] = useState('Alt+Ctrl+V');
  
  // 快捷键设置状态
  const [hotkeyConfig, setHotkeyConfig] = useState<HotkeyConfig>({
    alt: true,
    ctrl: true,
    shift: false,
    left_ctrl: false,
    right_ctrl: false,
    key: 'V',
    intercept_ctrl_v: false,
  });

  // 是否显示快捷键信息设置面板
  const [showHotkeySettings, setShowHotkeySettings] = useState(false);

  /**
   * 输入框更新逻辑封装
   */
  const handleInputChange = (
    setter: React.Dispatch<React.SetStateAction<string>>,
    event: React.ChangeEvent<HTMLInputElement>
  ) => {
    setter(event.target.value);
  };

  /**
   * 失焦时校验输入是否合法，不合法则回退
   */
  const handleInputBlur = (
    current: string,
    setter: React.Dispatch<React.SetStateAction<string>>,
    last: React.MutableRefObject<string>
  ) => {
    if (/^[1-9]\d{0,5}$/.test(current)) {
      last.current = current;
    } else {
      setter(last.current);
    }
  };

  /**
   * 点击“粘贴”按钮时
   */
  const handleClick = () => {
    triggerPaste(false);
  };

  /**
   * 根据 fromShortcut 判断是否跳过倒计时
   */
  const triggerPaste = (fromShortcut = false) => {
    console.log(`触发粘贴: fromShortcut=${fromShortcut}`);
    if (isPaused) {
      console.log('功能已暂停，不执行粘贴');
      setErrMsg('功能已暂停');
      return;
    }

    setButtonDisabled(true);
    
    // 如果是全局快捷键触发，跳过倒计时
    if (fromShortcut) {
      executePaste();
      return;
    }
    
    // 否则进行3秒倒计时
    setCounter(3);
    const interval = setInterval(() => {
      setCounter(prev => {
        if (prev === 1) {
          clearInterval(interval);
          executePaste();
          return 0;
        }
        return prev - 1;
      });
    }, 1000);
  };

  /**
   * 执行粘贴
   */
  const executePaste = async () => {
    console.log('开始执行粘贴');
    try {
      await invoke('paste', {
        stand: parseInt(lastStand.current),
        float: parseInt(lastFloat.current),
      });
      setErrMsg('');
    } catch (e) {
      console.error('paste命令执行失败:', e);
      setErrMsg(String(e));
    }

    // 收尾
    setButtonDisabled(false);
    setCounter(-1);
  };

  /**
   * 切换暂停状态
   */
  const togglePause = async () => {
    try {
      const newPauseState = await invoke('toggle_pause') as boolean;
      setIsPaused(newPauseState);
      setErrMsg(newPauseState ? '功能已暂停' : '');
    } catch (e) {
      console.error(e);
    }
  };

  /**
   * 切换显示快捷键设置面板
   */
  const toggleHotkeySettings = () => {
    setShowHotkeySettings(!showHotkeySettings);
  };

  /**
   * 将前端当前 hotkeyConfig 更新给后端，并持久化到本地
   */
  const updateHotkey = async () => {
    try {
      const description = await invoke('update_shortcut', { config: hotkeyConfig }) as string;
      setShortcutText(description);
      setShowHotkeySettings(false);
      setErrMsg('快捷键已更新。若提示需要重启，请重启应用以生效。');
    } catch (e) {
      setErrMsg(`更新快捷键失败: ${e}`);
    }
  };

  /**
   * 重启应用
   */
  const restartApp = async () => {
    try {
      await invoke('restart_app');
    } catch (e) {
      setErrMsg(`重启应用失败: ${e}`);
    }
  };

  /**
   * 修改快捷键配置时
   */
  const handleHotkeyChange = (field: keyof HotkeyConfig, value: boolean | string) => {
    const updatedConfig = { ...hotkeyConfig, [field]: value };

    // 如果勾选了 ctrl，则 left_ctrl 和 right_ctrl 设为 false
    // 如果勾选 left_ctrl，则 ctrl 和 right_ctrl 设为 false
    // 如果勾选 right_ctrl，则 ctrl 和 left_ctrl 设为 false
    if (field === 'ctrl' && value === true) {
      updatedConfig.left_ctrl = false;
      updatedConfig.right_ctrl = false;
    } else if ((field === 'left_ctrl' || field === 'right_ctrl') && value === true) {
      updatedConfig.ctrl = false;
      if (field === 'left_ctrl') updatedConfig.right_ctrl = false;
      if (field === 'right_ctrl') updatedConfig.left_ctrl = false;
    }

    setHotkeyConfig(updatedConfig);
  };

  /**
   * 从后端获取初始的快捷键信息
   */
  const fetchHotkeyConfig = async () => {
    try {
      const config = await invoke('get_shortcut') as HotkeyConfig;
      setHotkeyConfig(config);

      // 获取描述文字
      const description = await invoke('update_shortcut', { config }) as string;
      setShortcutText(description);
    } catch (e) {
      console.error('获取快捷键配置失败:', e);
    }
  };

  /**
   * 组件挂载后
   */
  useEffect(() => {
    // 监测系统深色模式
    const mediaQueryList = window.matchMedia('(prefers-color-scheme: dark)');
    setDarkMode(mediaQueryList.matches);
    const listener = (event: MediaQueryListEvent) => {
      setDarkMode(event.matches);
    };
    mediaQueryList.addEventListener('change', listener);

    // 获取后端已有的快捷键信息
    fetchHotkeyConfig();

    // 监听“trigger-paste”事件，这个事件在后端被全局快捷键触发
    const unlisten = listen('trigger-paste', () => {
      triggerPaste(true);
    });

    return () => {
      mediaQueryList.removeEventListener('change', listener);
      unlisten.then(fn => fn());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isPaused]);

  /**
   * 提供可供选择的键
   */
  const keyOptions = [
    'A','B','C','D','E','F','G','H','I','J','K','L','M',
    'N','O','P','Q','R','S','T','U','V','W','X','Y','Z',
    '0','1','2','3','4','5','6','7','8','9',
    'F1','F2','F3','F4','F5','F6','F7','F8','F9','F10','F11','F12'
  ];

  return (
    <ThemeProvider theme={theme}>
      <CssBaseline />
      <Box sx={{ 
        width: '100%', 
        height: '100%', 
        display: 'flex', 
        flexDirection: 'column',
        padding: 3,
        overflow: 'auto'
      }}>
        <Box sx={{ 
          display: 'flex', 
          flexDirection: 'column', 
          alignItems: 'flex-start', 
          width: '100%' 
        }}>
          {/* 提示或错误信息 */}
          {errMsg === '' ? (
            <Typography variant="body1" sx={{ mb: 2, fontWeight: 'bold' }}>
              单击按钮后, 将在3S后开始, 延迟(ms)范围为[基本延迟, 基本延迟+浮动值]<br/>
              全局快捷键: {shortcutText}
            </Typography>
          ) : (
            <Box sx={{ mb: 2 }}>
              <Typography variant="body1" sx={{ fontWeight: 'bold', color: 'error.main', mb: 1 }}>
                {errMsg}
              </Typography>
              {/* 如果包含“重启”字样，则给个按钮 */}
              {errMsg.includes('重启') && (
                <Button 
                  variant="contained" 
                  color="primary" 
                  size="small"
                  onClick={restartApp}
                  sx={{ mt: 1 }}
                >
                  重启应用
                </Button>
              )}
            </Box>
          )}

          {/* 参数设置区域 */}
          <Box sx={{ 
            display: 'flex', 
            flexDirection: 'column', 
            alignItems: 'flex-start', 
            width: '100%', 
            mb: 3 
          }}>
            <Stack spacing={2} sx={{ width: '100%' }}>
              <Box sx={{ display: 'flex', alignItems: 'center' }}>
                <Typography sx={{ minWidth: 100 }}>基本延迟:</Typography>
                <TextField
                  value={stand}
                  size="small"
                  sx={{ ml: 1, width: 100 }}
                  onChange={(e) => handleInputChange(setStand, e as React.ChangeEvent<HTMLInputElement>)}
                  onBlur={() => handleInputBlur(stand, setStand, lastStand)}
                />
              </Box>
              
              <Box sx={{ display: 'flex', alignItems: 'center' }}>
                <Typography sx={{ minWidth: 100 }}>浮动值:</Typography>
                <TextField
                  value={float}
                  size="small"
                  sx={{ ml: 1, width: 100 }}
                  onChange={(e) => handleInputChange(setFloat, e as React.ChangeEvent<HTMLInputElement>)}
                  onBlur={() => handleInputBlur(float, setFloat, lastFloat)}
                />
              </Box>
              
              <Box sx={{ display: 'flex', alignItems: 'center' }}>
                <Typography sx={{ minWidth: 100 }}>暂停功能:</Typography>
                <Switch
                  checked={isPaused}
                  onChange={togglePause}
                />
              </Box>

              {/* 快捷键设置按钮 */}
              <Box>
                <Button 
                  variant="outlined"
                  onClick={toggleHotkeySettings}
                  size="small"
                >
                  {showHotkeySettings ? '隐藏快捷键设置' : '自定义快捷键'}
                </Button>
              </Box>
              
              {/* 快捷键设置面板 */}
              {showHotkeySettings && (
                <Paper 
                  elevation={2} 
                  sx={{ 
                    p: 2, 
                    width: '100%', 
                    mt: 1, 
                    mb: 1 
                  }}
                >
                  <Stack spacing={2}>
                    <FormControlLabel
                      control={
                        <Checkbox 
                          checked={hotkeyConfig.intercept_ctrl_v}
                          onChange={(e) => handleHotkeyChange('intercept_ctrl_v', e.target.checked)}
                        />
                      }
                      label="劫持系统Ctrl+V"
                    />

                    {/* 如果勾选了劫持系统Ctrl+V，则无需再显示其他组合键设置。可自行按需求决定显示或隐藏 */}
                    {!hotkeyConfig.intercept_ctrl_v && (
                      <>
                        <FormControlLabel
                          control={
                            <Checkbox 
                              checked={hotkeyConfig.alt}
                              onChange={(e) => handleHotkeyChange('alt', e.target.checked)}
                            />
                          }
                          label="Alt"
                        />
                        <FormControlLabel
                          control={
                            <Checkbox 
                              checked={hotkeyConfig.ctrl}
                              onChange={(e) => handleHotkeyChange('ctrl', e.target.checked)}
                            />
                          }
                          label="Ctrl"
                        />
                        <FormControlLabel
                          control={
                            <Checkbox 
                              checked={hotkeyConfig.left_ctrl}
                              onChange={(e) => handleHotkeyChange('left_ctrl', e.target.checked)}
                            />
                          }
                          label="左Ctrl"
                        />
                        <FormControlLabel
                          control={
                            <Checkbox 
                              checked={hotkeyConfig.right_ctrl}
                              onChange={(e) => handleHotkeyChange('right_ctrl', e.target.checked)}
                            />
                          }
                          label="右Ctrl"
                        />
                        <FormControlLabel
                          control={
                            <Checkbox 
                              checked={hotkeyConfig.shift}
                              onChange={(e) => handleHotkeyChange('shift', e.target.checked)}
                            />
                          }
                          label="Shift"
                        />

                        <FormControl fullWidth>
                          <InputLabel id="key-select-label">键</InputLabel>
                          <Select
                            labelId="key-select-label"
                            value={hotkeyConfig.key}
                            label="键"
                            onChange={(e) => handleHotkeyChange('key', e.target.value)}
                          >
                            {keyOptions.map(key => (
                              <MenuItem key={key} value={key}>
                                {key}
                              </MenuItem>
                            ))}
                          </Select>
                        </FormControl>
                      </>
                    )}

                    <Box sx={{ mt: 1 }}>
                      <Button 
                        variant="contained"
                        onClick={updateHotkey}
                      >
                        保存快捷键
                      </Button>
                    </Box>
                  </Stack>
                </Paper>
              )}
            </Stack>
          </Box>
          
          {/* 触发粘贴按钮 */}
          <Button
            variant="contained"
            disabled={buttonDisabled}
            onClick={handleClick}
          >
            {counter === -1 ? (
              '粘贴'
            ) : counter === 0 ? (
              <CircularProgress size={24} color="inherit" />
            ) : (
              counter
            )}
          </Button>
        </Box>
      </Box>
    </ThemeProvider>
  );
}
