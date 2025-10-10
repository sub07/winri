mod window;

use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    sync::mpsc,
    thread,
    time::Duration,
};

use bimap::BiMap;
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM},
        UI::{
            Accessibility::{HWINEVENTHOOK, SetWinEventHook},
            WindowsAndMessaging::{
                DispatchMessageA, EVENT_OBJECT_CREATE, EVENT_OBJECT_DESTROY,
                EVENT_SYSTEM_FOREGROUND, EnumWindows, GetMessageA, GetSystemMetrics, MSG,
                SM_CXSCREEN, SM_CYSCREEN, TranslateMessage, WINEVENT_OUTOFCONTEXT,
                WINEVENT_SKIPOWNPROCESS,
            },
        },
    },
    core::BOOL,
};

pub fn managed_windows() -> anyhow::Result<HashSet<Window>> {
    unsafe extern "system" fn enum_callback(window: HWND, out_list: LPARAM) -> BOOL {
        let list = unsafe { &mut *(out_list.0 as *mut Vec<HWND>) };
        list.push(window);
        true.into() // Continue enumeration
    }

    let mut result = Vec::new();

    unsafe {
        EnumWindows(Some(enum_callback), LPARAM(&raw mut result as isize))?;
    }

    let windows = result
        .into_iter()
        .filter_map(|hwnd| Window::from(hwnd).ok())
        .filter(|window| {
            is_managed_window(*window)
                .inspect_err(|err| {
                    eprintln!(
                        "Error checking managed window {:?} ({:?}): {}",
                        window.title(),
                        window.class(),
                        err
                    );
                })
                .unwrap_or(false)
        })
        .collect::<HashSet<_>>();

    Ok(windows)
}

use crate::window::{Window, filter::is_managed_window};

unsafe extern "system" fn hook_callback(
    _hwineventhook: HWINEVENTHOOK,
    event: u32,
    _hwnd: HWND,
    _idobject: i32,
    _idchild: i32,
    _ideventthread: u32,
    _dwmseventtime: u32,
) {
    fn handle_window() -> anyhow::Result<()> {
        println!("---------------");
        for (index, window) in managed_windows()?.iter().enumerate() {
            println!(
                "{} - {} - {}",
                window.title()?.as_deref().unwrap_or("No title"),
                window.class()?,
                window.process_name()?
            );
            // let (padding_x, padding_y) = window.padding()?;
            // window.move_window(
            //     index as i32 * 600 - padding_x,
            //     -padding_y,
            //     600 + padding_x,
            //     600 + padding_y,
            // )?;
        }
        thread::sleep(Duration::from_millis(2000));
        Ok(())
    }

    if event == EVENT_OBJECT_CREATE || event == EVENT_OBJECT_DESTROY {
        if let Err(err) = handle_window() {
            eprintln!("{err}");
        }
    }
}

fn main() -> anyhow::Result<()> {
    // let w = Window::focused()?;
    // w.move_window(-100, 50, 1000, 1000)?;

    // SetWinEventHook(
    //     EVENT_OBJECT_CREATE,
    //     EVENT_OBJECT_DESTROY,
    //     None,
    //     Some(hook_callback),
    //     0,
    //     0,
    //     WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
    // );
    //
    //
    // let managed_windows = managed_windows()?;

    setup_horizontal_tiling()?;
    //
    Ok(())
}

fn setup_horizontal_tiling() -> anyhow::Result<()> {
    // let (enum_trigger_tx, enum_trigger_rx) = mpsc::channel();

    // thread::spawn(move || {
    //     loop {
    //         enum_trigger_tx.send(()).unwrap();
    //         thread::sleep(Duration::from_millis(1000));
    //     }
    // });

    let (screen_width, screen_height) =
        unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) };

    let mut window_indices = BiMap::<Window, usize>::new();

    loop {
        // enum_trigger_rx.recv().unwrap();
        thread::sleep(Duration::from_millis(1000));
        let managed_windows = managed_windows()?;
        if managed_windows.is_empty() {
            window_indices.clear();
            continue;
        }
        let mut window_to_remove = Vec::new();
        // remove windows that are no longer managed
        for window in window_indices.left_values() {
            if !managed_windows.contains(window) {
                window_to_remove.push(*window);
            }
        }

        if window_to_remove.is_empty() && (managed_windows.len() == window_indices.len()) {
            continue;
        }

        for window in window_to_remove {
            window_indices.remove_by_left(&window);
        }

        // packing remaining windows
        let last_index = window_indices.right_values().max().unwrap_or(&0);
        let mut empty_index = 0;

        for index in 0..=*last_index {
            if index == empty_index {
                if window_indices.contains_right(&index) {
                    empty_index += 1;
                }
            } else if let Some((window, _)) = window_indices.remove_by_right(&index) {
                window_indices.insert(window, empty_index);
                empty_index += 1;
            }
        }

        // add new windows
        let mut next_index = empty_index;
        for window in &managed_windows {
            if !window_indices.contains_left(window) {
                window_indices.insert(*window, next_index);
                next_index += 1;
            }
        }

        // layout windows
        let height = screen_height;
        let width = screen_width / managed_windows.len() as i32;

        println!("--------------------");
        let last_index = next_index - 1;
        for index in 0..=last_index {
            let window = window_indices.get_by_right(&index).expect("invalid state");
            let x = index as i32 * width;
            let y = 0;
            let w = width;
            let h = height;
            println!(
                "Window #{}({}) at ({}, {}, {}, {})",
                index,
                window.process_name()?,
                x,
                y,
                w,
                h,
            );

            window.move_window(x, y, w, h)?;
        }
    }
}
