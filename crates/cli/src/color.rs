// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

use crate::args::ColorMode;
use std::io::IsTerminal;

pub fn enabled(mode: ColorMode) -> bool {
    match mode {
        ColorMode::Never => false,
        ColorMode::Always => {
            enable_vt();
            true
        }
        ColorMode::Auto => {
            let on = std::io::stdout().is_terminal()
                && std::env::var_os("NO_COLOR").is_none_or(|v| v.is_empty())
                && enable_vt();
            on
        }
    }
}

#[cfg(windows)]
fn enable_vt() -> bool {
    use std::ffi::c_void;
    extern "system" {
        fn GetStdHandle(n: u32) -> *mut c_void;
        fn GetConsoleMode(h: *mut c_void, mode: *mut u32) -> i32;
        fn SetConsoleMode(h: *mut c_void, mode: u32) -> i32;
    }
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    const ENABLE_VT: u32 = 0x0004;
    // SAFETY: plain kernel32 calls on the process's own stdout handle with a valid out pointer.
    unsafe {
        let h = GetStdHandle(STD_OUTPUT_HANDLE);
        let mut mode = 0u32;
        if h.is_null() || GetConsoleMode(h, &mut mode) == 0 {
            return true;
        }
        mode & ENABLE_VT != 0 || SetConsoleMode(h, mode | ENABLE_VT) != 0
    }
}

#[cfg(not(windows))]
fn enable_vt() -> bool {
    true
}
