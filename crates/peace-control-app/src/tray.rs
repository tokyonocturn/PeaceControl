//! Windows shell integration: tray icon, global hotkeys, foreground process.
//!
//! The application creates one hidden window that receives the tray icon
//! callbacks, the global hotkeys and the tray menu's `WM_COMMAND` messages. Its
//! messages are pumped from the interface loop with `PeekMessage` filtered to
//! that window, so the tray keeps working while egui owns the main window and
//! no second event loop is needed.
//!
//! Nothing here is simulated: the tray icon is a real `Shell_NotifyIconW` entry,
//! the popup is a real Win32 menu whose selections arrive as `WM_COMMAND`
//! (exactly what Explorer sends), hotkeys are registered with `RegisterHotKey`
//! and released in `Drop`, and the foreground process is read from Win32.

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

/// One selectable tray menu command, with the identifier Windows puts in
/// `WM_COMMAND` when the item is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuCommand {
    ToggleWindow,
    ToggleEnabled,
    OpenSettings,
    Exit,
    /// Index into the configuration list shown in the menu.
    Preset(usize),
}

impl MenuCommand {
    pub const PRESET_BASE: usize = 1000;

    pub fn id(self) -> usize {
        match self {
            MenuCommand::ToggleWindow => 1,
            MenuCommand::ToggleEnabled => 2,
            MenuCommand::OpenSettings => 3,
            MenuCommand::Exit => 4,
            MenuCommand::Preset(index) => Self::PRESET_BASE + index,
        }
    }

    pub fn from_id(id: usize) -> Option<MenuCommand> {
        match id {
            1 => Some(MenuCommand::ToggleWindow),
            2 => Some(MenuCommand::ToggleEnabled),
            3 => Some(MenuCommand::OpenSettings),
            4 => Some(MenuCommand::Exit),
            id if id >= Self::PRESET_BASE => Some(MenuCommand::Preset(id - Self::PRESET_BASE)),
            _ => None,
        }
    }
}

/// A node of the popup menu. The menu is described as data so its labels,
/// identifiers and check marks can be asserted in tests without a desktop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuNode {
    Item {
        label: String,
        command: Option<MenuCommand>,
        checked: bool,
        enabled: bool,
    },
    Separator,
    Submenu {
        label: String,
        items: Vec<MenuNode>,
    },
}

/// Everything the tray menu needs to describe the current state.
pub struct MenuState<'a> {
    pub presets: &'a [String],
    pub enabled: bool,
    pub visible: bool,
    pub preset_playing: &'a str,
}

/// Builds the popup menu contents for the given state.
///
/// The check marks come straight from the live state, so reopening the menu
/// always reflects what the application is currently doing.
pub fn menu_tree(state: &MenuState<'_>) -> Vec<MenuNode> {
    let mut nodes = Vec::new();
    nodes.push(MenuNode::Item {
        label: if state.visible {
            "Hide PeaceControl".to_string()
        } else {
            "Show PeaceControl".to_string()
        },
        command: Some(MenuCommand::ToggleWindow),
        checked: false,
        enabled: true,
    });
    nodes.push(MenuNode::Item {
        label: "Equalizer enabled".to_string(),
        command: Some(MenuCommand::ToggleEnabled),
        checked: state.enabled,
        enabled: true,
    });
    nodes.push(MenuNode::Separator);

    if state.presets.is_empty() {
        nodes.push(MenuNode::Item {
            label: "No configurations".to_string(),
            command: None,
            checked: false,
            enabled: false,
        });
    } else {
        nodes.push(MenuNode::Submenu {
            label: "Configurations".to_string(),
            items: state
                .presets
                .iter()
                .take(30)
                .enumerate()
                .map(|(index, name)| MenuNode::Item {
                    label: name.clone(),
                    command: Some(MenuCommand::Preset(index)),
                    checked: name == state.preset_playing,
                    enabled: true,
                })
                .collect(),
        });
    }

    nodes.push(MenuNode::Separator);
    nodes.push(MenuNode::Item {
        label: "Settings…".to_string(),
        command: Some(MenuCommand::OpenSettings),
        checked: false,
        enabled: true,
    });
    nodes.push(MenuNode::Item {
        label: "Exit".to_string(),
        command: Some(MenuCommand::Exit),
        checked: false,
        enabled: true,
    });
    nodes
}

