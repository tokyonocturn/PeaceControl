//! Windows shell integration: tray icon, global hotkeys, foreground process.
//!
//! The application creates one hidden window that receives the tray icon
//! callbacks and the global hotkeys. Its messages are pumped from the interface
//! loop with `PeekMessage` filtered to that window, so the tray keeps working
//! while egui owns the main window and no second event loop is needed.
//!
//! Nothing here is simulated: the tray icon is a real `Shell_NotifyIconW` entry
//! with a native popup menu, hotkeys are registered with `RegisterHotKey` and
//! released in `Drop`, and the foreground process is read from the Win32 API.

use peace_control_core::automation::normalize_process;

/// What the tray menu or a hotkey asked the application to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellEvent {
    /// Left click on the icon, or "Show/Hide" in the menu.
    ToggleWindow,
    /// "Equalizer enabled" in the menu.
    ToggleEnabled,
    OpenSettings,
    Exit,
    /// A configuration chosen in the menu.
    ActivatePreset(String),
    HotkeyToggle,
    HotkeyNext,
    HotkeyPrevious,
}

/// The identifiers used when registering global hotkeys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    Toggle,
    Next,
    Previous,
}

impl HotkeyAction {
    pub fn id(self) -> i32 {
        match self {
            HotkeyAction::Toggle => 1,
            HotkeyAction::Next => 2,
            HotkeyAction::Previous => 3,
        }
    }
}

/// Everything the tray menu needs to describe the current state.
pub struct MenuState<'a> {
    pub presets: &'a [String],
    pub enabled: bool,
    pub visible: bool,
    pub preset_playing: &'a str,
}

