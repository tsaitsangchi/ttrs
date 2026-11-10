//! 標準確認欄位（docs/adr/0013-standard-maintenance-actions.md SA9）：所有維護作業的「確認」功能鈕（確認 confirm／取消確認 notconfirm）。
//! TipTop 只有部分資料表有確認狀態（例如料件 imaacti／ima1010）；沒有的資料表由 TTRS 加上
//! 已確認否、確認人、確認時間三個欄位（SA9-1）。行為比照料件 aimi100：已確認可以修改、不可刪除（SA9-2，TipTop 9023）。

use chrono::{DateTime, Utc};

use crate::DomainError;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Confirmation {
    pub confirmed: bool,
    /// 確認人（使用者代號）
    pub by: Option<String>,
    pub at: Option<DateTime<Utc>>,
}

impl Confirmation {
    /// 已確認（轉入的既有資料，SA9-3）
    pub fn confirmed(by: Option<String>, at: Option<DateTime<Utc>>) -> Self {
        Self {
            confirmed: true,
            by,
            at,
        }
    }

    /// 確認：已確認 → RECORD_ALREADY_CONFIRMED
    pub fn confirm(&mut self, user: &str, now: DateTime<Utc>) -> Result<(), DomainError> {
        if self.confirmed {
            return Err(DomainError::RecordAlreadyConfirmed);
        }
        *self = Self::confirmed(Some(user.to_owned()), Some(now));
        Ok(())
    }

    /// 取消確認：未確認 → RECORD_NOT_CONFIRMED；清空確認人與時間
    pub fn unconfirm(&mut self) -> Result<(), DomainError> {
        if !self.confirmed {
            return Err(DomainError::RecordNotConfirmed);
        }
        *self = Self::default();
        Ok(())
    }

    /// 已確認不可刪除，須先取消確認 → RECORD_CONFIRMED_NOT_DELETABLE
    pub fn check_deletable(&self) -> Result<(), DomainError> {
        if self.confirmed {
            return Err(DomainError::RecordConfirmedNotDeletable);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirm_and_unconfirm() {
        let mut c = Confirmation::default();
        assert_eq!(c.unconfirm(), Err(DomainError::RecordNotConfirmed));
        c.confirm("tiptop", Utc::now()).unwrap();
        assert_eq!(c.by.as_deref(), Some("tiptop"));
        assert_eq!(
            c.check_deletable(),
            Err(DomainError::RecordConfirmedNotDeletable)
        );
        assert_eq!(
            c.confirm("tiptop", Utc::now()),
            Err(DomainError::RecordAlreadyConfirmed)
        );
        c.unconfirm().unwrap();
        assert_eq!(c, Confirmation::default());
        assert!(c.check_deletable().is_ok());
    }
}
