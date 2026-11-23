// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! MSIX package identity: login start goes through the manifest's StartupTask, since Windows ignores HKCU Run for
//! packaged apps.

#[cfg(windows)]
mod imp {
    use std::sync::OnceLock;
    use windows::ApplicationModel::Activation::ActivationKind;
    use windows::ApplicationModel::{AppInstance, StartupTask, StartupTaskState};
    use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, WIN32_ERROR};
    use windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName;

    const TASK_ID: &str = "tokenwhy";

    pub fn packaged() -> bool {
        static P: OnceLock<bool> = OnceLock::new();
        *P.get_or_init(|| {
            let mut len = 0u32;
            let rc = unsafe { GetCurrentPackageFullName(&mut len, None) };
            rc == ERROR_INSUFFICIENT_BUFFER || rc == WIN32_ERROR(0)
        })
    }

    pub fn launched_by_startup_task() -> bool {
        packaged()
            && AppInstance::GetActivatedEventArgs()
                .and_then(|a| a.Kind())
                .is_ok_and(|k| k == ActivationKind::StartupTask)
    }

    fn task() -> windows::core::Result<StartupTask> {
        StartupTask::GetAsync(&TASK_ID.into())?.join()
    }

    pub fn enabled() -> Option<bool> {
        let s = task().and_then(|t| t.State()).ok()?;
        Some(s == StartupTaskState::Enabled || s == StartupTaskState::EnabledByPolicy)
    }

    pub fn set(on: bool) -> Option<bool> {
        let t = task().ok()?;
        if on {
            let s = t.RequestEnableAsync().ok()?.join().ok()?;
            Some(s == StartupTaskState::Enabled || s == StartupTaskState::EnabledByPolicy)
        } else {
            t.Disable().ok()?;
            enabled()
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn packaged() -> bool {
        false
    }
    pub fn launched_by_startup_task() -> bool {
        false
    }
    pub fn enabled() -> Option<bool> {
        None
    }
    pub fn set(_: bool) -> Option<bool> {
        None
    }
}

pub use imp::*;
