//! 權限檢查開關（authorization.feature AQ12）：既有規格預設不檢查權限；
//! 權限規格的「目前使用者執行 …」步驟才以真正的 Authorizer 檢查（記憶體與 PostgreSQL 同一套）。

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use async_trait::async_trait;
use ttrs_app::{
    Actor, AppError, Authorizer, PermissionAuthorizer,
    authorization::Grant,
    ports::{ProgramPermissionRepository, UserPermissionRepository},
};
use ttrs_domain::authorization::EffectivePermission;

pub struct ToggleAuthorizer {
    inner: PermissionAuthorizer,
    enforce: AtomicBool,
}

impl ToggleAuthorizer {
    pub fn new(
        classes: Arc<dyn ProgramPermissionRepository>,
        special: Arc<dyn UserPermissionRepository>,
    ) -> Arc<Self> {
        Arc::new(Self {
            inner: PermissionAuthorizer::new(classes, special),
            enforce: AtomicBool::new(false),
        })
    }

    pub fn set_enforced(&self, on: bool) {
        self.enforce.store(on, Ordering::SeqCst);
    }
}

#[async_trait]
impl Authorizer for ToggleAuthorizer {
    async fn effective(
        &self,
        actor: &Actor,
        program: &str,
    ) -> Result<Option<EffectivePermission>, AppError> {
        self.inner.effective(actor, program).await
    }

    async fn authorize(
        &self,
        actor: &Actor,
        program: &'static str,
        action: &'static str,
    ) -> Result<Grant, AppError> {
        if self.enforce.load(Ordering::SeqCst) {
            self.inner.authorize(actor, program, action).await
        } else {
            Ok(Grant::unrestricted(actor, program, action))
        }
    }
}
