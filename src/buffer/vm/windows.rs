use crate::{error::windows::Error, SystemError};
use once_cell::sync::OnceCell;
use std::convert::TryInto;
use winapi::{
    ctypes::c_void,
    shared::minwindef::DWORD,
    um::{
        handleapi::CloseHandle,
        handleapi::INVALID_HANDLE_VALUE,
        memoryapi::{
            MapViewOfFileEx, UnmapViewOfFile, VirtualAlloc, VirtualFree, FILE_MAP_ALL_ACCESS,
        },
        sysinfoapi::{GetSystemInfo, SYSTEM_INFO},
        winbase::CreateFileMappingA,
        winnt::{HANDLE, MEM_RELEASE, MEM_RESERVE, PAGE_NOACCESS, PAGE_READWRITE, SEC_COMMIT},
    },
};

fn system_info() -> (usize, usize) {
    static INFO: OnceCell<(usize, usize)> = OnceCell::new();
    *INFO.get_or_init(|| {
        let system_info = unsafe {
            let mut system_info = std::mem::MaybeUninit::<SYSTEM_INFO>::uninit();
            GetSystemInfo(system_info.as_mut_ptr());
            system_info.assume_init()
        };
        (
            system_info.dwPageSize as usize,
            system_info.dwAllocationGranularity as usize,
        )
    })
}

pub fn page_size() -> usize {
    system_info().0
}

pub fn allocation_size() -> usize {
    system_info().1
}

struct FileHandle(HANDLE);

impl Drop for FileHandle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

impl FileHandle {
    fn as_handle(&self) -> HANDLE {
        self.0
    }
}

fn create_file_mapping(size: usize) -> Result<FileHandle, SystemError> {
    let handle = unsafe {
        CreateFileMappingA(
            INVALID_HANDLE_VALUE,
            std::ptr::null_mut(),
            PAGE_READWRITE | SEC_COMMIT,
            size.checked_shr(std::mem::size_of::<DWORD>() as u32 * 8)
                .unwrap_or(0)
                .try_into()
                .unwrap(),
            size.try_into().unwrap(),
            std::ptr::null_mut(),
        )
    };

    if handle == INVALID_HANDLE_VALUE {
        Err(Error::last().into())
    } else {
        Ok(FileHandle(handle))
    }
}

fn reserve_memory(size: usize) -> Result<*mut c_void, SystemError> {
    unsafe {
        let address = VirtualAlloc(std::ptr::null_mut(), size, MEM_RESERVE, PAGE_NOACCESS);
        if address.is_null() || VirtualFree(address, 0, MEM_RELEASE) == 0 {
            Err(Error::last().into())
        } else {
            Ok(address)
        }
    }
}

unsafe fn map_view_of_file(
    handle: HANDLE,
    size: usize,
    address: *mut c_void,
) -> Result<(), SystemError> {
    let address = MapViewOfFileEx(handle, FILE_MAP_ALL_ACCESS, 0, 0, size, address);
    if address.is_null() {
        Err(Error::last().into())
    } else {
        Ok(())
    }
}

unsafe fn unmap_view_of_file(address: *mut c_void) -> Result<(), SystemError> {
    if UnmapViewOfFile(address) == 0 {
        Err(Error::last().into())
    } else {
        Ok(())
    }
}

pub fn allocate_mirrored(size: usize) -> Result<*mut u8, SystemError> {
    if size == 0 {
        return Ok(std::ptr::null_mut());
    }

    let double_size = size.checked_mul(2).unwrap();

    assert!(size % allocation_size() == 0);

    // Create the underlying file mapping
    let handle = create_file_mapping(size)?;

    // Try to create the mappings
    let mut tries = 0;
    const MAX_TRIES: usize = 5;
    loop {
        tries += 1;
        let ptr = reserve_memory(double_size)?;
        unsafe {
            if let Err(err) = map_view_of_file(handle.as_handle(), size, ptr) {
                if tries == MAX_TRIES {
                    break Err(err);
                } else {
                    continue;
                }
            }
            if let Err(err) = map_view_of_file(handle.as_handle(), size, ptr.add(size)) {
                if tries == MAX_TRIES {
                    unmap_view_of_file(ptr).unwrap();
                    break Err(err);
                } else {
                    continue;
                }
            }
        }
        break Ok(ptr as *mut u8);
    }
}

pub unsafe fn deallocate_mirrored<T>(address: *mut T, size: usize) -> Result<(), SystemError> {
    if !address.is_null() {
        unmap_view_of_file(address as *mut c_void)?;
        unmap_view_of_file(address.add(size) as *mut c_void)?;
    }
    Ok(())
}
