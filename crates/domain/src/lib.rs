//! 領域層：TipTop 業務規則的 Rust 版本。
//! 每個模組開頭註明對應的 TipTop 程式代號與資料表，方便回溯原始 4GL。

pub mod dictionary;
pub mod error;
pub mod item;
pub mod lang;
pub mod legacy_catalog;
pub mod permission;
pub mod permission_class;
pub mod plant;
pub mod program;
pub mod qbe;
pub mod user;
pub mod user_permission;

pub use error::DomainError;
