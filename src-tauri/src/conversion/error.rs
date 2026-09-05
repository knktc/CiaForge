use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConversionError {
    #[error("could not read {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("{path} is not a CCI file: missing {expected} magic")]
    InvalidMagic {
        path: String,
        expected: &'static str,
    },
    #[error("{path} has an invalid {name} partition range")]
    InvalidPartition { path: String, name: &'static str },
    #[error("{path} has no game executable partition")]
    MissingGamePartition { path: String },
    #[error("{path} has an invalid ExtHeader SHA-256 hash")]
    InvalidExtHeaderHash { path: String },
    #[error("{path} does not contain an ExeFS icon")]
    MissingIcon { path: String },
    #[error("{path} uses {mode:?} encryption, which is not implemented yet")]
    UnsupportedEncryption {
        path: String,
        mode: super::cci::EncryptionMode,
    },
    #[error("CIA template error: {0}")]
    Template(&'static str),
    #[error("{path} already exists; CiaForge will not overwrite it")]
    AlreadyExists { path: String },
}
