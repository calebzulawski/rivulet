//! Errors produced by streams.

/// Error used when a grant or release will never fail.
#[derive(Copy, Clone, Debug)]
pub enum Infallible {}

impl core::fmt::Display for Infallible {
    fn fmt(&self, _: &mut core::fmt::Formatter) -> core::fmt::Result {
        unreachable!()
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Infallible {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        unreachable!()
    }
}

#[cfg(feature = "std")]
impl core::convert::From<Infallible> for std::io::Error {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

impl core::convert::From<Infallible> for GrantOverflow {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

/// Error produced when a request is too large to grant.
///
/// Contains the maximum allowable grant.
#[derive(Copy, Clone, Debug)]
pub struct GrantOverflow(pub usize);

impl core::fmt::Display for GrantOverflow {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> Result<(), core::fmt::Error> {
        write!(f, "request exceeded maximum possible grant of `{}`", self.0)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for GrantOverflow {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

#[cfg(feature = "std")]
impl std::convert::From<GrantOverflow> for std::io::Error {
    fn from(e: GrantOverflow) -> Self {
        Self::new(std::io::ErrorKind::InvalidInput, e)
    }
}
