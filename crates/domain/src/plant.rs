//! 營運中心（TipTop: azp_file / 各營運中心資料庫）
//! 新系統採單一 schema + plant_id 欄位隔離，見 docs/adr/0001-architecture.md。

use crate::DomainError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlantId(String);

impl PlantId {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim();
        if v.is_empty() {
            return Err(DomainError::PlantIdEmpty);
        }
        Ok(Self(v.to_uppercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
