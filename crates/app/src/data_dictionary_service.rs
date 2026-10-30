//! Data Dictionary Maintenance 資料字典維護 use case（TipTop: p_zta）
//! 規格：specs/features/azz/data_dictionary.feature（D1–D8）
//!
//! 只維護資料表與欄位的名稱、目的、多語言說明；結構（欄位、索引、限制）由資料庫唯讀取得，
//! 不執行任何 DDL（結構只由 migrations/ 變更）。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    dictionary::{
        ColumnText, Lang, TableDictionary, TableDictionaryAttrs, TableText, TableType,
        migration_touches,
    },
    lang::{DEFAULT_LANG, localized},
    program::ModuleCode,
};
use ttrs_shared::{
    ColumnInfoDto, ColumnTextDto, ConstraintDto, ConstraintKindDto, IndexDto, MigrationDto,
    TableDictionaryDto, TableDictionaryInput, TableTextDto, TableTypeDto,
};

use crate::{
    Actor, AppError,
    error::RepoError,
    ports::{ConstraintKind, DictionaryRepository, SchemaCatalog, TableSchema},
};

#[derive(Clone)]
pub struct DataDictionaryService {
    repo: Arc<dyn DictionaryRepository>,
    schema: Arc<dyn SchemaCatalog>,
}

impl DataDictionaryService {
    pub fn new(repo: Arc<dyn DictionaryRepository>, schema: Arc<dyn SchemaCatalog>) -> Self {
        Self { repo, schema }
    }

    /// 建立資料表字典（p_zta_a）；資料表必須已存在於資料庫（D1）
    pub async fn create(
        &self,
        actor: &Actor,
        input: TableDictionaryInput,
    ) -> Result<TableDictionaryDto, AppError> {
        let name = TableDictionary::parse_name(&input.table_name)?;
        let module = self.module(&input.module_code).await?;
        let dict = TableDictionary::new(&name, module, attrs(&input), &actor.user_id, today())?;
        self.schema_table(&dict.table_name).await?;
        match self.repo.insert(&dict).await {
            Ok(()) => self.to_dto(&dict, &Lang::default_lang()).await,
            Err(RepoError::Conflict) => Err(DomainError::DictAlreadyExists.into()),
            Err(e) => Err(infra(e)),
        }
    }

    /// 修改表頭（p_zta_u）：模組、類型、上游/下游；資料表代碼不可改
    pub async fn update(
        &self,
        table: &str,
        input: TableDictionaryInput,
    ) -> Result<TableDictionaryDto, AppError> {
        let mut dict = self.load(table).await?;
        let module = self.module(&input.module_code).await?;
        dict.update(module, attrs(&input))?;
        self.repo.update(&dict).await.map_err(infra)?;
        self.to_dto(&dict, &Lang::default_lang()).await
    }

    pub async fn get(&self, table: &str) -> Result<TableDictionaryDto, AppError> {
        self.get_localized(table, DEFAULT_LANG).await
    }

    /// 以指定語系顯示名稱與目的；該語系無資料時退回繁中（ADR 0005）
    pub async fn get_localized(
        &self,
        table: &str,
        lang: &str,
    ) -> Result<TableDictionaryDto, AppError> {
        let lang = Lang::parse(lang)?;
        self.to_dto(&self.load(table).await?, &lang).await
    }

    pub async fn list(&self) -> Result<Vec<TableDictionaryDto>, AppError> {
        self.list_localized(DEFAULT_LANG).await
    }

    pub async fn list_localized(&self, lang: &str) -> Result<Vec<TableDictionaryDto>, AppError> {
        let lang = Lang::parse(lang)?;
        let list = self.repo.list().await.map_err(infra)?;
        let mut out = Vec::with_capacity(list.len());
        for d in &list {
            out.push(self.to_dto(d, &lang).await?);
        }
        Ok(out)
    }

    /// 資料庫中的所有資料表（含尚未建立字典者）
    pub async fn schema_tables(&self) -> Result<Vec<String>, AppError> {
        self.schema.table_names().await.map_err(infra)
    }

    /// 刪除字典，連同表與欄位的多語言說明（D3）；實際資料表不受影響
    pub async fn delete(&self, table: &str) -> Result<(), AppError> {
        let dict = self.load(table).await?;
        self.repo.delete(&dict.table_name).await.map_err(infra)
    }

    /// 維護某語系的表名稱、目的、備註（gat03/gat04/gat05）
    pub async fn set_table_text(&self, table: &str, text: TableTextDto) -> Result<(), AppError> {
        let dict = self.load(table).await?;
        let text = TableText::new(&text.lang, text.name, text.purpose, text.remark)?;
        self.repo
            .set_table_text(&dict.table_name, &text)
            .await
            .map_err(infra)
    }

