#![windows_subsystem = "windows"]

use std::sync::atomic::{AtomicBool, Ordering};
use tray_icon::{TrayIconBuilder, menu::{Menu, MenuItem, MenuEvent}, Icon};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

static mut HOOK: HHOOK = HHOOK(std::ptr::null_mut());
static CAPSLOCK_PRESSED: AtomicBool = AtomicBool::new(false);

// Low-level keyboard hook callback
unsafe extern "system" fn keyboard_hook_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 {
        let kb = *(lparam.0 as *const KBDLLHOOKSTRUCT);
        let vk_code = VIRTUAL_KEY(kb.vkCode as u16);
        let is_key_down = wparam.0 == WM_KEYDOWN as usize || wparam.0 == WM_SYSKEYDOWN as usize;
        let is_key_up = wparam.0 == WM_KEYUP as usize || wparam.0 == WM_SYSKEYUP as usize;

        // Handle CapsLock -> Ctrl
        if vk_code == VK_CAPITAL {
            if is_key_down {
                CAPSLOCK_PRESSED.store(true, Ordering::SeqCst);
                send_key_event(VK_LCONTROL, true);
            } else if is_key_up {
                CAPSLOCK_PRESSED.store(false, Ordering::SeqCst);
                send_key_event(VK_LCONTROL, false);
            }
            return LRESULT(1); // Block the original CapsLock key
        }

        // Handle Ctrl+HJKL -> Arrow keys (only when Ctrl IS pressed)
        let ctrl_pressed = CAPSLOCK_PRESSED.load(Ordering::SeqCst)
            || GetAsyncKeyState(VK_LCONTROL.0 as i32) as u16 & 0x8000 != 0
            || GetAsyncKeyState(VK_RCONTROL.0 as i32) as u16 & 0x8000 != 0;

        if ctrl_pressed {
            let arrow_key = match vk_code {
                VK_H => Some(VK_LEFT),
                VK_J => Some(VK_DOWN),
                VK_K => Some(VK_UP),
                VK_L => Some(VK_RIGHT),
                _ => None,
            };

            if let Some(arrow) = arrow_key {
                // Release Ctrl before sending arrow key
                if is_key_down {
                    if CAPSLOCK_PRESSED.load(Ordering::SeqCst) {
                        send_key_event(VK_LCONTROL, false);
                    } else if GetAsyncKeyState(VK_LCONTROL.0 as i32) as u16 & 0x8000 != 0 {
                        send_key_event(VK_LCONTROL, false);
                    } else if GetAsyncKeyState(VK_RCONTROL.0 as i32) as u16 & 0x8000 != 0 {
                        send_key_event(VK_RCONTROL, false);
                    }
                    send_key_event(arrow, true);
                } else if is_key_up {
                    send_key_event(arrow, false);
                    // Re-press Ctrl if CapsLock is still held
                    if CAPSLOCK_PRESSED.load(Ordering::SeqCst) {
                        send_key_event(VK_LCONTROL, true);
                    } else if GetAsyncKeyState(VK_LCONTROL.0 as i32) as u16 & 0x8000 != 0 {
                        send_key_event(VK_LCONTROL, true);
                    } else if GetAsyncKeyState(VK_RCONTROL.0 as i32) as u16 & 0x8000 != 0 {
                        send_key_event(VK_RCONTROL, true);
                    }
                }
                return LRESULT(1); // Block the original HJKL key
            }
        }
    }

    CallNextHookEx(HOOK, code, wparam, lparam)
}

// Send synthetic key event
unsafe fn send_key_event(vk: VIRTUAL_KEY, is_down: bool) {
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if is_down { KEYBD_EVENT_FLAGS(0) } else { KEYEVENTF_KEYUP },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
}

// Create a keyboard-themed icon (HJKL keys with arrows)
fn create_icon() -> Icon {
    use image::{ImageBuffer, Rgba};

    let size = 64u32;
    let mut img = ImageBuffer::from_fn(size, size, |_, _| Rgba([0u8, 0u8, 0u8, 0u8]));

    // Draw a simple keyboard key representation with HJKL
    for y in 12..52 {
        for x in 12..52 {
            // Outer border (white/gray)
            if x == 12 || x == 51 || y == 12 || y == 51 {
                img.put_pixel(x, y, Rgba([200, 200, 200, 255]));
            }
            // Inner fill (dark blue/purple gradient)
            else if x > 12 && x < 51 && y > 12 && y < 51 {
                let intensity = 60 + ((y - 12) * 2) as u8;
                img.put_pixel(x, y, Rgba([80, 60, intensity, 255]));
            }
        }
    }

    // Draw "HJKL" text pattern in the center (simplified pixel font)
    let letter_positions = [(18, 26), (26, 26), (34, 26), (42, 26)]; // H J K L positions

    for (cx, cy) in letter_positions {
        // Draw small white dots to represent keys
        for dy in 0..3 {
            for dx in 0..3 {
                img.put_pixel(cx + dx, cy + dy, Rgba([255, 255, 255, 255]));
            }
        }
    }

    // Draw small arrow indicators
    let arrow_y = 38;
    // Left arrow
    img.put_pixel(19, arrow_y, Rgba([100, 200, 255, 255]));
    img.put_pixel(20, arrow_y - 1, Rgba([100, 200, 255, 255]));
    img.put_pixel(20, arrow_y + 1, Rgba([100, 200, 255, 255]));

    // Down arrow
    img.put_pixel(27, arrow_y + 1, Rgba([100, 200, 255, 255]));
    img.put_pixel(27 - 1, arrow_y, Rgba([100, 200, 255, 255]));
    img.put_pixel(27 + 1, arrow_y, Rgba([100, 200, 255, 255]));

    // Up arrow
    img.put_pixel(35, arrow_y - 1, Rgba([100, 200, 255, 255]));
    img.put_pixel(35 - 1, arrow_y, Rgba([100, 200, 255, 255]));
    img.put_pixel(35 + 1, arrow_y, Rgba([100, 200, 255, 255]));

    // Right arrow
    img.put_pixel(43, arrow_y, Rgba([100, 200, 255, 255]));
    img.put_pixel(42, arrow_y - 1, Rgba([100, 200, 255, 255]));
    img.put_pixel(42, arrow_y + 1, Rgba([100, 200, 255, 255]));

    let rgba = img.into_raw();
    Icon::from_rgba(rgba, size, size).expect("Failed to create icon")
}

fn main() {
    // Install low-level keyboard hook
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap();
        HOOK = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), hinstance, 0).unwrap();
    }

    // Create custom icon
    let icon = create_icon();

    // Create system tray menu
    let menu = Menu::new();
    let quit_item = MenuItem::new("Quit", true, None);
    menu.append(&quit_item).unwrap();

    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("win-c-hjkl - Keyboard Remapper\nCapsLock→Ctrl, Ctrl+HJKL→Arrows")
        .with_icon(icon)
        .build()
        .unwrap();

    let menu_channel = MenuEvent::receiver();

    // Message loop
    loop {
        // Check for tray/menu events
        if let Ok(event) = menu_channel.try_recv() {
            if event.id == quit_item.id() {
                break;
            }
        }

        // Process Windows messages
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE).as_bool() {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // Cleanup
    unsafe {
        UnhookWindowsHookEx(HOOK).ok();
    }
}
