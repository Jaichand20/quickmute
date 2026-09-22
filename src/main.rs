#![windows_subsystem = "windows"]

mod audio;
mod feedback;
mod tray;

use audio::AudioManager;
use feedback::play_feedback;
use tray::{TrayIcon, ID_TRAY_EXIT, ID_TRAY_TOGGLE, WM_TRAYICON};

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};
use std::sync::OnceLock;
use std::time::Instant;
use windows::core::w;
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    keybd_event, RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, KEYBD_EVENT_FLAGS,
    KEYEVENTF_KEYUP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW,
    GetWindowLongPtrW, PostMessageW, PostQuitMessage, RegisterClassW, RegisterWindowMessageW,
    SetWindowLongPtrW, SetWindowsHookExW, UnhookWindowsHookEx, GWLP_USERDATA, KBDLLHOOKSTRUCT,
    MSG, WINDOW_EX_STYLE, WH_KEYBOARD_LL, WM_APP, WM_COMMAND, WM_DESTROY, WM_HOTKEY,
    WM_KEYDOWN, WM_LBUTTONUP, WM_RBUTTONUP, WM_SYSKEYDOWN, WNDCLASSW, WS_OVERLAPPED,
};

const WM_APP_TRIGGER_MUTE: u32 = WM_APP + 2;
static GLOBAL_HWND: AtomicIsize = AtomicIsize::new(0);
static WM_TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);
static CONFIGURED_KEYS: OnceLock<Vec<u32>> = OnceLock::new();