    pub async fn table_texts(&self, table: &str) -> Result<Vec<TableTextDto>, AppError> {
        let table = TableDictionary::parse_name(table)?;
        let texts = self.repo.table_texts(&table).await.map_err(infra)?;
        Ok(texts.iter().map(table_text_dto).collect())
    }

    /// 維護某語系的欄位名稱、說明、額外說明（gaq03/gaq04/gaq05；鍵值為資料表＋欄位，D2）
    pub async fn set_column_text(&self, table: &str, text: ColumnTextDto) -> Result<(), AppError> {
        let dict = self.load(table).await?;
        let text = ColumnText::new(
            &text.column_name,
            &text.lang,
            text.name,
            text.description,
            text.extra,
        )?;
        let schema = self.schema_table(&dict.table_name).await?;
        if !schema.columns.iter().any(|c| c.name == text.column_name) {
            return Err(DomainError::DictColumnNotFound(text.column_name).into());
        }
        self.repo
            .set_column_text(&dict.table_name, &text)
            .await
            .map_err(infra)
    }

    pub async fn column_texts(&self, table: &str) -> Result<Vec<ColumnTextDto>, AppError> {
        let table = TableDictionary::parse_name(table)?;
        let texts = self.repo.column_texts(&table).await.map_err(infra)?;
        Ok(texts.into_iter().map(column_text_dto).collect())
    }

    /// 資料庫實際欄位結構（唯讀）
    pub async fn columns(&self, table: &str) -> Result<Vec<ColumnInfoDto>, AppError> {
        let schema = self.schema_table(table).await?;
        let pk: Vec<&String> = schema
            .constraints
            .iter()
            .filter(|c| c.kind == ConstraintKind::PrimaryKey)
            .flat_map(|c| &c.columns)
            .collect();
        Ok(schema
            .columns
            .iter()
            .map(|c| ColumnInfoDto {
                name: c.name.clone(),
                data_type: c.data_type.clone(),
                not_null: c.not_null,
                primary_key: pk.contains(&&c.name),
                default: c.default.clone(),
            })
            .collect())
    }

    pub async fn indexes(&self, table: &str) -> Result<Vec<IndexDto>, AppError> {
        let schema = self.schema_table(table).await?;
        Ok(schema
            .indexes
            .into_iter()
            .map(|i| IndexDto {
                name: i.name,
                unique: i.unique,
                columns: i.columns,
            })
            .collect())
    }

    pub async fn constraints(&self, table: &str) -> Result<Vec<ConstraintDto>, AppError> {
        let schema = self.schema_table(table).await?;
        Ok(schema
            .constraints
            .into_iter()
            .map(|c| ConstraintDto {
                name: c.name,
                kind: match c.kind {
                    ConstraintKind::PrimaryKey => ConstraintKindDto::PrimaryKey,
                    ConstraintKind::ForeignKey => ConstraintKindDto::ForeignKey,
                    ConstraintKind::Unique => ConstraintKindDto::Unique,
                    ConstraintKind::Check => ConstraintKindDto::Check,
                },
                columns: c.columns,
                ref_table: c.ref_table,
                ref_columns: c.ref_columns,
                ref_constraint: c.ref_constraint,
                check_clause: c.check_clause,
            })
            .collect())
    }

    /// 變更紀錄：內容涉及該表的 migration（D5）
    pub async fn history(&self, table: &str) -> Result<Vec<MigrationDto>, AppError> {
        let table = TableDictionary::parse_name(table)?;
        let migrations = self.schema.migrations().await.map_err(infra)?;
        Ok(migrations
            .into_iter()
            .filter(|m| migration_touches(&m.sql, &table))
            .map(|m| MigrationDto {
                version: m.version,
                description: m.description,
            })
            .collect())
    }

    /// 匯出 CSV（D6，比照 p_zta_exp2excel）：欄位代碼、型別、名稱、額外說明；
    /// 名稱與額外說明各自依語系退回繁中（ADR 0005）
    pub async fn export_csv(&self, table: &str, lang: &str) -> Result<String, AppError> {
        let lang = Lang::parse(lang)?;
        let columns = self.columns(table).await?;
        let texts = self
            .repo
            .column_texts(&TableDictionary::parse_name(table)?)
            .await
            .map_err(infra)?;
        let header = match lang.as_str() {
            "en" => ["Column", "Type", "Name", "Extra"],
            "zh-CN" => ["栏位代码", "类型", "名称", "额外说明"],
            _ => ["欄位代碼", "型別", "名稱", "額外說明"],
        };
        let mut out = csv_line(&header);
        for c in &columns {
            let own: Vec<&ColumnText> = texts.iter().filter(|t| t.column_name == c.name).collect();
            let name = localized(&own, &lang, |t| t.name.clone()).unwrap_or_default();
            let extra = localized(&own, &lang, |t| t.extra.clone()).unwrap_or_default();
            out.push_str(&csv_line(&[&c.name, &c.data_type, &name, &extra]));
        }
        Ok(out)
    }

