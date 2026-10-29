//! 應用層：協調 domain 與 repository，不含 SQL、不含 HTTP。

pub mod error;
pub mod item_service;
#[cfg(feature = "memory")]
pub mod memory;
pub mod ports;
pub mod program_query;
pub mod program_service;

pub use error::AppError;
pub use item_service::ItemService;
pub use program_service::{Actor, ProgramService};
