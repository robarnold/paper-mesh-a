//! Failures from an explicit system power-off request.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerOffError {
    Unsupported,
    Busy,
    ExternalPower,
    Preparation,
    StillPowered,
}

impl PowerOffError {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Unsupported => "Power off is unavailable",
            Self::Busy => "Device busy. Try again",
            Self::ExternalPower => "Disconnect USB and external power",
            Self::Preparation => "Power-off preparation failed",
            Self::StillPowered => "Device still powered. Try again",
        }
    }
}
