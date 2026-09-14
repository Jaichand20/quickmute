use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, POINT};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleBitmap, CreateCompatibleDC, CreateSolidBrush, DeleteDC, DeleteObject,
    FillRect, GetDC, ReleaseDC, SelectObject, HBITMAP, HGDIOBJ,
};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, CreatePopupMenu, DestroyIcon, DestroyMenu, GetCursorPos,
    InsertMenuW, SetForegroundWindow, TrackPopupMenu, ICONINFO, MF_BYPOSITION, MF_SEPARATOR,
    MF_STRING, TPM_BOTTOMALIGN, TPM_LEFTALIGN, HICON,
};

pub const WM_TRAYICON: u32 = 0x8000 + 1; // WM_APP + 1
pub const ID_TRAY_TOGGLE: usize = 1001;
pub const ID_TRAY_EXIT: usize = 1002;

pub struct TrayIcon {
    hwnd: HWND,
    current_icon: HICON,
}

impl TrayIcon {
    pub fn new(hwnd: HWND) -> Self {
        Self {
            hwnd,
            current_icon: HICON::default(),
        }
    }

    pub fn init(&mut self, is_muted: bool) {
        self.update(is_muted);
    }

    pub fn update(&mut self, is_muted: bool) {
        unsafe {
            if !self.current_icon.is_invalid() {
                let _ = DestroyIcon(self.current_icon);
            }

            // Green = Unmuted (RGB 0x22, 0xC5, 0x5E), Red = Muted (RGB 0xEF, 0x44, 0x44)
            let color = if is_muted {
                COLORREF(0x004444EF) // BGR for Windows GDI: Red
            } else {
                COLORREF(0x005EC522) // BGR for Windows GDI: Green
            };

            self.current_icon = create_circle_icon(color);

            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: 1,
                uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
                uCallbackMessage: WM_TRAYICON,
                hIcon: self.current_icon,
                ..Default::default()
            };

            let tip = if is_muted {
                "QuickMute: MUTED [Home to unmute]"
            } else {
                "QuickMute: LIVE [Home to mute]"
            };

            let wide: Vec<u16> = tip.encode_utf16().chain(std::iter::once(0)).collect();
            let copy_len = wide.len().min(nid.szTip.len() - 1);
            nid.szTip[..copy_len].copy_from_slice(&wide[..copy_len]);

            // Try modify first; if not present, add it
            if !Shell_NotifyIconW(NIM_MODIFY, &nid).as_bool() {
                let _ = Shell_NotifyIconW(NIM_ADD, &nid);
            }
        }
    }

    pub fn show_context_menu(&self, is_muted: bool) {
        unsafe {
            let menu = match CreatePopupMenu() {
                Ok(m) => m,
                Err(_) => return,
            };

            let toggle_text = if is_muted {
                w!("Unmute Microphone")
            } else {
                w!("Mute Microphone")
            };

            let _ = InsertMenuW(menu, 0, MF_BYPOSITION | MF_STRING, ID_TRAY_TOGGLE, toggle_text);
            let _ = InsertMenuW(menu, 1, MF_BYPOSITION | MF_SEPARATOR, 0, PCWSTR::null());
            let _ = InsertMenuW(menu, 2, MF_BYPOSITION | MF_STRING, ID_TRAY_EXIT, w!("Exit QuickMute"));

            let mut cursor = POINT::default();
            let _ = GetCursorPos(&mut cursor);

            let _ = SetForegroundWindow(self.hwnd);
            let _ = TrackPopupMenu(
                menu,
                TPM_BOTTOMALIGN | TPM_LEFTALIGN,
                cursor.x,
                cursor.y,
                0,
                self.hwnd,
                None,
            );
            let _ = DestroyMenu(menu);
        }
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        unsafe {
            let nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: 1,
                ..Default::default()
            };
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
            if !self.current_icon.is_invalid() {
                let _ = DestroyIcon(self.current_icon);
            }
        }
    }
}

unsafe fn create_circle_icon(color: COLORREF) -> HICON {
    let size = 16;
    let hdc = GetDC(HWND::default());
    let mem_dc = CreateCompatibleDC(hdc);

    let hbm_color: HBITMAP = CreateCompatibleBitmap(hdc, size, size);
    let hbm_mask: HBITMAP = CreateCompatibleBitmap(hdc, size, size);

    let old_bmp: HGDIOBJ = SelectObject(mem_dc, hbm_color);
    let brush = CreateSolidBrush(color);

    let rect = windows::Win32::Foundation::RECT {
        left: 0,
        top: 0,
        right: size,
        bottom: size,
    };
    FillRect(mem_dc, &rect, brush);

    let _ = SelectObject(mem_dc, old_bmp);
    let _ = DeleteObject(brush);
    let _ = DeleteDC(mem_dc);
    let _ = ReleaseDC(HWND::default(), hdc);

    let icon_info = ICONINFO {
        fIcon: windows::Win32::Foundation::BOOL::from(true),
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: hbm_mask,
        hbmColor: hbm_color,
    };

    let icon = CreateIconIndirect(&icon_info).unwrap_or_default();
    let _ = DeleteObject(hbm_color);
    let _ = DeleteObject(hbm_mask);

    icon
}
