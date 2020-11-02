//! Virtual memory utilities

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix as implementation;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as implementation;

/// Returns the system's page size.
pub fn page_size() -> usize {
    implementation::page_size()
}

/// Returns the system's allocation size.
pub fn allocation_size() -> usize {
    implementation::allocation_size()
}

/// Allocates a region of memory double `size`, such that the first `size` bytes are also
/// accessible via the second `size` bytes.
///
/// # Panics
/// Panics if `size` is not a multiple of [`allocation_size`](fn.allocation_size.html).
pub fn allocate_mirrored(size: usize) -> Result<*mut u8, crate::error::SystemError> {
    implementation::allocate_mirrored(size)
}

/// Deallocates a mirrored memory region.
///
/// # Safety
/// `address` must be a pointer returned by calling
/// [`allocate_mirrored`](fn.allocate_mirrored.html) with `size`.
pub unsafe fn deallocate_mirrored(
    address: *mut u8,
    size: usize,
) -> Result<(), crate::error::SystemError> {
    implementation::deallocate_mirrored(address, size)
}

#[cfg(test)]
mod test {
    use super::*;

    struct Mirrored(*mut u8, usize);

    impl Drop for Mirrored {
        fn drop(&mut self) {
            unsafe { deallocate_mirrored(self.0, self.1).unwrap() }
        }
    }

    impl Mirrored {
        fn new(size: usize) -> Self {
            Self(allocate_mirrored(size).unwrap(), size)
        }
    }

    #[test]
    fn zero_size() {
        let mirrored = Mirrored::new(0);
        assert!(mirrored.0.is_null());
    }

    #[test]
    fn assorted_sizes() {
        fn test_impl(size: usize) {
            let mirrored = Mirrored::new(size);
            assert!(!mirrored.0.is_null());
        }

        test_impl(allocation_size());
        test_impl(2 * allocation_size());
        test_impl(100 * allocation_size());
    }

    #[test]
    #[should_panic]
    fn invalid_size() {
        Mirrored::new(allocation_size() / 2);
    }
}
