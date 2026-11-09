//! 選單目錄（menu_item ← TipTop zm_file；menu_maintenance.feature）。共用 schema
use async_trait::async_trait;
use ttrs_app::{error::RepoError, ports::MenuRepository};
use ttrs_domain::menu::MenuItem;

use crate::{PgStore, pg::db_err};

type Row = (String, i32, String);

fn item((menu, seq, program): Row) -> MenuItem {
    MenuItem { menu, seq, program }
}

#[async_trait]
impl MenuRepository for PgStore {
    async fn all_menu_items(&self) -> Result<Vec<MenuItem>, RepoError> {
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT menu_no, seq, program_no FROM menu_item ORDER BY menu_no COLLATE \"C\", seq",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows.into_iter().map(item).collect())
    }

    async fn menu_items(&self, menu: &str) -> Result<Vec<MenuItem>, RepoError> {
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT menu_no, seq, program_no FROM menu_item WHERE menu_no = $1 ORDER BY seq",
        )
        .bind(menu)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows.into_iter().map(item).collect())
    }

    async fn insert_menu_item(&self, i: &MenuItem) -> Result<(), RepoError> {
        sqlx::query("INSERT INTO menu_item (menu_no, seq, program_no) VALUES ($1, $2, $3)")
            .bind(&i.menu)
            .bind(i.seq)
            .bind(&i.program)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn remove_menu_item(&self, menu: &str, program: &str) -> Result<bool, RepoError> {
        let n = sqlx::query("DELETE FROM menu_item WHERE menu_no = $1 AND program_no = $2")
            .bind(menu)
            .bind(program)
            .execute(&self.pool)
            .await
            .map_err(db_err)?
            .rows_affected();
        Ok(n > 0)
    }

    async fn set_menu_seqs(&self, menu: &str, items: &[MenuItem]) -> Result<(), RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        // 主鍵 (menu_no, seq) 可延後檢查（migration 0026）：交換序號時交易結束才檢查
        sqlx::query("SET CONSTRAINTS menu_item_pkey DEFERRED")
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        for i in items {
            sqlx::query(
                "UPDATE menu_item SET seq = $3 WHERE menu_no = $1 AND program_no = $2 AND seq <> $3",
            )
            .bind(menu)
            .bind(&i.program)
            .bind(i.seq)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        }
        tx.commit().await.map_err(db_err)
    }

    async fn clear_menu(&self, menu: &str) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM menu_item WHERE menu_no = $1")
            .bind(menu)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }
}
