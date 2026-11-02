//! 應用層：協調 domain 與 repository，不含 SQL、不含 HTTP。

pub mod data_dictionary_service;
pub mod error;
pub mod item_service;
pub mod legacy_catalog_service;
#[cfg(feature = "memory")]
pub mod memory;
pub mod permission_class_service;
pub mod permission_service;
pub mod plant_service;
pub mod ports;
pub mod program_query;
pub mod program_service;
pub mod query;
pub mod user_permission_service;
pub mod user_service;

pub use data_dictionary_service::DataDictionaryService;
pub use error::AppError;
pub use item_service::ItemService;
pub use legacy_catalog_service::LegacyCatalogService;
pub use permission_class_service::PermissionClassService;
pub use permission_service::ProgramPermissionService;
pub use plant_service::PlantService;
pub use program_service::{Actor, ProgramService};
pub use user_permission_service::UserPermissionService;
pub use user_service::UserService;
