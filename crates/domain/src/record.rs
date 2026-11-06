//! 資料列的版本與時間（docs/adr/0012-change-history.md §1、§4）：樂觀鎖的版本號、建立與最後修改時間（到秒）。
//! 由儲存端維護（PostgreSQL 觸發器 ttrs_bump_version／記憶體後端），use case 只負責比對版本。

use chrono::{DateTime, Utc};

use crate::error::DomainError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp {
    /// 版本號：新增為 1，每次修改加 1
    pub version: i32,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

impl Default for Stamp {
    fn default() -> Self {
        Self {
            version: 1,
            created_at: None,
            updated_at: None,
        }
    }
}

impl Stamp {
    /// 請求帶的版本與目前版本不同 → RECORD_CHANGED；沒帶版本（內部呼叫）時不比對
    pub fn check(&self, expected: Option<i32>) -> Result<(), DomainError> {
        match expected {
            Some(v) if v != self.version => Err(DomainError::RecordChanged),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_mismatch_is_record_changed() {
        let s = Stamp::default();
        assert!(s.check(None).is_ok());
        assert!(s.check(Some(1)).is_ok());
        assert_eq!(s.check(Some(2)), Err(DomainError::RecordChanged));
    }
}
