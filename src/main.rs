use std::{ffi::OsString, os::windows::ffi::OsStringExt, time::Duration};

use clap::Parser;
use thiserror::Error;

const RIGHT_MOUSE_BUTTON: i32 = winapi::um::winuser::VK_RBUTTON;
const LEFT_MOUSE_BUTTON: i32 = winapi::um::winuser::VK_LBUTTON;
const MIDDLE_MOUSE_BUTTON: i32 = winapi::um::winuser::VK_MBUTTON;

#[derive(Error, Debug)]
enum Error {
    #[error("Failed to get mouse position")]
    GetMousePosition,
    #[error("Failed to set mouse position")]
    SetMousePosition,
}

type Result<T> = std::result::Result<T, Error>;

unsafe fn get_mouse_position() -> Result<(i32, i32)> {
    let mut point = std::mem::zeroed();
    if winapi::um::winuser::GetCursorPos(&mut point) == 0 {
        return Err(Error::GetMousePosition);
    }
    Ok((point.x, point.y))
}

unsafe fn set_mouse_position(x: i32, y: i32) -> Result<()> {
    if winapi::um::winuser::SetCursorPos(x, y) == 0 {
        return Err(Error::SetMousePosition);
    }
    Ok(())
}

unsafe fn get_mouse_button_pressed(button: i32) -> bool {
    winapi::um::winuser::GetAsyncKeyState(button) != 0
}

unsafe fn get_active_process() -> Option<String> {
    let mut process_id = 0;
    winapi::um::winuser::GetWindowThreadProcessId(
        winapi::um::winuser::GetForegroundWindow(),
        &mut process_id,
    );
    let handle = winapi::um::processthreadsapi::OpenProcess(
        winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION,
        0,
        process_id,
    );
    if handle.is_null() {
        return None;
    }
    let mut buffer = [0u16; 1024];
    let size = buffer.len() as u32;
    if winapi::um::psapi::GetProcessImageFileNameW(handle, buffer.as_mut_ptr(), size) == 0 {
        return None;
    }
    let path = std::ffi::OsString::from_wide(&buffer[..]);
    let path = std::path::Path::new(&path).file_name()?.to_os_string();
    Some(
        path.to_string_lossy()
            .to_string()
            .trim_end_matches('\0')
            .to_string(),
    )
}

#[derive(Parser)]
struct Opts {
    /// Process names to enable locking the mouse for. Can specify multiple processes.
    ///
    /// Example:
    /// $ cursor-clamp "Wow.exe" "Gw2-64.exe"
    processes: Vec<OsString>,

    /// The button to lock the mouse with.
    ///
    /// 1: Left mouse button
    ///
    /// 2: Right mouse button (default)
    ///
    /// 3: Middle mouse button
    ///
    /// Example:
    /// $ cursor-clamp "Wow.exe" --button 1
    #[clap(short, long, default_value = "2")]
    button: i32,

    /// The interval in milliseconds to check the mouse button state.
    ///
    /// Example:
    /// $ cursor-clamp "Wow.exe" --interval 100
    #[clap(short, long, default_value = "1")]
    interval: u64,
}

#[tokio::main]
async fn main() {
    env_logger::builder()
        .parse_env(env_logger::Env::new().default_filter_or("info"))
        .init();

    let mut mouse_position = (0, 0);
    let mut mouse_pressed = false;
    let mut last_mouse_pressed = false;

    let opts: Opts = Opts::parse();

    let button = match opts.button {
        1 => LEFT_MOUSE_BUTTON,
        2 => RIGHT_MOUSE_BUTTON,
        3 => MIDDLE_MOUSE_BUTTON,
        _ => {
            log::error!("Invalid button value");
            return;
        }
    };

    let processes = opts.processes;

    let interval = Duration::from_millis(opts.interval);

    loop {
        let active_process = unsafe { get_active_process() };
        if let Some(active_process) = active_process {
            if processes.contains(&OsString::from(active_process)) {
                if unsafe { get_mouse_button_pressed(button) } {
                    if !mouse_pressed {
                        mouse_pressed = true;

                        if let Ok(new_mouse_position) = unsafe { get_mouse_position() } {
                            mouse_position = new_mouse_position;
                        }

                        log::info!("Remembering mouse position: {:?}", mouse_position);
                    }
                } else if mouse_pressed {
                    mouse_pressed = false;

                    if let Err(err) =
                        unsafe { set_mouse_position(mouse_position.0, mouse_position.1) }
                    {
                        log::error!("Failed to set mouse position: {}", err);
                    }
                    log::info!("Set mouse position: {:?}", mouse_position);
                }
            } else if mouse_pressed {
                mouse_pressed = false;

                if let Err(err) = unsafe { set_mouse_position(mouse_position.0, mouse_position.1) }
                {
                    log::error!("Failed to set mouse position: {}", err);
                }
                log::info!("Set mouse position: {:?}", mouse_position);
            }
        } else {
            log::error!("Failed to get active process");
        }

        if mouse_pressed != last_mouse_pressed {
            last_mouse_pressed = mouse_pressed;
        }

        tokio::time::sleep(interval).await;
    }
}
