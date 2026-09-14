#![windows_subsystem = "windows"]

mod audio;
mod feedback;
mod tray;

use audio::AudioManager;
use feedback::play_feedback;
use tray::{TrayIcon, ID_TRAY_EXIT, ID_TRAY_TOGGLE, WM_TRAYICON};

use std::cell::RefCell;
use std::rc::Rc;
use windows::core::w;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, VK_HOME};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, PostQuitMessage,
    RegisterClassW, SetWindowLongPtrW, GetWindowLongPtrW, GWLP_USERDATA, MSG, WINDOW_EX_STYLE,
    WM_COMMAND, WM_HOTKEY, WM_RBUTTONUP, WNDCLASSW, WS_OVERLAPPED,
};

const HOTKEY_ID: i32 = 1;

struct AppState {
    audio: AudioManager,
    tray: TrayIcon,
}

fn main() -> windows::core::Result<()> {
    unsafe {
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

        let audio = AudioManager::new()?;
        let initial_muted = audio.is_muted().unwrap_or(false);

        let mut tray = TrayIcon::new(hwnd);
        tray.init(initial_muted);

        let state = Rc::new(RefCell::new(AppState { audio, tray }));
        let state_ptr = Rc::into_raw(state.clone()) as isize;
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr);

        // Register global Home key hotkey (VK_HOME = 0x24)
        let _ = RegisterHotKey(hwnd, HOTKEY_ID, HOT_KEY_MODIFIERS(0), VK_HOME.0 as u32);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
            DispatchMessageW(&msg);
        }

        let _ = UnregisterHotKey(hwnd, HOTKEY_ID);
        // Reconstitute Rc to drop properly
        let _ = Rc::from_raw(state_ptr as *const RefCell<AppState>);
    }

    Ok(())
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

    match msg {
        WM_HOTKEY => {
            if wparam.0 as i32 == HOTKEY_ID {
                if let Ok(mut app) = state.try_borrow_mut() {
                    if let Ok(new_mute) = app.audio.toggle_mute() {
                        app.tray.update(new_mute);
                        play_feedback(new_mute);
                    }
                }
            }
            LRESULT(0)
        }
        WM_TRAYICON => {
            let event = (lparam.0 & 0xFFFF) as u32;
            if event == WM_RBUTTONUP {
                if let Ok(app) = state.try_borrow() {
                    let is_muted = app.audio.is_muted().unwrap_or(false);
                    app.tray.show_context_menu(is_muted);
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
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
