use crate::{fmt, io::ErrorKind};

use super::abi;

/// Wraps a μITRON error code.
#[derive(Debug, Copy, Clone)]
pub struct ItronError {
    er: abi::ER,
}

impl ItronError {
    /// Construct `ItronError` from the specified error code. Returns `None` if the
    /// error code does not represent a failure or warning.
    #[inline]
    pub fn new(er: abi::ER) -> Option<Self> {
        if er < 0 {
            Some(Self { er })
        } else {
            None
        }
    }

    /// Returns `Ok(er)` if `er` represents a success or `Err(_)` otherwise.
    #[inline]
    pub fn err_if_negative(er: abi::ER) -> Result<abi::ER, Self> {
        if let Some(error) = Self::new(er) {
            Err(error)
        } else {
            Ok(er)
        }
    }

    /// Get the raw error code.
    #[inline]
    pub fn as_raw(&self) -> abi::ER {
        self.er
    }
}

impl fmt::Display for ItronError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(name) = error_name(self.er) {
            write!(f, "{} ({})", name, self.er)
        } else {
            write!(f, "{}", self.er)
        }
    }
}

/// Describe the specified μITRON error code. Returns `None` if it's an
/// undefined error code.
pub fn error_name(er: abi::ER) -> Option<&'static str> {
    match er {
        er if er >= 0 => Some("operation successful"),

        // μITRON 4.0
        abi::E_SYS => Some("system error"),
        abi::E_NOSPT => Some("unsupported function"),
        abi::E_RSFN => Some("reserved function code"),
        abi::E_RSATR => Some("reserved attribute"),
        abi::E_PAR => Some("parameter error"),
        abi::E_ID => Some("invalid ID number"),
        abi::E_CTX => Some("context error"),
        abi::E_MACV => Some("memory access violation"),
        abi::E_OACV => Some("object access violation"),
        abi::E_ILUSE => Some("illegal service call use"),
        abi::E_NOMEM => Some("insufficient memory"),
        abi::E_NOID => Some("no ID number available"),
        abi::E_OBJ => Some("object state error"),
        abi::E_NOEXS => Some("non-existent object"),
        abi::E_QOVR => Some("queue overflow"),
        abi::E_RLWAI => Some("forced release from waiting"),
        abi::E_TMOUT => Some("polling failure or timeout"),
        abi::E_DLT => Some("waiting object deleted"),
        abi::E_CLS => Some("waiting object state changed"),
        abi::E_WBLK => Some("non-blocking code accepted"),
        abi::E_BOVR => Some("buffer overflow"),

        // The TOPPERS third generation kernels
        abi::E_NORES => Some("insufficient system resources"),
        abi::E_RASTER => Some("termination request raised"),
        abi::E_COMM => Some("communication failure"),

        _ => None,
    }
}

pub fn decode_error_kind(er: abi::ER) -> ErrorKind {
    match er {
        // Success
        er if er >= 0 => ErrorKind::Other,

        // μITRON 4.0
        // abi::E_SYS
        abi::E_NOSPT => ErrorKind::Unsupported, // Some("unsupported function"),
        abi::E_RSFN => ErrorKind::InvalidInput, // Some("reserved function code"),
        abi::E_RSATR => ErrorKind::InvalidInput, // Some("reserved attribute"),
        abi::E_PAR => ErrorKind::InvalidInput,  // Some("parameter error"),
        abi::E_ID => ErrorKind::NotFound,       // Some("invalid ID number"),
        // abi::E_CTX
        abi::E_MACV => ErrorKind::PermissionDenied, // Some("memory access violation"),
        abi::E_OACV => ErrorKind::PermissionDenied, // Some("object access violation"),
        // abi::E_ILUSE
        abi::E_NOMEM => ErrorKind::OutOfMemory, // Some("insufficient memory"),
        abi::E_NOID => ErrorKind::OutOfMemory,  // Some("no ID number available"),
        // abi::E_OBJ
        abi::E_NOEXS => ErrorKind::NotFound, // Some("non-existent object"),
        // abi::E_QOVR
        abi::E_RLWAI => ErrorKind::Interrupted, // Some("forced release from waiting"),
        abi::E_TMOUT => ErrorKind::TimedOut,    // Some("polling failure or timeout"),
        // abi::E_DLT
        // abi::E_CLS
        // abi::E_WBLK
        // abi::E_BOVR

        // The TOPPERS third generation kernels
        abi::E_NORES => ErrorKind::OutOfMemory, // Some("insufficient system resources"),
        // abi::E_RASTER
        // abi::E_COMM
        _ => ErrorKind::Other,
    }
}
