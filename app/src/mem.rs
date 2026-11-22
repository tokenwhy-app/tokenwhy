// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! Windows keeps the parse heap of a cold build committed after a burst; this gives it back.

#[cfg(windows)]
pub fn trim() {
    use std::ffi::c_void;
    #[link(name = "kernel32")]
    extern "system" {
        fn GetProcessHeap() -> *mut c_void;
        fn HeapCompact(heap: *mut c_void, flags: u32) -> usize;
        fn GetCurrentProcess() -> *mut c_void;
        fn SetProcessWorkingSetSize(process: *mut c_void, min: usize, max: usize) -> i32;
    }
    // SAFETY: both calls act on this process's own heap and working set; usize::MAX for min and max is the
    // documented "remove as many pages as possible" request.
    unsafe {
        HeapCompact(GetProcessHeap(), 0);
        SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
    }
}

#[cfg(not(windows))]
pub fn trim() {}
