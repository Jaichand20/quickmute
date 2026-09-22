#![windows_subsystem = "windows"]

mod audio;
mod feedback;
mod tray;

use audio::AudioManager;
use feedback::play_feedback;
use tray::{TrayIcon, ID_TRAY_EXIT, ID_TRAY_TOGGLE, WM_TRAYICON};

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use windows::core::w;
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetWindowLongPtrW,
    PostQuitMessage, RegisterClassW, RegisterWindowMessageW, SetWindowLongPtrW, GWLP_USERDATA,
    MSG, WINDOW_EX_STYLE, WM_COMMAND, WM_DESTROY, WM_HOTKEY, WM_LBUTTONUP, WM_RBUTTONUP,
    WNDCLASSW, WS_OVERLAPPED,
};

static WM_TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

struct AppState {
    audio: AudioManager,
    tray: TrayIcon,
}

fn main() -> windows::core::Result<()> {
    unsafe {
        // Enforce single instance via named mutex in user session (Local\)
        let mutex_handle = CreateMutexW(
            None,
            true,
            w!("Local\\QuickMute_SingleInstance_Mutex_Jaichand20"),
        )?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(mutex_handle);
            return Ok(());
        }

        let instance = HINSTANCE::default();
        let class_name = w!("QuickMuteWindowClass");

        let wnd_class = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: instance,
            lpszClassName: class_name,
            ..Default::default()
        };
        let _ = RegisterClassW(&wnd_class);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            w!("QuickMuteHiddenWindow"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            HWND::default(),
            None,
            instance,
            None,
        )?;

        // Register for TaskbarCreated message in case Windows Explorer restarts
        let taskbar_msg = RegisterWindowMessageW(w!("TaskbarCreated"));
        WM_TASKBAR_CREATED.store(taskbar_msg, Ordering::Relaxed);

        let mut audio = AudioManager::new()?;
        let initial_muted = audio.is_muted().unwrap_or(false);

        let mut tray = TrayIcon::new(hwnd);
        tray.init(initial_muted);

        let state = Rc::new(RefCell::new(AppState { audio, tray }));
        let state_ptr = Rc::into_raw(state.clone()) as isize;
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr);

        // Register configured hotkeys (defaults to F13 = 0x7C)
        let hotkeys = get_configured_keys();
        for (idx, &vk) in hotkeys.iter().enumerate() {
            let id = (idx + 1) as i32;
            let _ = RegisterHotKey(hwnd, id, HOT_KEY_MODIFIERS(0), vk);
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
            DispatchMessageW(&msg);
        }

        for (idx, _) in hotkeys.iter().enumerate() {
            let id = (idx + 1) as i32;
            let _ = UnregisterHotKey(hwnd, id);
        }
        let _ = Rc::from_raw(state_ptr as *const RefCell<AppState>);
        let _ = CloseHandle(mutex_handle);
    }

    Ok(())
}

fn get_configured_keys() -> Vec<u32> {
    let config_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("config.ini")))
        .unwrap_or_else(|| std::path::PathBuf::from("config.ini"));

    let mut keys = Vec::new();

    if let Ok(content) = std::fs::read_to_string(&config_path) {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.starts_with(';') || line.is_empty() {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                if k.trim().eq_ignore_ascii_case("hotkey") {
                    for part in v.split(',') {
                        match part.trim().to_ascii_uppercase().as_str() {
                            "F13" => keys.push(0x7C),
                            "HOME" => keys.push(0x24),
                            "F14" => keys.push(0x7D),
                            "F15" => keys.push(0x7E),
                            "F16" => keys.push(0x7F),
                            "F17" => keys.push(0x80),
                            "F18" => keys.push(0x81),
                            "F19" => keys.push(0x82),
                            "F20" => keys.push(0x83),
                            "F21" => keys.push(0x84),
                            "F22" => keys.push(0x85),
                            "F23" => keys.push(0x86),
                            "F24" => keys.push(0x87),
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    if keys.is_empty() {
        // Default to F13 (the dedicated macro button)
        keys.push(0x7C);
        let default_config = "\
# QuickMute Configuration
# Configure your physical mute hotkey below.
# Supported keys: F13, HOME, F14, F15, F16, F17, F18, F19, F20, F21, F22, F23, F24
# You can also specify multiple keys separated by commas (e.g. hotkey = F13, HOME)
hotkey = F13
";
        let _ = std::fs::write(&config_path, default_config);
    }

    keys
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RefCell<AppState>;
    if state_ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }

    let state = &*state_ptr;

    let taskbar_msg = WM_TASKBAR_CREATED.load(Ordering::Relaxed);
    if taskbar_msg != 0 && msg == taskbar_msg {
        if let Ok(mut app) = state.try_borrow_mut() {
            let is_muted = app.audio.is_muted().unwrap_or(false);
            app.tray.re_add(is_muted);
        }
        return LRESULT(0);
    }

    match msg {
        WM_HOTKEY => {
            if let Ok(mut app) = state.try_borrow_mut() {
                if let Ok(new_mute) = app.audio.toggle_mute() {
                    app.tray.update(new_mute);
                    play_feedback(new_mute);
                }
            }
            LRESULT(0)
        }
        WM_TRAYICON => {
            let event = (lparam.0 & 0xFFFF) as u32;
            if event == WM_RBUTTONUP {
                if let Ok(mut app) = state.try_borrow_mut() {
                    let is_muted = app.audio.is_muted().unwrap_or(false);
                    app.tray.show_context_menu(is_muted);
                }
            } else if event == WM_LBUTTONUP {
                if let Ok(mut app) = state.try_borrow_mut() {
                    if let Ok(new_mute) = app.audio.toggle_mute() {
                        app.tray.update(new_mute);
                        play_feedback(new_mute);
                    }
                }
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let cmd_id = (wparam.0 & 0xFFFF) as usize;
            if cmd_id == ID_TRAY_TOGGLE {
                if let Ok(mut app) = state.try_borrow_mut() {
                    if let Ok(new_mute) = app.audio.toggle_mute() {
                        app.tray.update(new_mute);
                        play_feedback(new_mute);
                    }
                }
            } else if cmd_id == ID_TRAY_EXIT {
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
