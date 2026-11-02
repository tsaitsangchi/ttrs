use thiserror::Error;
use ttrs_domain::DomainError;

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Domain(#[from] DomainError),
    #[error("基礎設施錯誤：{0}")]
    Infra(String),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Domain(e) => e.code(),
            Self::Infra(_) => "INFRA_ERROR",
        }
    }
}

/// Repository 實作回傳的錯誤；唯一鍵衝突需轉成 `Conflict`、仍被其他資料參照（外鍵）轉成 `InUse`，
/// 讓應用層翻成業務錯誤。
#[derive(Debug, Error)]
pub enum RepoError {
    #[error("唯一鍵衝突")]
    Conflict,
    #[error("資料仍被其他資料參照")]
    InUse,
    #[error("{0}")]
    Other(String),
}
