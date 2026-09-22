use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateBitmap, CreateCompatibleBitmap, CreateCompatibleDC, CreatePen, CreateSolidBrush,
    DeleteDC, DeleteObject, Ellipse, FillRect, GetDC, ReleaseDC, SelectObject, HBITMAP, HGDIOBJ,
    PEN_STYLE, PS_SOLID,
};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, CreatePopupMenu, DestroyIcon, DestroyMenu, GetCursorPos, InsertMenuW,
    PostMessageW, SetForegroundWindow, TrackPopupMenu, ICONINFO, MF_BYPOSITION, MF_SEPARATOR,
    MF_STRING, TPM_BOTTOMALIGN, TPM_LEFTALIGN, HICON, WM_NULL,
};

pub const WM_TRAYICON: u32 = 0x8000 + 1; // WM_APP + 1
pub const ID_TRAY_TOGGLE: usize = 1001;
pub const ID_TRAY_EXIT: usize = 1002;

pub struct TrayIcon {
    hwnd: HWND,
    icon_muted: HICON,
    icon_unmuted: HICON,
    added: bool,
}

impl TrayIcon {
    pub fn new(hwnd: HWND) -> Self {
        unsafe {
            // Green = Unmuted (RGB 0x22, 0xC5, 0x5E -> BGR 0x005EC522)
            // Red = Muted (RGB 0xEF, 0x44, 0x44 -> BGR 0x004444EF)
            let icon_unmuted = create_circle_icon(COLORREF(0x005EC522));
            let icon_muted = create_circle_icon(COLORREF(0x004444EF));

            Self {
                hwnd,
                icon_muted,
                icon_unmuted,
                added: false,
            }
        }
    }

    pub fn init(&mut self, is_muted: bool) {
        self.update(is_muted);
    }

    pub fn update(&mut self, is_muted: bool) {
        unsafe {
            let nid = self.build_nid(is_muted);
            if self.added {
                if !Shell_NotifyIconW(NIM_MODIFY, &nid).as_bool() {
                    let _ = Shell_NotifyIconW(NIM_ADD, &nid);
                }
            } else {
                if Shell_NotifyIconW(NIM_ADD, &nid).as_bool() {
                    self.added = true;
                }
            }
        }
    }

    pub fn re_add(&mut self, is_muted: bool) {
        unsafe {
            let nid = self.build_nid(is_muted);
            let _ = Shell_NotifyIconW(NIM_ADD, &nid);
            self.added = true;
        }
    }

    unsafe fn build_nid(&self, is_muted: bool) -> NOTIFYICONDATAW {
        let mut nid = std::mem::zeroed::<NOTIFYICONDATAW>();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = self.hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        nid.uCallbackMessage = WM_TRAYICON;
        nid.hIcon = if is_muted {
            self.icon_muted
        } else {
            self.icon_unmuted
        };

        let tip = if is_muted {
            "QuickMute: MUTED [Button to unmute]"
        } else {
            "QuickMute: LIVE [Button to mute]"
        };

        let wide: Vec<u16> = tip.encode_utf16().chain(std::iter::once(0)).collect();
        let copy_len = wide.len().min(nid.szTip.len() - 1);
        nid.szTip[..copy_len].copy_from_slice(&wide[..copy_len]);

        nid
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
            // Post WM_NULL to dismiss menu when clicking away
            let _ = PostMessageW(self.hwnd, WM_NULL, WPARAM(0), LPARAM(0));
            let _ = DestroyMenu(menu);
        }
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        unsafe {
            let mut nid = std::mem::zeroed::<NOTIFYICONDATAW>();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = self.hwnd;
            nid.uID = 1;
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);

            if !self.icon_muted.is_invalid() {
                let _ = DestroyIcon(self.icon_muted);
            }
            if !self.icon_unmuted.is_invalid() {
                let _ = DestroyIcon(self.icon_unmuted);
            }
        }
    }
}

unsafe fn create_circle_icon(color: COLORREF) -> HICON {
    let size = 16;
    let hdc = GetDC(HWND::default());
    let mem_dc = CreateCompatibleDC(hdc);
    let mask_dc = CreateCompatibleDC(hdc);

    let hbm_color: HBITMAP = CreateCompatibleBitmap(hdc, size, size);
    let hbm_mask: HBITMAP = CreateBitmap(size, size, 1, 1, None);

    let old_color_bmp: HGDIOBJ = SelectObject(mem_dc, hbm_color);
    let old_mask_bmp: HGDIOBJ = SelectObject(mask_dc, hbm_mask);

    // 1. Prepare mask: 1 is transparent, 0 is opaque.
    // Fill mask with white (1 = transparent)
    let white_brush = CreateSolidBrush(COLORREF(0x00FFFFFF));
    let rect = RECT {
        left: 0,
        top: 0,
        right: size,
        bottom: size,
    };
    FillRect(mask_dc, &rect, white_brush);
    let _ = DeleteObject(white_brush);

    // Draw opaque circle on mask with black (0 = opaque)
    let black_brush = CreateSolidBrush(COLORREF(0x00000000));
    let black_pen = CreatePen(PEN_STYLE(PS_SOLID.0), 1, COLORREF(0x00000000));
    let old_mask_brush = SelectObject(mask_dc, black_brush);
    let old_mask_pen = SelectObject(mask_dc, black_pen);
    let _ = Ellipse(mask_dc, 1, 1, 15, 15);
    let _ = SelectObject(mask_dc, old_mask_brush);
    let _ = SelectObject(mask_dc, old_mask_pen);
    let _ = DeleteObject(black_brush);
    let _ = DeleteObject(black_pen);

    // 2. Prepare color bitmap: background black, circle colored
    let black_bg = CreateSolidBrush(COLORREF(0x00000000));
    FillRect(mem_dc, &rect, black_bg);
    let _ = DeleteObject(black_bg);

    let color_brush = CreateSolidBrush(color);
    let color_pen = CreatePen(PEN_STYLE(PS_SOLID.0), 1, color);
    let old_color_brush = SelectObject(mem_dc, color_brush);
    let old_color_pen = SelectObject(mem_dc, color_pen);
    let _ = Ellipse(mem_dc, 1, 1, 15, 15);
    let _ = SelectObject(mem_dc, old_color_brush);
    let _ = SelectObject(mem_dc, old_color_pen);
    let _ = DeleteObject(color_brush);
    let _ = DeleteObject(color_pen);

    // Clean up DCs
    let _ = SelectObject(mem_dc, old_color_bmp);
    let _ = SelectObject(mask_dc, old_mask_bmp);
    let _ = DeleteDC(mem_dc);
    let _ = DeleteDC(mask_dc);
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
