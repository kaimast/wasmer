// This file contains code from external sources.
// Attributions: https://github.com/wasmerio/wasmer/blob/master/ATTRIBUTIONS.md

//! Low-level abstraction for allocating and managing zero-filled pages
//! of memory.

use more_asserts::assert_le;
use more_asserts::assert_lt;
use std::io;
use std::ptr;
use std::slice;

use std::os::unix::io::RawFd;

/// Round `size` up to the nearest multiple of `page_size`.
fn round_up_to_page_size(size: usize, page_size: usize) -> usize {
    (size + (page_size - 1)) & !(page_size - 1)
}

/// A simple struct consisting of a page-aligned pointer to page-aligned
/// and initially-zeroed memory and a length.
#[derive(Debug)]
pub struct Mmap {
    // Note that this is stored as a `usize` instead of a `*const` or `*mut`
    // pointer to allow this structure to be natively `Send` and `Sync` without
    // `unsafe impl`. This type is sendable across threads and shareable since
    // the coordination all happens at the OS layer.
    ptr: usize,
    len: usize,
    memfd: Option<RawFd>,
}

impl Mmap {
    /// Construct a new empty instance of `Mmap`.
    pub fn new() -> Self {
        // Rust's slices require non-null pointers, even when empty. `Vec`
        // contains code to create a non-null dangling pointer value when
        // constructed empty, so we reuse that here.
        let empty = Vec::<u8>::new();
        Self {
            ptr: empty.as_ptr() as usize,
            len: 0,
            memfd: None,
        }
    }

    /// Create a new `Mmap` pointing to at least `size` bytes of page-aligned accessible memory.
    pub fn with_at_least(size: usize) -> Result<Self, String> {
        let page_size = region::page::size();
        let rounded_size = round_up_to_page_size(size, page_size);
        Self::accessible_reserved(rounded_size, rounded_size, None)
    }

    /// Create a new `Mmap` pointing to `accessible_size` bytes of page-aligned accessible memory,
    /// within a reserved mapping of `mapping_size` bytes. `accessible_size` and `mapping_size`
    /// must be native page-size multiples.
    #[cfg(not(target_os = "windows"))]
    pub fn accessible_reserved(
        accessible_size: usize,
        mapping_size: usize,
        memfd: Option<RawFd>,
    ) -> Result<Self, String> {
        let page_size = region::page::size();
        assert_le!(accessible_size, mapping_size);
        assert_eq!(mapping_size & (page_size - 1), 0);
        assert_eq!(accessible_size & (page_size - 1), 0);

        // Mmap may return EINVAL if the size is zero, so just
        // special-case that.
        if mapping_size == 0 {
            return Ok(Self::new());
        }

        let memfd = if let Some(memfd) = memfd {
            memfd
        } else {
            let label = std::ffi::CString::new("wasmer").unwrap();
            let flags = nix::sys::memfd::MemFdCreateFlag::empty();
            nix::sys::memfd::memfd_create(&label, flags)
                    .expect("Failed to create memfd")
        };

        nix::unistd::ftruncate(memfd, mapping_size as i64).expect("Failed to resize memfd");
        log::trace!("Created memfd {} of size {}", memfd, mapping_size);

        Ok(if accessible_size == mapping_size {
            // Allocate a single read-write region at once.
            let ptr = unsafe {
                libc::mmap(
                    ptr::null_mut(),
                    mapping_size,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_SHARED,
                    memfd,
                    0,
                )
            };

            let ptr = ptr as usize;
            log::trace!("Memory mapped {} bytes to {:#X}-{:#X}", mapping_size, ptr, ptr+mapping_size);

            if ptr as isize == -1_isize {
                return Err(io::Error::last_os_error().to_string());
            }

            Self {
                ptr, len: mapping_size,
                memfd: Some(memfd),
            }
        } else {
            // Reserve the mapping size.
            let ptr = unsafe {
                libc::mmap(
                    ptr::null_mut(),
                    mapping_size,
                    libc::PROT_READ | libc::PROT_WRITE,
                    //FIXME libc::PROT_NONE,
                    libc::MAP_SHARED,
                    memfd,
                    0,
                )
            };

            let ptr = ptr as usize;
            log::trace!("Memory mapped {} bytes to {:#X}-{:#X}", mapping_size, ptr, ptr+mapping_size);

            if ptr as isize == -1_isize {
                return Err(io::Error::last_os_error().to_string());
            }

            let mut result = Self {
                ptr, len: mapping_size,
                memfd: Some(memfd),
            };

            if accessible_size != 0 {
                // Commit the accessible size.
                result.make_accessible(0, accessible_size)?;
            }

            result
        })
    }

