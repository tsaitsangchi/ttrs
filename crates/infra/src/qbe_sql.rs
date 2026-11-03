//! QBE 篩選 → SQL 條件（各作業查詢共用）：欄位名稱只來自各作業的白名單（`column`），條件值一律 push_bind。
//! 條件語法見 ttrs_domain::qbe；文字以位元組順序比較、區分大小寫（QBE2）。

use sqlx::{FromRow, PgPool, Postgres, QueryBuilder, postgres::PgRow};
use ttrs_app::error::RepoError;
use ttrs_domain::{
    authorization::DataScope,
    qbe::{PatternPart, QbeFilter, QbeTerm, QbeValue},
};

use crate::pg::db_err;

/// 查詢鍵值對應的資料庫欄位
#[derive(Debug, Clone, Copy)]
pub(crate) enum Column {
    /// (欄位, 是否文字)
    Scalar(&'static str, bool),
    /// 文字陣列，逐一比對每個值（允許執行功能，YQ4）
    TextArray(&'static str),
}

/// 每個篩選加上 ` AND (…)`；條件之間 OR
pub(crate) fn push_filters(
    qb: &mut QueryBuilder<Postgres>,
    filters: &[QbeFilter],
    column: fn(&str) -> Option<Column>,
) -> Result<(), RepoError> {
    for f in filters {
        let col = column(f.field)
            .ok_or_else(|| RepoError::Other(format!("未定義的查詢欄位：{}", f.field)))?;
        qb.push(" AND (");
        for (i, term) in f.terms.iter().enumerate() {
            if i > 0 {
                qb.push(" OR ");
            }
            match col {
                Column::Scalar(c, is_text) => push_term(qb, c, is_text, term),
                Column::TextArray(c) => push_set_term(qb, c, term),
            }
        }
        qb.push(")");
    }
    Ok(())
}

/// 單一欄位、單一條件
pub(crate) fn push_term(
    qb: &mut QueryBuilder<Postgres>,
    col: &'static str,
    is_text: bool,
    term: &QbeTerm,
) {
    let collate = if is_text { r#" COLLATE "C""# } else { "" };
    let mut cmp = |op: &str, v: &QbeValue| {
        qb.push(col).push(collate).push(op);
        push_value(qb, v);
    };
    match term {
        QbeTerm::IsNull => {
            if is_text {
                qb.push("(")
                    .push(col)
                    .push(" IS NULL OR ")
                    .push(col)
                    .push(" = '')");
            } else {
                qb.push(col).push(" IS NULL");
            }
        }
        QbeTerm::Eq(v) => cmp(" = ", v),
        QbeTerm::Ne(v) => cmp(" <> ", v),
        QbeTerm::Lt(v) => cmp(" < ", v),
        QbeTerm::Le(v) => cmp(" <= ", v),
        QbeTerm::Gt(v) => cmp(" > ", v),
        QbeTerm::Ge(v) => cmp(" >= ", v),
        QbeTerm::Range(lo, hi) => {
            cmp(" BETWEEN ", lo);
            qb.push(" AND ");
            push_value(qb, hi);
        }
        QbeTerm::Like(parts) => {
            qb.push(col).push(collate).push(r" LIKE ");
            qb.push_bind(like_pattern(parts));
            qb.push(r" ESCAPE '\'");
        }
    }
}

/// 文字陣列：任一值符合即符合；`<>x` 為沒有任何值等於 x，`=` 為空陣列（同 QbeFilter::matches_set）
fn push_set_term(qb: &mut QueryBuilder<Postgres>, col: &'static str, term: &QbeTerm) {
    match term {
        QbeTerm::IsNull => {
            qb.push("cardinality(").push(col).push(") = 0");
        }
        QbeTerm::Ne(v) => {
            qb.push("NOT EXISTS (SELECT 1 FROM unnest(")
                .push(col)
                .push(r#") AS a(v) WHERE a.v COLLATE "C" = "#);
            push_value(qb, v);
            qb.push(")");
        }
        _ => {
            qb.push("EXISTS (SELECT 1 FROM unnest(")
                .push(col)
                .push(") AS a(v) WHERE ");
            push_term(qb, "a.v", true, term);
            qb.push(")");
        }
    }
}

fn push_value(qb: &mut QueryBuilder<Postgres>, v: &QbeValue) {
    match v {
        QbeValue::Text(s) => qb.push_bind(s.clone()),
        QbeValue::Integer(n) => qb.push_bind(*n),
        QbeValue::Date(d) => qb.push_bind(*d),
        QbeValue::Flag(b) => qb.push_bind(*b),
    };
}

/// `*` → `%`、`?` → `_`；字面的 `%` `_` `\` 以反斜線跳脫
fn like_pattern(parts: &[PatternPart]) -> String {
    let mut out = String::new();
    for p in parts {
        match p {
            PatternPart::AnyMany => out.push('%'),
            PatternPart::AnyOne => out.push('_'),
            PatternPart::Literal(s) => {
                for c in s.chars() {
                    if matches!(c, '%' | '_' | '\\') {
                        out.push('\\');
                    }
                    out.push(c);
                }
            }
        }
    }
    out
}

/// 資料權限的查詢範圍（p_zx.4gl:194-204）：` AND 建立者 = 目前使用者`、` AND 建立部門 = ANY(部門)`
pub(crate) fn push_scope(
    qb: &mut QueryBuilder<Postgres>,
    scope: &DataScope,
    owner_col: &'static str,
    dept_col: &'static str,
) {
    if let Some(owner) = &scope.owner {
        qb.push(" AND ").push(owner_col).push(" = ");
        qb.push_bind(owner.clone());
    }
    if let Some(depts) = &scope.depts {
        qb.push(" AND ").push(dept_col).push(" = ANY(");
        qb.push_bind(depts.clone());
        qb.push(")");
    }
}

/// 分頁查詢：`count` 與 `select` 為到 `WHERE TRUE` 為止的 SQL，`push_where` 加上篩選條件，
/// 依 `order_by`（如 ` ORDER BY u.user_id COLLATE "C"`）排序後取指定範圍；回傳資料列與總筆數
pub(crate) async fn query_page<R>(
    pool: &PgPool,
    count: &'static str,
    select: &'static str,
    push_where: impl Fn(&mut QueryBuilder<Postgres>) -> Result<(), RepoError>,
    order_by: &'static str,
    offset: u64,
    limit: u64,
) -> Result<(Vec<R>, u64), RepoError>
where
    R: for<'r> FromRow<'r, PgRow> + Send + Unpin,
{
    let mut qb = QueryBuilder::<Postgres>::new(count);
    push_where(&mut qb)?;
    let total: i64 = qb
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(db_err)?;

    let mut qb = QueryBuilder::<Postgres>::new(select);
    push_where(&mut qb)?;
    qb.push(order_by).push(" LIMIT ");
    qb.push_bind(i64::try_from(limit).unwrap_or(i64::MAX));
    qb.push(" OFFSET ");
    qb.push_bind(i64::try_from(offset).unwrap_or(i64::MAX));
    let rows = qb
        .build_query_as::<R>()
        .fetch_all(pool)
        .await
        .map_err(db_err)?;
    Ok((rows, u64::try_from(total).unwrap_or(0)))
}
