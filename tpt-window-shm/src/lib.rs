// SPDX-License-Identifier: MIT OR Apache-2.0
//! Shared memory for the tpt-window display protocol.
//!
//! `SharedMemory` is a named file mapping that implements
//! `tpt_window_core::channel::RingMemory`, so a
//! `FrameRing` (in `tpt_window_core::channel`) can run over it. The two sides open
//! the same name, one creating it and the other opening it.
//!
//! This crate holds the unsafe code for the transport, and core stays safe. Windows only.
//! Dropping a [`SharedMemory`] unmaps its view and closes its handle.
//!
//! The creating process makes the mapping, and the other process opens it by the same name.
//! A bare name is kept in the session's `Local\` namespace:
//!
//! ```no_run
//! use tpt_window_core::channel::FrameRing;
//! use tpt_window_shm::SharedMemory;
//!
//! // Process A: create the ring and reset it once.
//! let mut ring = FrameRing::new(SharedMemory::create("tpt-display-0", 64 * 1024)?)?;
//! ring.reset();
//!
//! // Process B: open the same name with the same size.
//! let peer = FrameRing::new(SharedMemory::open("tpt-display-0", 64 * 1024)?)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
#![cfg(windows)]
#![warn(missing_docs)]

use std::sync::atomic::{AtomicU32, Ordering};

use tpt_window_core::channel::RingMemory;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Memory::{
    CreateFileMappingW, FILE_MAP_ALL_ACCESS, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile,
    OpenFileMappingW, PAGE_READWRITE, UnmapViewOfFile,
};

/// Windows error code: a mapping with this name already exists.
const ERROR_ALREADY_EXISTS: i32 = 183;
/// Windows error code: the file was not found (used for a missing mapping name).
const ERROR_FILE_NOT_FOUND: i32 = 2;

/// Why a mapping could not be created or opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShmError {
    /// The name is empty or contains a NUL character.
    BadName,
    /// `create` found a mapping with this name already.
    AlreadyExists,
    /// `open` found no mapping with this name.
    NotFound,
    /// The OS call failed with this error code.
    Os(i32),
}

impl core::fmt::Display for ShmError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ShmError::BadName => f.write_str("invalid shared-memory name"),
            ShmError::AlreadyExists => f.write_str("shared memory with this name exists"),
            ShmError::NotFound => f.write_str("no shared memory with this name"),
            ShmError::Os(code) => write!(f, "OS error {code}"),
        }
    }
}

impl std::error::Error for ShmError {}

/// A mapped view of a named file mapping.
pub struct SharedMemory {
    handle: HANDLE,
    view: *mut u8,
    len: usize,
}

// SAFETY: the view is a plain process-wide mapping, and this type only reaches it through
// the `RingMemory` methods, which use atomics for the indices. Moving the value to another
// thread does not change what the mapping means.
unsafe impl Send for SharedMemory {}

/// Converts a mapping name to the wide, NUL-terminated form the OS wants. A bare name is put
/// in the `Local\` namespace, so it is session-private and never lands in the machine-wide
/// `Global\` namespace, where a service could create a mapping that any session can reach.
fn wide(name: &str) -> Result<Vec<u16>, ShmError> {
    if name.is_empty() || name.contains('\u{0}') {
        return Err(ShmError::BadName);
    }
    let full = if name.contains('\\') {
        name.to_owned()
    } else {
        format!(r"Local\{name}")
    };
    Ok(full.encode_utf16().chain(Some(0)).collect())
}

fn last_error() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