/// Turns a `WM_COMMAND` identifier plus the visible configuration list into an
/// application event. This is the single place menu identifiers are decoded.
pub fn event_for_command(command: MenuCommand, presets: &[String]) -> Option<ShellEvent> {
    match command {
        MenuCommand::ToggleWindow => Some(ShellEvent::ToggleWindow),
        MenuCommand::ToggleEnabled => Some(ShellEvent::ToggleEnabled),
        MenuCommand::OpenSettings => Some(ShellEvent::OpenSettings),
        MenuCommand::Exit => Some(ShellEvent::Exit),
        MenuCommand::Preset(index) => presets.get(index).cloned().map(ShellEvent::ActivatePreset),
    }
}

#[cfg(windows)]
mod sys {
    use super::{
        event_for_command, menu_tree, normalize_process, HotkeyAction, MenuCommand, MenuNode,
        MenuState, ShellEvent,
    };
    use peace_control_core::hotkeys::Hotkey;
    use std::ffi::c_void;
    use std::mem::size_of;
    use std::ptr::{null, null_mut};
    use std::sync::Mutex;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
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
        DestroyIcon, DestroyMenu, DestroyWindow, DispatchMessageW, FindWindowW,
        GetForegroundWindow, GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId,
        LoadCursorW, PeekMessageW, PostMessageW, RegisterClassW, SetForegroundWindow,
        SetWindowLongPtrW, SetWindowPos, ShowWindow, TrackPopupMenu, TranslateMessage, GWL_EXSTYLE,
        HICON, HMENU, ICONINFO, IDC_ARROW, MF_CHECKED, MF_GRAYED, MF_POPUP, MF_SEPARATOR,
        MF_STRING, MSG, PM_REMOVE, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW,
        SW_RESTORE, SW_SHOWNOACTIVATE, TPM_LEFTBUTTON, TPM_RIGHTBUTTON, WM_APP, WM_COMMAND,
        WM_CONTEXTMENU, WM_HOTKEY, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WNDCLASSW, WS_EX_APPWINDOW,
        WS_EX_TOOLWINDOW,
    };

    const TRAY_MESSAGE: u32 = WM_APP + 1;

    /// Where the main window is parked while the application runs in the tray.
    /// The window stays shown (off-screen) rather than hidden, because Windows
    /// stops painting a hidden window and egui only calls `update` while it
    /// paints - and `update` is what drains the tray and hotkey queue.
    const PARKED: i32 = -24000;

    /// The rectangle the window was last shown at, so Show restores it exactly.
    static LAST_SHOWN: Mutex<Option<(i32, i32, i32, i32)>> = Mutex::new(None);

    /// The application's main window, matched by title and owned by this process.
    fn main_window() -> Option<HWND> {
        unsafe {
            let title = wide(&format!("PeaceControl {}", peace_control_core::VERSION));
            let hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
            if hwnd.is_null() {
                return None;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid != windows_sys::Win32::System::Threading::GetCurrentProcessId() {
                return None;
            }
            Some(hwnd)
        }
    }

    /// Parks the main window off-screen without a taskbar button.
    ///
    /// Returns false when the window cannot be found, which lets the caller fall
    /// back to a plain hide.
    pub fn hide_window() -> bool {
        let Some(hwnd) = main_window() else {
            log::warn!("could not find the main window to park it in the tray");
            return false;
        };
        unsafe {
            let mut rect: RECT = std::mem::zeroed();
            if GetWindowRect(hwnd, &mut rect) != 0 && rect.left > PARKED {
                if let Ok(mut saved) = LAST_SHOWN.lock() {
                    *saved = Some((
                        rect.left,
                        rect.top,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                    ));
                }
            }
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(
                hwnd,
                GWL_EXSTYLE,
                (style | WS_EX_TOOLWINDOW as isize) & !(WS_EX_APPWINDOW as isize),
            );
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                PARKED,
                PARKED,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        true
    }

    /// Brings the window back on screen at the rectangle it was parked from.
    pub fn show_window() -> bool {
        let Some(hwnd) = main_window() else {
            log::warn!("could not find the main window to restore it");
            return false;
        };
        let saved = LAST_SHOWN.lock().ok().and_then(|rect| *rect);
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(
                hwnd,
                GWL_EXSTYLE,
                (style & !(WS_EX_TOOLWINDOW as isize)) | WS_EX_APPWINDOW as isize,
            );
            let mut flags = SWP_NOZORDER | SWP_NOACTIVATE | SWP_SHOWWINDOW;
            let (x, y, width, height) = match saved {
                Some((x, y, width, height)) => (x, y, width, height),
                None => {
                    flags |= SWP_NOSIZE;
                    (160, 120, 0, 0)
                }
            };
            SetWindowPos(hwnd, std::ptr::null_mut(), x, y, width, height, flags);
            ShowWindow(hwnd, SW_RESTORE);
            SetForegroundWindow(hwnd);
        }
        true
    }

    /// Events recorded by the window procedure and drained by `Shell::poll`.
    enum RawEvent {
        TrayLeft,
        TrayRight,
        Hotkey(i32),
        Command(usize),
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
            // WM_COMMAND is what Windows sends when a menu item is chosen, so
            // real clicks and posted commands share this exact path.
            WM_COMMAND => {
                push(RawEvent::Command(wparam & 0xFFFF));
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

    /// Appends a menu tree to a Win32 popup menu.
    unsafe fn append_nodes(menu: HMENU, nodes: &[MenuNode]) {
        for node in nodes {
            match node {
                MenuNode::Separator => {
                    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, null());
                }
                MenuNode::Item {
                    label,
                    command,
                    checked,
                    enabled,
                } => {
                    let mut flags = MF_STRING;
                    if *checked {
                        flags |= MF_CHECKED;
                    }
                    if !*enabled {
                        flags |= MF_GRAYED;
                    }
                    let id = command.map(MenuCommand::id).unwrap_or(0);
                    let _ = AppendMenuW(menu, flags, id, wide(label).as_ptr());
                }
                MenuNode::Submenu { label, items } => {
                    let submenu = CreatePopupMenu();
                    if submenu.is_null() {
                        continue;
                    }
                    append_nodes(submenu, items);
                    let _ = AppendMenuW(menu, MF_POPUP, submenu as usize, wide(label).as_ptr());
                }
            }
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

        /// Sends a menu command exactly as the shell does after a selection.
        ///
        /// This posts `WM_COMMAND` to the tray window, which is the same message
        /// a real menu click produces, so tests and users share one code path.
        #[cfg(test)]
        pub fn post_command(&self, command: MenuCommand) {
            unsafe {
                let _ = PostMessageW(self.hwnd, WM_COMMAND, command.id(), 0);
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
                    RawEvent::TrayRight => self.popup_menu(state),
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
                    RawEvent::Command(id) => {
                        if let Some(command) = MenuCommand::from_id(id) {
                            if let Some(event) = event_for_command(command, state.presets) {
                                output.push(event);
                            } else {
                                log::warn!("tray command {id} refers to a missing configuration");
                            }
                        } else {
                            log::warn!("unknown tray command id {id}");
                        }
                    }
                }
            }
            output
        }

        /// Shows the native context menu. Selections arrive later as
        /// `WM_COMMAND` and are decoded by [`Shell::poll`].
        fn popup_menu(&self, state: &MenuState<'_>) {
            unsafe {
                let menu = CreatePopupMenu();
                if menu.is_null() {
                    return;
                }
                append_nodes(menu, &menu_tree(state));
                let _ = SetForegroundWindow(self.hwnd);
                // Without TPM_RETURNCMD Windows posts the selection as
                // WM_COMMAND to this window, which is the path a user takes.
                let _ = TrackPopupMenu(
                    menu,
                    TPM_LEFTBUTTON | TPM_RIGHTBUTTON,
                    0,
                    0,
                    0,
                    self.hwnd,
                    null(),
                );
                let _ = PostMessageW(self.hwnd, WM_NULL, 0, 0);
                let _ = DestroyMenu(menu);
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
pub use sys::{foreground_process, hide_window, show_window, Shell};

#[cfg(not(windows))]
pub struct Shell;

#[cfg(not(windows))]
impl Shell {
    pub fn new(_tooltip: &str) -> Option<Shell> {
        None
    }
    pub fn set_tooltip(&self, _tooltip: &str) {}
    #[cfg(test)]
    pub fn post_command(&self, _command: MenuCommand) {}
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

#[cfg(not(windows))]
pub fn hide_window() -> bool {
    false
}

#[cfg(not(windows))]
pub fn show_window() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state<'a>(
        presets: &'a [String],
        enabled: bool,
        visible: bool,
        playing: &'a str,
    ) -> MenuState<'a> {
        MenuState {
            presets,
            enabled,
            visible,
            preset_playing: playing,
        }
    }

    fn labels(nodes: &[MenuNode]) -> Vec<String> {
        nodes
            .iter()
            .map(|node| match node {
                MenuNode::Item { label, .. } => label.clone(),
                MenuNode::Separator => "---".to_string(),
                MenuNode::Submenu { label, .. } => label.clone(),
            })
            .collect()
    }

    #[test]
    fn hotkey_ids_are_stable_and_unique() {
        assert_eq!(HotkeyAction::Toggle.id(), 1);
        assert_eq!(HotkeyAction::Next.id(), 2);
        assert_ne!(HotkeyAction::Next.id(), HotkeyAction::Previous.id());
    }

    #[test]
    fn command_ids_round_trip() {
        for command in [
            MenuCommand::ToggleWindow,
            MenuCommand::ToggleEnabled,
            MenuCommand::OpenSettings,
            MenuCommand::Exit,
            MenuCommand::Preset(0),
            MenuCommand::Preset(7),
            MenuCommand::Preset(29),
        ] {
            assert_eq!(MenuCommand::from_id(command.id()), Some(command));
        }
        assert_eq!(MenuCommand::from_id(0), None);
        assert_eq!(MenuCommand::from_id(999), None);
        assert_eq!(MenuCommand::Preset(0).id(), 1000);
    }

    #[test]
    fn menu_describes_the_live_state() {
        let presets = vec!["Flat".to_string(), "Bass".to_string()];
        let visible = menu_tree(&state(&presets, true, true, "Bass"));
        assert_eq!(
            labels(&visible),
            vec![
                "Hide PeaceControl",
                "Equalizer enabled",
                "---",
                "Configurations",
                "---",
                "Settings…",
                "Exit"
            ]
        );
        match &visible[0] {
            MenuNode::Item { command, .. } => {
                assert_eq!(*command, Some(MenuCommand::ToggleWindow))
            }
            other => panic!("unexpected first node: {other:?}"),
        }
        match &visible[1] {
            MenuNode::Item { checked, .. } => assert!(*checked),
            other => panic!("unexpected toggle node: {other:?}"),
        }
        let hidden = menu_tree(&state(&presets, false, false, "Flat"));
        match &hidden[0] {
            MenuNode::Item { label, .. } => assert_eq!(label, "Show PeaceControl"),
            other => panic!("unexpected first node: {other:?}"),
        }
        match &hidden[1] {
            MenuNode::Item { checked, .. } => assert!(!checked),
            other => panic!("unexpected toggle node: {other:?}"),
        }
    }

    #[test]
    fn configurations_submenu_checks_the_active_one() {
        let presets = vec!["Flat".to_string(), "Bass".to_string(), "Vocal".to_string()];
        let tree = menu_tree(&state(&presets, true, true, "Bass"));
        let submenu = tree
            .iter()
            .find_map(|node| match node {
                MenuNode::Submenu { items, .. } => Some(items),
                _ => None,
            })
            .expect("configurations submenu");
        assert_eq!(submenu.len(), 3);
        let checked: Vec<String> = submenu
            .iter()
            .filter_map(|node| match node {
                MenuNode::Item {
                    label,
                    command,
                    checked,
                    ..
                } if *checked => Some(format!("{label}:{}", command.unwrap().id())),
                _ => None,
            })
            .collect();
        assert_eq!(checked, vec!["Bass:1001".to_string()]);
    }

    #[test]
    fn an_empty_library_shows_a_disabled_placeholder() {
        let tree = menu_tree(&state(&[], true, true, ""));
        assert!(tree.iter().any(|node| matches!(
            node,
            MenuNode::Item {
                label,
                command: None,
                enabled: false,
                ..
            } if label == "No configurations"
        )));
    }

    #[test]
    fn commands_map_to_events_including_presets() {
        let presets = vec!["A".to_string(), "B".to_string()];
        assert_eq!(
            event_for_command(MenuCommand::ToggleWindow, &presets),
            Some(ShellEvent::ToggleWindow)
        );
        assert_eq!(
            event_for_command(MenuCommand::ToggleEnabled, &presets),
            Some(ShellEvent::ToggleEnabled)
        );
        assert_eq!(
            event_for_command(MenuCommand::OpenSettings, &presets),
            Some(ShellEvent::OpenSettings)
        );
        assert_eq!(
            event_for_command(MenuCommand::Exit, &presets),
            Some(ShellEvent::Exit)
        );
        assert_eq!(
            event_for_command(MenuCommand::Preset(1), &presets),
            Some(ShellEvent::ActivatePreset("B".to_string()))
        );
        assert_eq!(event_for_command(MenuCommand::Preset(9), &presets), None);
    }

    #[test]
    fn foreground_lookup_never_panics() {
        let _ = foreground_process();
    }

    #[cfg(windows)]
    #[test]
    fn posted_menu_commands_arrive_through_the_window_procedure() {
        // Without an interactive desktop the tray cannot be created; the test
        // then has nothing to prove and simply returns.
        let Some(mut shell) = Shell::new("PeaceControl test") else {
            return;
        };
        let presets = vec!["First".to_string(), "Second".to_string()];
        let menu_state = state(&presets, true, true, "First");

        // Nothing is queued yet.
        assert!(shell.poll(&menu_state).is_empty());

        let expectations = [
            (MenuCommand::ToggleWindow, ShellEvent::ToggleWindow),
            (MenuCommand::ToggleEnabled, ShellEvent::ToggleEnabled),
            (MenuCommand::OpenSettings, ShellEvent::OpenSettings),
            (MenuCommand::Exit, ShellEvent::Exit),
            (
                MenuCommand::Preset(1),
                ShellEvent::ActivatePreset("Second".to_string()),
            ),
        ];
        for (command, expected) in expectations {
            shell.post_command(command);
            // The message is queued; poll dispatches it through the real
            // window procedure before decoding the event.
            std::thread::sleep(std::time::Duration::from_millis(10));
            let events = shell.poll(&menu_state);
            assert_eq!(events, vec![expected], "command {command:?}");
        }

        // The configurations submenu offered by the menu is the list that the
        // identifiers point into, so an index past the end is dropped, not
        // mapped to another configuration.
        shell.post_command(MenuCommand::Preset(30));
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(shell.poll(&menu_state).is_empty());
    }
}
