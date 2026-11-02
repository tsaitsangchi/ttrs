//! 系統管理模組 (TipTop azz)

mod data_dictionary;
mod legacy_catalog;
mod permission_class_maintenance;
mod program_maintenance;
mod program_permission_maintenance;
mod user_maintenance;
mod user_permission_maintenance;

pub use data_dictionary::DataDictionary;
pub use permission_class_maintenance::PermissionClassMaintenance;
pub use program_maintenance::ProgramMaintenance;
pub use program_permission_maintenance::{ProgramPermissions, ProgramPermissionsFor};
pub use user_maintenance::UserMaintenance;
pub use user_permission_maintenance::{UserPermissions, UserPermissionsFor};