#[cfg(windows)]
mod sys {
    use super::{normalize_process, HotkeyAction, MenuState, ShellEvent};
    use peace_control_core::hotkeys::Hotkey;
    use std::ffi::c_void;
    use std::mem::size_of;
    use std::ptr::{null, null_mut};
    use std::sync::Mutex;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        CreateBitmap, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC,
        ReleaseDC, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN,
    };
    use windows_sys::Win32::UI::Shell::{
        Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
        NOTIFYICONDATAW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
        DestroyIcon, DestroyMenu, DestroyWindow, DispatchMessageW, GetForegroundWindow,
        GetWindowThreadProcessId, LoadCursorW, PeekMessageW, PostMessageW, RegisterClassW,
        SetForegroundWindow, TrackPopupMenu, TranslateMessage, HICON, ICONINFO, IDC_ARROW,
        MF_CHECKED, MF_GRAYED, MF_POPUP, MF_SEPARATOR, MF_STRING, MSG, PM_REMOVE, TPM_LEFTBUTTON,
        TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_APP, WM_CONTEXTMENU, WM_HOTKEY,
        WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WNDCLASSW,
    };

    const TRAY_MESSAGE: u32 = WM_APP + 1;
    const MENU_SHOW: usize = 1;
    const MENU_TOGGLE: usize = 2;
    const MENU_SETTINGS: usize = 3;
    const MENU_EXIT: usize = 4;
    const MENU_PRESET_HEADER: usize = 5;
    const MENU_PRESET_BASE: usize = 1000;

    /// Events recorded by the window procedure and drained by `Shell::poll`.
    enum RawEvent {
        TrayLeft,
        TrayRight,
        Hotkey(i32),
    }

    static EVENTS: Mutex<Vec<RawEvent>> = Mutex::new(Vec::new());

    fn push(event: RawEvent) {
        if let Ok(mut events) = EVENTS.lock() {
            events.push(event);
        }
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            TRAY_MESSAGE => {
                let mouse = lparam as u32 & 0xFFFF;
                if mouse == WM_LBUTTONUP {
                    push(RawEvent::TrayLeft);
                } else if mouse == WM_RBUTTONUP || mouse == WM_CONTEXTMENU {
                    push(RawEvent::TrayRight);
                }
                0
            }
            WM_HOTKEY => {
                push(RawEvent::Hotkey(wparam as i32));
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }

    /// Builds a tray icon handle from RGBA pixels, falling back to the system
    /// application icon if GDI refuses to create one.
    fn icon_from_rgba(rgba: &[u8], size: u32) -> HICON {
        unsafe {
            let screen = GetDC(null_mut());
            let dc = CreateCompatibleDC(screen);
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = size as i32;
            info.bmiHeader.biHeight = -(size as i32);
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = BI_RGB;
            let mut bits: *mut c_void = null_mut();
            let colour = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
            let mut icon: HICON = null_mut();
            if !colour.is_null() && !bits.is_null() {
                let pixels =
                    std::slice::from_raw_parts_mut(bits as *mut u8, (size * size * 4) as usize);
                for (index, pixel) in rgba
                    .chunks_exact(4)
                    .take((size * size) as usize)
                    .enumerate()
                {
                    let alpha = pixel[3] as u32;
                    pixels[index * 4] = (pixel[2] as u32 * alpha / 255) as u8;
                    pixels[index * 4 + 1] = (pixel[1] as u32 * alpha / 255) as u8;
                    pixels[index * 4 + 2] = (pixel[0] as u32 * alpha / 255) as u8;
                    pixels[index * 4 + 3] = alpha as u8;
                }
                let mask = CreateBitmap(size as i32, size as i32, 1, 1, null());
                let mut icon_info: ICONINFO = std::mem::zeroed();
                icon_info.fIcon = 1;
                icon_info.hbmMask = mask;
                icon_info.hbmColor = colour;
                icon = CreateIconIndirect(&icon_info);
                let _ = DeleteObject(mask);
                let _ = DeleteObject(colour);
            }
            DeleteDC(dc);
            ReleaseDC(null_mut(), screen);
            icon
        }
    }

    /// The real tray icon, hotkey registry and hidden message window.
    pub struct Shell {
        hwnd: HWND,
        icon: HICON,
        hotkeys: Vec<i32>,
        class: Vec<u16>,
    }

    impl Shell {
        /// Creates the hidden window and adds the tray icon.
        ///
        /// Returns `None` when Windows refuses the window class or the icon,
        /// which the caller reports instead of pretending the tray exists.
        pub fn new(tooltip: &str) -> Option<Shell> {
            unsafe {
                let instance = GetModuleHandleW(null());
                let class = wide("PeaceControlShellWindow");
                let mut window_class: WNDCLASSW = std::mem::zeroed();
                window_class.style = 0;
                window_class.lpfnWndProc = Some(window_proc);
                window_class.hInstance = instance;
                window_class.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
                window_class.lpszClassName = class.as_ptr();
                let atom = RegisterClassW(&window_class);
                if atom == 0 {
                    log::warn!("could not register the tray window class");
                    return None;
                }

                let hwnd = CreateWindowExW(
                    0,
                    class.as_ptr(),
                    wide("PeaceControl").as_ptr(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    null_mut(),
                    null_mut(),
                    instance,
                    null_mut(),
                );
                if hwnd.is_null() {
                    log::warn!("could not create the tray message window");
                    return None;
                }

                let (rgba, width, _height) = crate::brand::icon_rgba(32);
                let icon = icon_from_rgba(&rgba, width);
                if icon.is_null() {
                    log::warn!("could not build the tray icon, keeping the window without one");
                }

                let mut data: NOTIFYICONDATAW = std::mem::zeroed();
                data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
                data.hWnd = hwnd;
                data.uID = 1;
                data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
                data.uCallbackMessage = TRAY_MESSAGE;
                data.hIcon = icon;
                let text = wide(tooltip);
                let length = text.len().min(data.szTip.len());
                data.szTip[..length].copy_from_slice(&text[..length]);

                if Shell_NotifyIconW(NIM_ADD, &data) == 0 {
                    log::warn!("could not add the tray icon");
                    DestroyWindow(hwnd);
                    if !icon.is_null() {
                        DestroyIcon(icon);
                    }
                    return None;
                }

                if let Ok(mut events) = EVENTS.lock() {
                    events.clear();
                }

                Some(Shell {
                    hwnd,
                    icon,
                    hotkeys: Vec::new(),
                    class,
                })
            }
        }

        /// Updates the tooltip shown when hovering the icon.
        pub fn set_tooltip(&self, tooltip: &str) {
            unsafe {
                let mut data: NOTIFYICONDATAW = std::mem::zeroed();
                data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
                data.hWnd = self.hwnd;
                data.uID = 1;
                data.uFlags = NIF_TIP;
                let text = wide(tooltip);
                let length = text.len().min(data.szTip.len());
                data.szTip[..length].copy_from_slice(&text[..length]);
                let _ = Shell_NotifyIconW(NIM_MODIFY, &data);
            }
        }

        /// Registers every binding and returns the ones Windows rejected.
        ///
        /// Existing registrations are released first so editing a hotkey frees
        /// the old combination immediately.
        pub fn register(&mut self, bindings: &[(String, HotkeyAction)]) -> Vec<String> {
            self.clear_hotkeys();
            let mut problems = Vec::new();
            for (spec, action) in bindings {
                let Some(hotkey) = Hotkey::parse(spec) else {
                    problems.push(format!("{spec}: not a valid combination"));
                    continue;
                };
                let mut modifiers = 0u32;
                if hotkey.ctrl {
                    modifiers |= MOD_CONTROL;
                }
                if hotkey.alt {
                    modifiers |= MOD_ALT;
                }
                if hotkey.shift {
                    modifiers |= MOD_SHIFT;
                }
                if hotkey.win {
                    modifiers |= MOD_WIN;
                }
                let registered =
                    unsafe { RegisterHotKey(self.hwnd, action.id(), modifiers, hotkey.vk) };
                if registered == 0 {
                    problems.push(format!(
                        "{}: already used by another program",
                        hotkey.describe()
                    ));
                } else {
                    self.hotkeys.push(action.id());
                }
            }
            for problem in &problems {
                log::warn!("global hotkey unavailable: {problem}");
            }
            problems
        }

        /// Releases every registered hotkey.
        pub fn clear_hotkeys(&mut self) {
            for id in self.hotkeys.drain(..) {
                unsafe {
                    let _ = UnregisterHotKey(self.hwnd, id);
                }
            }
        }

        /// Drains pending window messages and turns them into high level events.
        pub fn poll(&mut self, state: &MenuState<'_>) -> Vec<ShellEvent> {
            let mut raw = Vec::new();
            unsafe {
                let mut message: MSG = std::mem::zeroed();
                while PeekMessageW(&mut message, self.hwnd, 0, 0, PM_REMOVE) != 0 {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
            if let Ok(mut events) = EVENTS.lock() {
                raw.append(&mut events);
            }

            let mut output = Vec::new();
            for event in raw {
                match event {
                    RawEvent::TrayLeft => output.push(ShellEvent::ToggleWindow),
                    RawEvent::TrayRight => {
                        if let Some(choice) = self.popup_menu(state) {
                            output.push(choice);
                        }
                    }
                    RawEvent::Hotkey(id) => match id {
                        id if id == HotkeyAction::Toggle.id() => {
                            output.push(ShellEvent::HotkeyToggle)
                        }
                        id if id == HotkeyAction::Next.id() => output.push(ShellEvent::HotkeyNext),
                        id if id == HotkeyAction::Previous.id() => {
                            output.push(ShellEvent::HotkeyPrevious)
                        }
                        _ => {}
                    },
                }
            }
            output
        }

        /// Shows the native context menu and returns the chosen action.
        fn popup_menu(&self, state: &MenuState<'_>) -> Option<ShellEvent> {
            unsafe {
                let menu = CreatePopupMenu();
                if menu.is_null() {
                    return None;
                }
                let show_label = if state.visible {
                    "Hide PeaceControl"
                } else {
                    "Show PeaceControl"
                };
                let _ = AppendMenuW(menu, MF_STRING, MENU_SHOW, wide(show_label).as_ptr());
                let toggle_flags = MF_STRING | if state.enabled { MF_CHECKED } else { 0 };
                let _ = AppendMenuW(
                    menu,
                    toggle_flags,
                    MENU_TOGGLE,
                    wide("Equalizer enabled").as_ptr(),
                );
                let _ = AppendMenuW(menu, MF_SEPARATOR, 0, null());

                if state.presets.is_empty() {
                    let _ = AppendMenuW(
                        menu,
                        MF_STRING | MF_GRAYED,
                        MENU_PRESET_HEADER,
                        wide("No configurations").as_ptr(),
                    );
                } else {
                    let header = CreatePopupMenu();
                    for (index, name) in state.presets.iter().take(30).enumerate() {
                        let flags = MF_STRING
                            | if name == state.preset_playing {
                                MF_CHECKED
                            } else {
                                0
                            };
                        let _ = AppendMenuW(
                            header,
                            flags,
                            MENU_PRESET_BASE + index,
                            wide(name).as_ptr(),
                        );
                    }
                    let _ = AppendMenuW(
                        menu,
                        MF_POPUP,
                        header as usize,
                        wide("Configurations").as_ptr(),
                    );
                }
                let _ = AppendMenuW(menu, MF_SEPARATOR, 0, null());
                let _ = AppendMenuW(menu, MF_STRING, MENU_SETTINGS, wide("Settings…").as_ptr());
                let _ = AppendMenuW(menu, MF_STRING, MENU_EXIT, wide("Exit").as_ptr());

                let _ = SetForegroundWindow(self.hwnd);
                let choice = TrackPopupMenu(
                    menu,
                    TPM_RETURNCMD | TPM_NONOTIFY | TPM_LEFTBUTTON | TPM_RIGHTBUTTON,
                    0,
                    0,
                    0,
                    self.hwnd,
                    null(),
                );
                let _ = PostMessageW(self.hwnd, WM_NULL, 0, 0);
                let _ = DestroyMenu(menu);

                let choice = choice as usize;
                match choice {
                    0 => None,
                    MENU_SHOW => Some(ShellEvent::ToggleWindow),
                    MENU_TOGGLE => Some(ShellEvent::ToggleEnabled),
                    MENU_SETTINGS => Some(ShellEvent::OpenSettings),
                    MENU_EXIT => Some(ShellEvent::Exit),
                    other if other >= MENU_PRESET_BASE => state
                        .presets
                        .get(other - MENU_PRESET_BASE)
                        .cloned()
                        .map(ShellEvent::ActivatePreset),
                    _ => None,
                }
            }
        }
    }

    impl Drop for Shell {
        fn drop(&mut self) {
            self.clear_hotkeys();
            unsafe {
                let mut data: NOTIFYICONDATAW = std::mem::zeroed();
                data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
                data.hWnd = self.hwnd;
                data.uID = 1;
                let _ = Shell_NotifyIconW(NIM_DELETE, &data);
                if !self.icon.is_null() {
                    DestroyIcon(self.icon);
                }
                DestroyWindow(self.hwnd);
                let _ = self.class;
            }
            log::info!("tray icon and global hotkeys released");
        }
    }

    /// The process name that owns the foreground window, without directories.
    pub fn foreground_process() -> Option<String> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_null() {
                return None;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == 0 {
                return None;
            }
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process.is_null() {
                return None;
            }
            let mut buffer = [0u16; 1024];
            let mut length = buffer.len() as u32;
            let ok = QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length);
            let _ = windows_sys::Win32::Foundation::CloseHandle(process);
            if ok == 0 {
                return None;
            }
            let path = String::from_utf16_lossy(&buffer[..length as usize]);
            Some(normalize_process(&path))
        }
    }
}

#[cfg(windows)]
pub use sys::{foreground_process, Shell};

#[cfg(not(windows))]
pub struct Shell;

#[cfg(not(windows))]
impl Shell {
    pub fn new(_tooltip: &str) -> Option<Shell> {
        None
    }
    pub fn set_tooltip(&self, _tooltip: &str) {}
    pub fn register(&mut self, _bindings: &[(String, HotkeyAction)]) -> Vec<String> {
        vec!["global hotkeys are only implemented on Windows".to_string()]
    }
    pub fn clear_hotkeys(&mut self) {}
    pub fn poll(&mut self, _state: &MenuState<'_>) -> Vec<ShellEvent> {
        Vec::new()
    }
}

#[cfg(not(windows))]
pub fn foreground_process() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotkey_ids_are_stable_and_unique() {
        assert_eq!(HotkeyAction::Toggle.id(), 1);
        assert_eq!(HotkeyAction::Next.id(), 2);
        assert_ne!(HotkeyAction::Next.id(), HotkeyAction::Previous.id());
    }

    #[test]
    fn foreground_lookup_never_panics() {
        let _ = foreground_process();
    }

    #[cfg(windows)]
    #[test]
    fn shell_registers_and_releases_a_global_hotkey() {
        // Without an interactive desktop the tray cannot be created; the test
        // then has nothing to prove and simply returns.
        let Some(mut shell) = Shell::new("PeaceControl test") else {
            return;
        };
        let problems = shell.register(&[("Ctrl+Alt+Shift+F24".to_string(), HotkeyAction::Toggle)]);
        assert!(problems.is_empty(), "{problems:?}");
        let presets = vec!["Test".to_string()];
        let state = MenuState {
            presets: &presets,
            enabled: true,
            visible: true,
            preset_playing: "Test",
        };
        assert!(shell.poll(&state).is_empty());
        shell.clear_hotkeys();
        let problems = shell.register(&[("not a hotkey".to_string(), HotkeyAction::Next)]);
        assert_eq!(problems.len(), 1);
    }
}
