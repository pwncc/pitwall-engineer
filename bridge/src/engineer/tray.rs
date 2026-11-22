//! Minimal Windows tray lifecycle; no game control, permissions, or network changes.
#[cfg(windows)]
pub fn start(
    root: std::path::PathBuf,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    label: String,
) {
    std::thread::spawn(move || unsafe {
        platform::run(
            root.join("status.json").to_string_lossy().into(),
            false,
            stop,
            label,
        )
    });
}
#[cfg(not(windows))]
pub fn start(_: std::path::PathBuf, _: std::sync::Arc<std::sync::atomic::AtomicBool>, _: String) {}
#[cfg(windows)]
pub fn start_receiver(url: String, stop: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    let label = format!(
        "Pitwall Engineer · receiver :{}",
        url.rsplit(':').next().unwrap_or("7437")
    );
    std::thread::spawn(move || unsafe { platform::run(url, true, stop, label) });
}
#[cfg(not(windows))]
pub fn start_receiver(_: String, _: std::sync::Arc<std::sync::atomic::AtomicBool>) {}
#[cfg(windows)]
mod platform {
    use std::sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    };
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Shell::{
                NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
                Shell_NotifyIconW,
            },
            WindowsAndMessaging::*,
        },
    };
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    static TARGET: OnceLock<String> = OnceLock::new();
    static RECEIVER: OnceLock<bool> = OnceLock::new();
    static STOP: OnceLock<Arc<AtomicBool>> = OnceLock::new();
    unsafe extern "system" fn procedure(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        if msg == WM_USER + 1 {
            if l as u32 == WM_LBUTTONDBLCLK {
                if let Some(root) = TARGET.get() {
                    let _ = open::that(root);
                }
            }
            if l as u32 == WM_RBUTTONUP {
                let menu = CreatePopupMenu();
                AppendMenuW(
                    menu,
                    MF_STRING,
                    1,
                    wide(if RECEIVER.get() == Some(&true) {
                        "Open engineer dashboard"
                    } else {
                        "Open agent status"
                    })
                    .as_ptr(),
                );
                AppendMenuW(
                    menu,
                    MF_STRING,
                    2,
                    wide(if RECEIVER.get() == Some(&true) {
                        "Stop engineer receiver"
                    } else {
                        "Stop telemetry agent"
                    })
                    .as_ptr(),
                );
                let mut p: POINT = std::mem::zeroed();
                GetCursorPos(&mut p);
                SetForegroundWindow(hwnd);
                let command = TrackPopupMenu(
                    menu,
                    TPM_RETURNCMD | TPM_NONOTIFY,
                    p.x,
                    p.y,
                    0,
                    hwnd,
                    std::ptr::null(),
                );
                DestroyMenu(menu);
                if command == 1 {
                    if let Some(root) = TARGET.get() {
                        let _ = open::that(root);
                    }
                }
                if command == 2 {
                    if let Some(stop) = STOP.get() {
                        stop.store(true, Ordering::Relaxed);
                    }
                    PostQuitMessage(0);
                }
            }
            return 0;
        }
        DefWindowProcW(hwnd, msg, w, l)
    }
    pub unsafe fn run(target: String, receiver: bool, stop: Arc<AtomicBool>, label: String) {
        let _ = TARGET.set(target);
        let _ = RECEIVER.set(receiver);
        let _ = STOP.set(stop);
        let module = GetModuleHandleW(std::ptr::null());
        let name = wide("LMUTelemetryAgentStatus");
        let class = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: module,
            lpszClassName: name.as_ptr(),
            ..std::mem::zeroed()
        };
        RegisterClassW(&class);
        let hwnd = CreateWindowExW(
            0,
            name.as_ptr(),
            name.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            module,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            return;
        }
        let mut data: NOTIFYICONDATAW = std::mem::zeroed();
        data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = hwnd;
        data.uID = 1;
        data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        data.uCallbackMessage = WM_USER + 1;
        data.hIcon = LoadIconW(module, wide("IDI_ICON1").as_ptr());
        if data.hIcon.is_null() {
            data.hIcon = LoadIconW(std::ptr::null_mut(), IDI_APPLICATION);
        }
        let tip: Vec<u16> = label.encode_utf16().take(127).collect();
        data.szTip[..tip.len()].copy_from_slice(&tip);
        Shell_NotifyIconW(NIM_ADD, &data);
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        Shell_NotifyIconW(NIM_DELETE, &data);
        DestroyWindow(hwnd);
    }
}