    /// Can this Mmap be duplicated?
    pub fn is_zygote(&self) -> bool {
        self.memfd.is_some()
    }

    /// Destroy this mmap and return the underlying memfd (if any)
    pub fn into_memfd(mut self) -> Option<RawFd> {
        self.memfd.take()
    }

    /// Create a new `Mmap` pointing to `accessible_size` bytes of page-aligned accessible memory,
    /// within a reserved mapping of `mapping_size` bytes. `accessible_size` and `mapping_size`
    /// must be native page-size multiples.
    #[cfg(target_os = "windows")]
    pub fn accessible_reserved(
        accessible_size: usize,
        mapping_size: usize,
    ) -> Result<Self, String> {
        use winapi::um::memoryapi::VirtualAlloc;
        use winapi::um::winnt::{MEM_COMMIT, MEM_RESERVE, PAGE_NOACCESS, PAGE_READWRITE};

        let page_size = region::page::size();
        assert_le!(accessible_size, mapping_size);
        assert_eq!(mapping_size & (page_size - 1), 0);
        assert_eq!(accessible_size & (page_size - 1), 0);

        // VirtualAlloc may return ERROR_INVALID_PARAMETER if the size is zero,
        // so just special-case that.
        if mapping_size == 0 {
            return Ok(Self::new());
        }

        Ok(if accessible_size == mapping_size {
            // Allocate a single read-write region at once.
            let ptr = unsafe {
                VirtualAlloc(
                    ptr::null_mut(),
                    mapping_size,
                    MEM_RESERVE | MEM_COMMIT,
                    PAGE_READWRITE,
                )
            };
            if ptr.is_null() {
                return Err(io::Error::last_os_error().to_string());
            }

            let ptr = ptr as usize;
            log::trace!("Memory mapped {} bytes to {:#X}-{:#X}", mapping_size, ptr, ptr+mapping_size);

            Self {
                ptr, len: mapping_size,
            }
        } else {
            // Reserve the mapping size.
            let ptr =
                unsafe { VirtualAlloc(ptr::null_mut(), mapping_size, MEM_RESERVE, PAGE_NOACCESS) };
            if ptr.is_null() {
                return Err(io::Error::last_os_error().to_string());
            }

            let ptr = ptr as usize;
            log::trace!("Memory mapped {} bytes to {:#X}-{:#X}", mapping_size, ptr, ptr+mapping_size);

            let mut result = Self {
                ptr, len: mapping_size,
            };

            if accessible_size != 0 {
                // Commit the accessible size.
                result.make_accessible(0, accessible_size)?;
            }

            result
        })
    }

    /// Make the memory starting at `start` and extending for `len` bytes accessible.
    /// `start` and `len` must be native page-size multiples and describe a range within
    /// `self`'s reserved memory.
    #[cfg(not(target_os = "windows"))]
    pub fn make_accessible(&mut self, start: usize, len: usize) -> Result<(), String> {
        let page_size = region::page::size();
        assert_eq!(start & (page_size - 1), 0);
        assert_eq!(len & (page_size - 1), 0);
        assert_lt!(len, self.len);
        assert_lt!(start, self.len - len);

        // Commit the accessible size.
        let ptr = self.ptr as *const u8;
        unsafe { region::protect(ptr.add(start), len, region::Protection::READ_WRITE) }
            .map_err(|e| e.to_string())
    }