    /// 資料表是否存在於資料庫
    pub async fn table_exists(&self, table: &str) -> Result<bool, AppError> {
        let table = TableDictionary::parse_name(table)?;
        Ok(self.schema.table(&table).await.map_err(infra)?.is_some())
    }

    async fn to_dto(
        &self,
        d: &TableDictionary,
        lang: &Lang,
    ) -> Result<TableDictionaryDto, AppError> {
        let texts = self.repo.table_texts(&d.table_name).await.map_err(infra)?;
        Ok(TableDictionaryDto {
            table_name: d.table_name.clone(),
            module_code: d.module.as_str().to_owned(),
            table_type: d.table_type.map(table_type_dto),
            upstream: d.upstream.clone(),
            downstream: d.downstream.clone(),
            created_by: d.created_by.clone(),
            created_on: d.created_on,
            name: localized(&texts, lang, |t| t.name.clone()),
            purpose: localized(&texts, lang, |t| t.purpose.clone()),
            texts: texts.iter().map(table_text_dto).collect(),
        })
    }

    async fn load(&self, table: &str) -> Result<TableDictionary, AppError> {
        let table = TableDictionary::parse_name(table)?;
        self.repo
            .get(&table)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::DictNotFound.into())
    }

    /// 資料庫中的資料表結構；不存在則 DICT_TABLE_NOT_IN_SCHEMA（D1）
    async fn schema_table(&self, table: &str) -> Result<TableSchema, AppError> {
        let table = TableDictionary::parse_name(table)?;
        self.schema
            .table(&table)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::DictTableNotInSchema(table).into())
    }

    async fn module(&self, raw: &str) -> Result<ModuleCode, AppError> {
        let module = ModuleCode::parse(raw)?;
        if !self.repo.module_exists(&module).await.map_err(infra)? {
            return Err(DomainError::ModuleNotFound.into());
        }
        Ok(module)
    }
}

fn infra(e: RepoError) -> AppError {
    AppError::Infra(e.to_string())
}

fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

fn attrs(i: &TableDictionaryInput) -> TableDictionaryAttrs {
    TableDictionaryAttrs {
        table_type: i.table_type.map(table_type),
        upstream: i.upstream.clone(),
        downstream: i.downstream.clone(),
    }
}

fn table_type(t: TableTypeDto) -> TableType {
    match t {
        TableTypeDto::Parameter => TableType::Parameter,
        TableTypeDto::Master => TableType::Master,
        TableTypeDto::Transaction => TableType::Transaction,
        TableTypeDto::Statistics => TableType::Statistics,
        TableTypeDto::Base => TableType::Base,
    }
}

fn table_type_dto(t: TableType) -> TableTypeDto {
    match t {
        TableType::Parameter => TableTypeDto::Parameter,
        TableType::Master => TableTypeDto::Master,
        TableType::Transaction => TableTypeDto::Transaction,
        TableType::Statistics => TableTypeDto::Statistics,
        TableType::Base => TableTypeDto::Base,
    }
}

fn table_text_dto(t: &TableText) -> TableTextDto {
    TableTextDto {
        lang: t.lang.as_str().to_owned(),
        name: t.name.clone(),
        purpose: t.purpose.clone(),
        remark: t.remark.clone(),
    }
}

fn column_text_dto(t: ColumnText) -> ColumnTextDto {
    ColumnTextDto {
        column_name: t.column_name,
        lang: t.lang.as_str().to_owned(),
        name: t.name,
        description: t.description,
        extra: t.extra,
    }
}

/// CSV 一列（RFC 4180：含逗號、引號、換行的欄位以雙引號包起來）
fn csv_line(fields: &[&str]) -> String {
    let cells: Vec<String> = fields
        .iter()
        .map(|f| {
            if f.contains([',', '"', '\n', '\r']) {
                format!("\"{}\"", f.replace('"', "\"\""))
            } else {
                (*f).to_owned()
            }
        })
        .collect();
    cells.join(",") + "\n"
}