unsafe extern "system" fn low_level_keyboard_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 && (wparam.0 as u32 == WM_KEYDOWN || wparam.0 as u32 == WM_SYSKEYDOWN) {
        let kbd = *(lparam.0 as *const KBDLLHOOKSTRUCT);
        log_debug(&format!("low_level_keyboard_proc: vk=0x{:X}, scan=0x{:X}", kbd.vkCode, kbd.scanCode));
        let is_target = CONFIGURED_KEYS
            .get()
            .map_or(kbd.vkCode == 0x7C, |keys| keys.contains(&kbd.vkCode));

        if is_target {
            log_debug(&format!("low_level_keyboard_proc: MATCHED hotkey vk=0x{:X}", kbd.vkCode));
            // Send unassigned mask key (0xE8) to tell Windows that a non-modifier key was pressed.
            // This prevents Windows from treating the Win+Ctrl+Alt+Shift release as the Office key (m365.cloud.microsoft).
            keybd_event(0xE8, 0, KEYBD_EVENT_FLAGS(0), 0);
            keybd_event(0xE8, 0, KEYEVENTF_KEYUP, 0);

            let hwnd_val = GLOBAL_HWND.load(Ordering::Relaxed);
            if hwnd_val != 0 {
                let _ = PostMessageW(
                    HWND(hwnd_val as *mut _),
                    WM_APP_TRIGGER_MUTE,
                    WPARAM(0),
                    LPARAM(0),
                );
                return LRESULT(1);
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

struct AppState {
    audio: AudioManager,
    tray: TrayIcon,
    last_trigger: Instant,
}

fn main() -> windows::core::Result<()> {
    std::panic::set_hook(Box::new(|info| {
        log_debug(&format!("PANIC OCCURRED: {:?}", info));
    }));

    unsafe {
        // Enforce single instance via named mutex in user session (Local\)
        let mutex_handle = CreateMutexW(
            None,
            true,
            w!("Local\\QuickMute_SingleInstance_Mutex_Jaichand20"),
        )?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            log_debug("Exiting: another instance is already running.");
            let _ = CloseHandle(mutex_handle);
            return Ok(());
        }

        // Attach to user's interactive desktop so system tray icon and UI are visible
        use windows::Win32::System::StationsAndDesktops::{
            OpenDesktopW, OpenWindowStationW, SetProcessWindowStation, SetThreadDesktop,
            DESKTOP_CONTROL_FLAGS,
        };
        if let Ok(winsta) = OpenWindowStationW(w!("WinSta0"), false, 0x02000000 | 0x0000037F) {
            let _ = SetProcessWindowStation(winsta);
            if let Ok(desk) = OpenDesktopW(w!("Default"), DESKTOP_CONTROL_FLAGS(0), false, 0x02000000 | 0x000001FF) {
                let ok_desk = SetThreadDesktop(desk);
                log_debug(&format!("Attached to WinSta0\\Default: {:?}", ok_desk.is_ok()));
            }
        }

        let instance: HINSTANCE = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
            .map(|h| HINSTANCE(h.0))
            .unwrap_or_default();
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

        let state = Rc::new(RefCell::new(AppState {
            audio,
            tray,
            last_trigger: Instant::now() - std::time::Duration::from_secs(10),
        }));
        let state_ptr = Rc::into_raw(state.clone()) as isize;
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr);

        GLOBAL_HWND.store(hwnd.0 as isize, Ordering::Relaxed);

        // Register configured hotkeys (defaults to F13 = 0x7C)
        let hotkeys = get_configured_keys();
        CONFIGURED_KEYS.set(hotkeys.clone()).ok();
        for (idx, &vk) in hotkeys.iter().enumerate() {
            let id = (idx + 1) as i32;
            let res = RegisterHotKey(hwnd, id, HOT_KEY_MODIFIERS(0), vk);
            log_debug(&format!("RegisterHotKey id={}, vk=0x{:X}: {:?}", id, vk, res));
        }

        // Install low-level keyboard hook (WH_KEYBOARD_LL) for reliable hardware macro capture
        let hook = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(low_level_keyboard_proc),
            instance,
            0,
        );
        log_debug(&format!("SetWindowsHookExW WH_KEYBOARD_LL: {:?}", hook.is_ok()));

        let mut msg = MSG::default();
        log_debug("Entering message loop");
        loop {
            log_debug("Waiting for GetMessageW...");
            let res = GetMessageW(&mut msg, HWND::default(), 0, 0);
            log_debug(&format!("GetMessageW returned {}, msg=0x{:X}", res.0, msg.message));
            if !res.as_bool() {
                break;
            }
            DispatchMessageW(&msg);
        }
        log_debug(&format!("Exited message loop, msg=0x{:X}", msg.message));

        if let Ok(h) = hook {
            let _ = UnhookWindowsHookEx(h);
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

fn log_debug(s: &str) {
    use std::io::Write;
    let log_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("quickmute.log")))
        .unwrap_or_else(|| std::path::PathBuf::from("quickmute.log"));
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&log_path) {
        let _ = writeln!(f, "{}", s);
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    log_debug(&format!("wnd_proc: msg=0x{:X}, wparam={}, lparam=0x{:X}", msg, wparam.0, lparam.0));

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
        WM_HOTKEY | WM_APP_TRIGGER_MUTE => {
            log_debug(&format!("Mute trigger received! msg=0x{:X}, wparam={}, lparam=0x{:X}", msg, wparam.0, lparam.0));
            match state.try_borrow_mut() {
                Ok(mut app) => {
                    if app.last_trigger.elapsed().as_millis() < 250 {
                        log_debug("Ignored trigger: debounce active (< 250ms)");
                        return LRESULT(0);
                    }
                    app.last_trigger = Instant::now();

                    match app.audio.toggle_mute() {
                        Ok(new_mute) => {
                            log_debug(&format!("Toggled mute successfully! new_mute={}", new_mute));
                            app.tray.update(new_mute);
                            play_feedback(new_mute);
                        }
                        Err(e) => {
                            log_debug(&format!("toggle_mute failed: {:?}", e));
                        }
                    }
                }
                Err(e) => {
                    log_debug(&format!("state.try_borrow_mut failed: {:?}", e));
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
