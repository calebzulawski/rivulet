mod shm_open_anonymous;
use crate::buffer::error::SystemError;
use nix::{
    sys::mman::{mmap, munmap, MapFlags, ProtFlags},
    unistd::{ftruncate, sysconf, SysconfVar},
};
use once_cell::sync::OnceCell;

pub fn page_size() -> usize {
    static PAGE_SIZE: OnceCell<usize> = OnceCell::new();
    *PAGE_SIZE.get_or_init(|| sysconf(SysconfVar::PAGE_SIZE).unwrap().unwrap() as usize)
}

pub fn allocation_size() -> usize {
    page_size()
}

pub fn allocate_mirrored(size: usize) -> Result<*mut u8, SystemError> {
    if size == 0 {
        return Ok(std::ptr::null_mut());
    }

    let double_size = size.checked_mul(2).unwrap();

    assert!(size % allocation_size() == 0);

    // Create the shared memory file
    let fd = shm_open_anonymous::shm_open_anonymous()?;
    ftruncate(fd.as_fd(), double_size as i64)?;

    // Create the memory region
    let ptr = unsafe {
        mmap(
            std::ptr::null_mut(),
            double_size,
            ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
            MapFlags::MAP_SHARED,
            fd.as_fd(),
            0,
        )?
    };

    // Remap the mirrored region
    unsafe {
        mmap(
            ptr.add(size) as *mut _,
            size,
            ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
            MapFlags::MAP_SHARED | MapFlags::MAP_FIXED,
            fd.as_fd(),
            0,
        )?;
    }

    Ok(ptr as *mut u8)
}

pub unsafe fn deallocate_mirrored(address: *mut u8, size: usize) -> Result<(), SystemError> {
    if !address.is_null() {
        munmap(address as *mut _, 2 * size)?;
    }
    Ok(())
}
