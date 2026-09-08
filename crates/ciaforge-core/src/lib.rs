//! Format conversion shared by the desktop and browser adapters.
//!
//! This crate deliberately owns no paths, files, Tauri commands, or browser
//! bindings. Hosts provide seekable input and output streams instead.

mod async_cci;
mod async_engine;
mod async_ncch;
mod cci;
mod cia;
mod copy;
mod engine;
mod error;
mod io;
mod ncch;
mod templates;
mod writer;

pub use async_cci::inspect as inspect_async;
pub use async_engine::convert_async;
pub use async_ncch::prepare as prepare_async;
pub use cci::CciHeader;
pub use cia::CiaPlan;
pub use copy::ProgressSink;
pub use engine::convert;
pub use error::ConversionError;
pub use io::{RandomAccessInput, RandomAccessOutput};
pub use ncch::PreparedGame;
pub use templates::RetailTemplates;
pub use writer::CiaHeader;
