#[cfg(windows)]
pub(crate) mod windows {
    #[derive(Debug)]
    pub struct Error(winapi::shared::minwindef::DWORD);

    impl Error {
        pub fn last() -> Self {
            Self(unsafe { winapi::um::errhandlingapi::GetLastError() })
        }
    }

    impl std::error::Error for Error {}

    impl std::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
            use std::convert::TryInto;
            use winapi::um::{
                winbase::{
                    FormatMessageW, FORMAT_MESSAGE_FROM_SYSTEM, FORMAT_MESSAGE_IGNORE_INSERTS,
                },
                winnt::{LANG_NEUTRAL, MAKELANGID, SUBLANG_DEFAULT},
            };

            let mut buf = [0; 256];
            let written = unsafe {
                FormatMessageW(
                    FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
                    std::ptr::null(),
                    self.0,
                    MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT).into(),
                    buf.as_mut_ptr(),
                    buf.len().try_into().unwrap(),
                    std::ptr::null_mut(),
                )
            };
            assert!(written != 0);
            write!(
                f,
                "{}",
                String::from_utf16_lossy(&buf[..written.try_into().unwrap()])
            )
        }
    }
}

/// Indicates a system error, such as out-of-memory.
#[derive(Debug)]
pub struct SystemError(
    #[cfg(unix)] pub(crate) nix::Error,
    #[cfg(windows)] pub(crate) windows::Error,
);

impl std::error::Error for SystemError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl std::fmt::Display for SystemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}", self.0)
    }
}

#[cfg(unix)]
impl From<nix::Error> for SystemError {
    fn from(e: nix::Error) -> Self {
        Self(e)
    }
}

#[cfg(windows)]
impl From<windows::Error> for SystemError {
    fn from(e: windows::Error) -> Self {
        Self(e)
    }
}
