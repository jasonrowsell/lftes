use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildError {
    InvalidCapacity,
    TooLarge,
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::InvalidCapacity => write!(f, "Capacity must be a power of two"),
            BuildError::TooLarge => write!(f, "Capacity exceeds maximum size"),
        }
    }
}

impl std::error::Error for BuildError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushError {
    BufferFull,
    Shutdown,
}

impl fmt::Display for PushError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PushError::BufferFull => write!(f, "Buffer is full"),
            PushError::Shutdown => write!(f, "Buffer is shutting down"),
        }
    }
}

impl std::error::Error for PushError {}
