//! The few Win32 calls the standard library does not offer.
//!
//! This is the only module of the crate that uses `unsafe` (Win32 FFI). Every
//! call passes NUL-terminated UTF-16 buffers that live until the call returns
//! and plain-data output buffers of the documented size.

#![allow(unsafe_code)]

use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows_sys::Win32::Globalization::GetUserDefaultUILanguage;
use windows_sys::Win32::Storage::CloudFilters::{
    CF_SYNC_ROOT_INFO_PROVIDER, CF_SYNC_ROOT_PROVIDER_INFO, CfGetSyncRootInfoByPath,
};
use windows_sys::Win32::Storage::FileSystem::{
    GetDriveTypeW, GetVolumePathNameW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    MoveFileExW,
};
use windows_sys::Win32::System::WindowsProgramming::DRIVE_REMOTE;
use windows_sys::Win32::UI::Shell::ShellExecuteW;

/// NUL-terminated UTF-16 form of `text`.
fn wide(text: &OsStr) -> Vec<u16> {
    text.encode_wide().chain(std::iter::once(0)).collect()
}

/// Absolute path with the `\\?\` prefix, so Win32 neither normalises names nor
/// applies the `MAX_PATH` limit. A path that is not verbatim yet is first made
/// absolute and normalised (`/` to `\\`, `.` and `..` resolved), because the
/// verbatim form takes every character literally. Works on UTF-16 units, so
/// no part of the name is lost in conversion.
fn verbatim(path: &Path) -> io::Result<Vec<u16>> {
    let verbatim_prefix: Vec<u16> = r"\\?\".encode_utf16().collect();
    let units: Vec<u16> = path.as_os_str().encode_wide().collect();
    if units.starts_with(&verbatim_prefix) {
        let mut out = units;
        out.push(0);
        return Ok(out);
    }
    let absolute = std::path::absolute(path)?;
    let units: Vec<u16> = absolute.as_os_str().encode_wide().collect();
    let unc_prefix: Vec<u16> = r"\\".encode_utf16().collect();
    let mut out = Vec::with_capacity(units.len() + 9);
    if let Some(rest) = units.strip_prefix(unc_prefix.as_slice()) {
        out.extend(r"\\?\UNC\".encode_utf16());
        out.extend_from_slice(rest);
    } else {
        out.extend_from_slice(&verbatim_prefix);
        out.extend_from_slice(&units);
    }
    out.push(0);
    Ok(out)
}

/// Replaces `to` by `from` atomically; returns only after the change reached
/// the disk (`MOVEFILE_WRITE_THROUGH`).
pub fn replace_file_write_through(from: &Path, to: &Path) -> io::Result<()> {
    let from = verbatim(from)?;
    let to = verbatim(to)?;
    // SAFETY: `from` and `to` are NUL-terminated UTF-16 strings that outlive
    // the call; MoveFileExW does not keep the pointers.
    let ok = unsafe {
        MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Whether `path` (an existing path) lies on a network drive: a mapped drive
/// letter or a UNC share.
pub fn is_network_path(path: &Path) -> bool {
    let text = path.to_string_lossy();
    if text.starts_with(r"\\?\UNC\") || (text.starts_with(r"\\") && !text.starts_with(r"\\?\")) {
        return true;
    }
    let input = wide(path.as_os_str());
    let mut volume = vec![0u16; 32_768];
    // SAFETY: `input` is NUL-terminated; `volume` is a writable buffer whose
    // length in UTF-16 units is passed as the capacity.
    let ok = unsafe {
        GetVolumePathNameW(
            input.as_ptr(),
            volume.as_mut_ptr(),
            u32::try_from(volume.len()).unwrap_or(u32::MAX),
        )
    };
    if ok == 0 {
        return false;
    }
    // SAFETY: on success GetVolumePathNameW wrote a NUL-terminated string.
    let drive_type = unsafe { GetDriveTypeW(volume.as_ptr()) };
    drive_type == DRIVE_REMOTE
}

/// Name of the cloud sync provider whose sync root contains `path` (OneDrive,
/// Dropbox, iCloud Drive, Google Drive … register their folders with the
/// Windows Cloud Files API). `None` if `path` is not inside a sync root.
pub fn cloud_sync_provider(path: &Path) -> Option<String> {
    let input = wide(path.as_os_str());
    // SAFETY: CF_SYNC_ROOT_PROVIDER_INFO is plain data; all-zero is valid.
    let mut info: CF_SYNC_ROOT_PROVIDER_INFO = unsafe { std::mem::zeroed() };
    let mut returned = 0u32;
    // SAFETY: `input` is NUL-terminated; `info` is a writable buffer of the
    // size passed; `returned` is a valid out pointer.
    let result = unsafe {
        CfGetSyncRootInfoByPath(
            input.as_ptr(),
            CF_SYNC_ROOT_INFO_PROVIDER,
            (&raw mut info).cast(),
            u32::try_from(std::mem::size_of::<CF_SYNC_ROOT_PROVIDER_INFO>()).ok()?,
            &raw mut returned,
        )
    };
    if result < 0 {
        return None;
    }
    let name = &info.ProviderName;
    let len = name.iter().position(|&c| c == 0).unwrap_or(name.len());
    let provider = String::from_utf16_lossy(&name[..len]).trim().to_owned();
    Some(if provider.is_empty() {
        "Cloud".to_owned()
    } else {
        provider
    })
}

/// Primary language of the Windows display language (`LANG_GERMAN` is
/// `0x07`), for the interface language "System" (section 11).
pub fn user_ui_primary_language() -> u16 {
    // SAFETY: no arguments, returns a LANGID.
    let language_id = unsafe { GetUserDefaultUILanguage() };
    language_id & 0x3ff
}

/// Hands `target` (a checked web or mail address) to the program Windows
/// opens it with, as one argument: no shell, no parameters, no directory.
pub fn shell_open(target: &str) -> io::Result<()> {
    /// `SW_SHOWNORMAL` (WinUser.h).
    const SHOW_NORMAL: i32 = 1;
    let operation = wide(OsStr::new("open"));
    let target = wide(OsStr::new(target));
    // SAFETY: both strings are NUL-terminated UTF-16 that live until the
    // call returns; no window handle, parameters or directory are passed.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SHOW_NORMAL,
        )
    };
    // ShellExecuteW reports success with a value greater than 32.
    let code = result as isize;
    if code > 32 {
        Ok(())
    } else {
        Err(io::Error::other(format!("ShellExecuteW returned {code}")))
    }
}