    /// Make the memory starting at `start` and extending for `len` bytes accessible.
    /// `start` and `len` must be native page-size multiples and describe a range within
    /// `self`'s reserved memory.
    #[cfg(target_os = "windows")]
    pub fn make_accessible(&mut self, start: usize, len: usize) -> Result<(), String> {
        use winapi::ctypes::c_void;
        use winapi::um::memoryapi::VirtualAlloc;
        use winapi::um::winnt::{MEM_COMMIT, PAGE_READWRITE};
        let page_size = region::page::size();
        assert_eq!(start & (page_size - 1), 0);
        assert_eq!(len & (page_size - 1), 0);
        assert_lt!(len, self.len);
        assert_lt!(start, self.len - len);

        // Commit the accessible size.
        let ptr = self.ptr as *const u8;
        if unsafe {
            VirtualAlloc(
                ptr.add(start) as *mut c_void,
                len,
                MEM_COMMIT,
                PAGE_READWRITE,
            )
        }
        .is_null()
        {
            return Err(io::Error::last_os_error().to_string());
        }

        Ok(())
    }

    /// Return the allocated memory as a slice of u8.
    pub fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr as *const u8, self.len) }
    }

    /// Return the allocated memory as a mutable slice of u8.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.ptr as *mut u8, self.len) }
    }

    /// Return the allocated memory as a pointer to u8.
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr as *const u8
    }

    /// Return the allocated memory as a mutable pointer to u8.
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr as *mut u8
    }

    /// Return the length of the allocated memory.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether any memory has been allocated.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Create an identical (COW) copy of this memory-mapped region
    pub fn duplicate(&self) -> Result<Self, String> {
        let memfd = if let Some(memfd) = self.memfd {
            memfd
        } else {
            return Err(String::from("Not a Zygote"));
        };

        let ptr = unsafe{ libc::mmap(
            ptr::null_mut(),
            self.len,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE,
            memfd,
            0,
        )} as usize;

        if ptr as isize == -1_isize {
            return Err(io::Error::last_os_error().to_string());
        }

        log::trace!("Duplicated memory from {:#X} to {:#X} ({} bytes)", self.ptr, ptr, self.len);

        Ok(Self {
            ptr, len: self.len,
            memfd: None,
        })
    }
}

impl Drop for Mmap {
    #[cfg(not(target_os = "windows"))]
    fn drop(&mut self) {
        if self.len != 0 {
            log::trace!("Memory unmapping {} bytes at {:#X}-{:#X}", self.len, self.ptr, self.ptr+self.len);

            let r = unsafe { libc::munmap(self.ptr as *mut libc::c_void, self.len) };
            assert_eq!(r, 0, "munmap failed: {}", io::Error::last_os_error());

            if let Some(memfd) = self.memfd {
                log::trace!("Closing memfd {}", memfd);
                nix::unistd::close(memfd).unwrap();
            }
        }
    }

    #[cfg(target_os = "windows")]
    fn drop(&mut self) {
        if self.len != 0 {
            log::trace!("Memory unmapping {} bytes at {:#X}-{:#X}", self.len, self.ptr, self.ptr+self.len);

            use winapi::ctypes::c_void;
            use winapi::um::memoryapi::VirtualFree;
            use winapi::um::winnt::MEM_RELEASE;
            let r = unsafe { VirtualFree(self.ptr as *mut c_void, 0, MEM_RELEASE) };
            assert_ne!(r, 0);
        }
    }
}

fn _assert() {
    fn _assert_send_sync<T: Send + Sync>() {}
    _assert_send_sync::<Mmap>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_up_to_page_size() {
        assert_eq!(round_up_to_page_size(0, 4096), 0);
        assert_eq!(round_up_to_page_size(1, 4096), 4096);
        assert_eq!(round_up_to_page_size(4096, 4096), 4096);
        assert_eq!(round_up_to_page_size(4097, 4096), 8192);
    }
}
