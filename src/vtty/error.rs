use thiserror::Error;

#[derive(Error, Debug)]
pub enum VttyError {
    #[error("Invalid ANSI sequence: {0:?}")]
    InvalidSequence(Vec<u8>),

    #[error("Unsupported CSI parameter: {0}")]
    UnsupportedCsiParam(String),

    #[error("Buffer overflow: cursor at ({row}, {col}) exceeds bounds ({max_rows}, {max_cols})")]
    BufferOverflow { row: usize, col: usize, max_rows: usize, max_cols: usize },

    #[error("Invalid color value: {0}")]
    InvalidColor(u8),
}

pub type VttyResult<T> = Result<T, VttyError>;
