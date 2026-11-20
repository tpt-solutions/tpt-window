// SPDX-License-Identifier: MIT OR Apache-2.0
//! Shared memory for the display protocol on Linux, built on `memfd_create`.
//!
//! `MemfdMemory` implements `tpt_window_core::channel::RingMemory`, so a `FrameRing` runs over
//! it. Two processes share it by passing the file descriptor (for example over a Unix socket with
//! `SCM_RIGHTS`); each maps the same bytes. The unsafe code lives in this crate, as the Windows
//! equivalent does in `tpt-window-shm`, and core stays safe.
//!
//! Linux only. The tests run in CI; they need a real kernel, so they do not run on other systems.
#![cfg(target_os = "linux")]
#![warn(missing_docs)]

use std::io;
use std::os::fd::{AsFd, BorrowedFd, FromRawFd, OwnedFd};
use std::sync::atomic::{AtomicU32, Ordering};

use tpt_window_core::channel::RingMemory;

/// A shared memory region backed by a `memfd`, mapped into this process.
pub struct MemfdMemory {
    fd: OwnedFd,
    ptr: *mut u8,
    len: usize,
}

// SAFETY: the mapping is plain shared memory. This type reaches it only through `RingMemory`,
// whose index cells are atomics, and moving the value to another thread does not change what
// the mapping means.
unsafe impl Send for MemfdMemory {}

impl MemfdMemory {
    /// Creates a region of `len` bytes. `name` labels it in `/proc` and has no other meaning.
    pub fn create(name: &str, len: usize) -> io::Result<Self> {
        let c_name = std::ffi::CString::new(name)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "name contains NUL"))?;
        // SAFETY: `c_name` is NUL-terminated and lives across the call.
        let raw = unsafe { libc::memfd_create(c_name.as_ptr(), libc::MFD_CLOEXEC) };
        if raw < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `raw` is a fresh descriptor that nothing else owns.
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        let size = libc::off_t::try_from(len)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "region too large"))?;
        // SAFETY: `fd` is valid for the duration of the call.
        if unsafe { libc::ftruncate(fd_raw(&fd), size) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Self::map(fd, len)
    }

    /// Maps a region another process created, from its descriptor. The region's size is taken
    /// from the descriptor.
    pub fn open(fd: OwnedFd) -> io::Result<Self> {
        // SAFETY: `stat` is written by fstat before it is read.
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        // SAFETY: `fd` is valid and `stat` is a valid out-pointer.
        if unsafe { libc::fstat(fd_raw(&fd), &mut stat) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let len = usize::try_from(stat.st_size)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "negative size"))?;
        Self::map(fd, len)
    }

    /// The descriptor, to pass to another process. The region stays mapped here.
    pub fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }

    fn map(fd: OwnedFd, len: usize) -> io::Result<Self> {
        // SAFETY: a shared read-write mapping of `len` bytes of `fd` at offset 0. A zero-length
        // mapping is refused by the kernel, which the caller sees as an error.
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd_raw(&fd),
                0,
            )
        };
        if ptr == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            fd,
            ptr: ptr.cast::<u8>(),
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
        // SAFETY: in range (checked above), and mmap returns a page-aligned base, so the cell is
        // aligned for AtomicU32.
        unsafe { self.ptr.add(offset).cast::<AtomicU32>() }
    }
}

fn fd_raw(fd: &OwnedFd) -> std::os::fd::RawFd {
    std::os::fd::AsRawFd::as_raw_fd(fd)
}

impl Drop for MemfdMemory {
    fn drop(&mut self) {
        // SAFETY: `ptr` and `len` came from the mmap in `map`, and nothing uses the mapping after
        // this. The descriptor is closed by `OwnedFd` afterwards.
        unsafe { libc::munmap(self.ptr.cast(), self.len) };
    }
}

impl RingMemory for MemfdMemory {
    fn len(&self) -> usize {
        self.len
    }

    fn read(&self, offset: usize, out: &mut [u8]) {
        self.check(offset, out.len());
        for (i, byte) in out.iter_mut().enumerate() {
            // SAFETY: in range (checked). The other process writes only bytes it has not yet
            // published, so reading published bytes is not a race. Volatile reads keep the
            // compiler from merging them.
            *byte = unsafe { self.ptr.add(offset + i).read_volatile() };
        }
    }

    fn write(&mut self, offset: usize, data: &[u8]) {
        self.check(offset, data.len());
        for (i, &byte) in data.iter().enumerate() {
            // SAFETY: in range (checked). The consumer never reads bytes that have not been
            // published, so writing free space does not race with a read.
            unsafe { self.ptr.add(offset + i).write_volatile(byte) };
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

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_window_core::channel::FrameRing;
    use tpt_window_core::protocol::Message;

    /// Two mappings of one region, as two processes would have.
    fn pair() -> (MemfdMemory, MemfdMemory) {
        let producer = MemfdMemory::create("tpt-window-test", 16 * 1024).expect("create");
        let consumer_fd = producer.as_fd().try_clone_to_owned().expect("dup");
        let consumer = MemfdMemory::open(consumer_fd).expect("open");
        (producer, consumer)
    }

    #[test]
    fn frames_cross_between_two_mappings_in_order() {
        let (producer, consumer) = pair();
        let mut tx = FrameRing::new(producer).expect("ring");
        tx.reset();
        let mut rx = FrameRing::new(consumer).expect("ring");
        for version in 0..2_000_u32 {
            tx.push(&Message::Hello { version }).expect("room");
            assert_eq!(rx.pop().unwrap(), Some(Message::Hello { version }));
        }
    }

    #[test]
    fn a_too_small_region_is_refused_by_the_ring() {
        let small = MemfdMemory::create("tpt-window-small", 64).expect("create");
        assert!(FrameRing::new(small).is_err());
    }
}