impl SharedMemory {
    /// Creates a mapping of `len` bytes named `name`. Fails if the name is taken.
    pub fn create(name: &str, len: usize) -> Result<Self, ShmError> {
        let name = wide(name)?;
        let (high, low) = ((len as u64 >> 32) as u32, len as u32);
        // SAFETY: a page-file-backed mapping. The name is a NUL-terminated buffer that
        // outlives the call.
        let handle = unsafe {
            CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                std::ptr::null(),
                PAGE_READWRITE,
                high,
                low,
                name.as_ptr(),
            )
        };
        if handle.is_null() {
            return Err(ShmError::Os(last_error()));
        }
        if last_error() == ERROR_ALREADY_EXISTS {
            // SAFETY: `handle` was returned by CreateFileMappingW above and is closed here only.
            unsafe { CloseHandle(handle) };
            return Err(ShmError::AlreadyExists);
        }
        Self::map(handle, len)
    }

    /// Opens an existing mapping named `name`. `len` must not be larger than the size the
    /// mapping was created with: for a page-file mapping the OS refuses a view past the end,
    /// and that refusal comes back as [`ShmError::Os`].
    pub fn open(name: &str, len: usize) -> Result<Self, ShmError> {
        let name = wide(name)?;
        // SAFETY: the name is a NUL-terminated buffer that outlives the call.
        let handle = unsafe { OpenFileMappingW(FILE_MAP_ALL_ACCESS, 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(match last_error() {
                ERROR_FILE_NOT_FOUND => ShmError::NotFound,
                code => ShmError::Os(code),
            });
        }
        Self::map(handle, len)
    }

    fn map(handle: HANDLE, len: usize) -> Result<Self, ShmError> {
        // SAFETY: `handle` is a valid file-mapping handle owned by this call.
        let view = unsafe { MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, len) };
        if view.Value.is_null() {
            let err = last_error();
            // SAFETY: `handle` is ours and is not used after this.
            unsafe { CloseHandle(handle) };
            return Err(ShmError::Os(err));
        }
        Ok(Self {
            handle,
            view: view.Value as *mut u8,
            len,
        })
    }

    fn check(&self, offset: usize, size: usize) {
        assert!(
            offset.checked_add(size).is_some_and(|end| end <= self.len),
            "shared-memory access out of range"
        );
    }

    fn cell(&self, offset: usize) -> *const AtomicU32 {
        self.check(offset, 4);
        assert!(
            offset % 4 == 0,
            "shared-memory cells must be 4-byte aligned"
        );
        // SAFETY: in range (checked), and the view is page-aligned, so the cell is aligned.
        unsafe { self.view.add(offset) as *const AtomicU32 }
    }
}

impl Drop for SharedMemory {
    fn drop(&mut self) {
        // SAFETY: `view` and `handle` came from MapViewOfFile and CreateFileMappingW /
        // OpenFileMappingW in this value, and nothing uses them after this.
        unsafe {
            UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                Value: self.view as *mut _,
            });
            CloseHandle(self.handle);
        }
    }
}

impl RingMemory for SharedMemory {
    fn len(&self) -> usize {
        self.len
    }

    fn read(&self, offset: usize, out: &mut [u8]) {
        self.check(offset, out.len());
        // The other side is another process that can write at any time. Volatile byte reads
        // stop the compiler from merging or re-reading these bytes. The ring copies each frame
        // out once and validates that copy, so a later write cannot change what is decoded.
        for (i, byte) in out.iter_mut().enumerate() {
            // SAFETY: in range (checked above). The ring reads only bytes the producer has
            // published with a release store, so no byte is read while it is being written.
            *byte = unsafe { self.view.add(offset + i).read_volatile() };
        }
    }

    fn write(&mut self, offset: usize, data: &[u8]) {
        self.check(offset, data.len());
        for (i, &byte) in data.iter().enumerate() {
            // SAFETY: in range (checked). The consumer never reads bytes that have not been
            // published, so writing free space does not race with a read.
            unsafe { self.view.add(offset + i).write_volatile(byte) };
        }
    }

    fn load_u32(&self, offset: usize) -> u32 {
        // SAFETY: `cell` checks range and alignment, and the mapping lives as long as `self`.
        unsafe { (*self.cell(offset)).load(Ordering::Acquire) }
    }

    fn store_u32(&mut self, offset: usize, value: u32) {
        // SAFETY: as in `load_u32`.
        unsafe { (*self.cell(offset)).store(value, Ordering::Release) }
    }
}
