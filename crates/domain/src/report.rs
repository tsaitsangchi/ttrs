//! 報表共用規則（docs/adr/0011-reports-and-printing.md）

use crate::DomainError;

/// 同步產生報表的筆數上限（ADR 0011 RP6）；超過時請縮小條件，背景作業另案
pub const MAX_SYNC_ROWS: u64 = 5_000;

/// 筆數超過上限 → REPORT_TOO_LARGE
pub fn check_size(rows: u64, limit: u64) -> Result<(), DomainError> {
    if rows > limit {
        return Err(DomainError::ReportTooLarge { limit });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_limit() {
        assert!(check_size(5_000, MAX_SYNC_ROWS).is_ok());
        assert_eq!(
            check_size(5_001, MAX_SYNC_ROWS).unwrap_err().code(),
            "REPORT_TOO_LARGE"
        );
    }
}
