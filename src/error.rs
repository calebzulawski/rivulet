//! Errors produced by streams.

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
