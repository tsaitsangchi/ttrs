//! Menu Maintenance 選單維護 use case（TipTop: p_zm / zm_file）與側欄樹狀選單（cl_create_4sm）
//! 規格：specs/features/azz/menu_maintenance.feature（MN1–MN16）
//!
//! - 維護：功能 query／insert／detail（單身新增、移除、上下移、重新編號）／reproduce／delete（ADR 0006）。
//! - 側欄：根節點為使用者的選單 zx05 → 權限類別的預設選單 zw03 → menu-main（MN9）；依序號排列；
//!   沒有權限或尚未移植的程式不顯示，沒有可顯示子項的目錄也不顯示（MN10、MN11）。

use std::sync::Arc;

use ttrs_domain::{
    DomainError,
    lang::{Lang, localized, non_empty},
    menu::{self, MAX_DEPTH, MenuItem},
    permission_class::ClassCode,
    program::{Program, ProgramNo},
    user::UserId,
};
use ttrs_shared::{MenuItemDto, MenuItemInput, MenuNodeDto, MenuSummaryDto};

use crate::{
    Actor, AppError, Authorizer,
    authorization::{Grant, PORTED_PROGRAMS},
    error::RepoError,
    ports::{MenuRepository, PermissionClassRepository, ProgramRepository, UserRepository},
};

/// 權限檢查用的程式代號
pub const PROGRAM: &str = "menu-maintenance";
/// 使用者與權限類別都沒有選單時的根目錄（MN9）
pub const DEFAULT_MENU: &str = "menu-main";

#[derive(Clone)]
pub struct MenuService {
    authz: Arc<dyn Authorizer>,
    menus: Arc<dyn MenuRepository>,
    programs: Arc<dyn ProgramRepository>,
    users: Arc<dyn UserRepository>,
    classes: Arc<dyn PermissionClassRepository>,
}

impl MenuService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        menus: Arc<dyn MenuRepository>,
        programs: Arc<dyn ProgramRepository>,
        users: Arc<dyn UserRepository>,
        classes: Arc<dyn PermissionClassRepository>,
    ) -> Self {
        Self {
            authz,
            menus,
            programs,
            users,
            classes,
        }
    }

    /// 目錄清單：模組 MENU 的程式與單身筆數（p_zm 查詢單頭，p_zm.4gl:121-180）
    pub async fn menus(&self, actor: &Actor, lang: &str) -> Result<Vec<MenuSummaryDto>, AppError> {
        self.authorize(actor, "query").await?;
        let lang = Lang::parse(lang)?;
        let all = self.menus.all_menu_items().await.map_err(infra)?;
        let mut out = Vec::new();
        for p in self.programs.list().await.map_err(infra)? {
            if !p.module.is_menu() {
                continue;
            }
            let count = all
                .iter()
                .filter(|i| i.menu == p.program_no.as_str())
                .count();
            out.push(MenuSummaryDto {
                menu_no: p.program_no.as_str().to_owned(),
                name: self.name(&p.program_no, &lang).await?,
                item_count: u32::try_from(count).unwrap_or(u32::MAX),
            });
        }
        Ok(out)
    }

    /// 目錄的單身，依序號排列
    pub async fn items(
        &self,
        actor: &Actor,
        menu: &str,
        lang: &str,
    ) -> Result<Vec<MenuItemDto>, AppError> {
        self.authorize(actor, "query").await?;
        let menu = self.menu(menu).await?;
        let lang = Lang::parse(lang)?;
        self.item_dtos(menu.program_no.as_str(), &lang).await
    }

    /// 新增單身（p_zm.4gl:637-930）：子項須為已登錄的程式（MN3）、不可形成迴圈（azz-702，MN4）、
    /// 序號與程式在同一目錄不可重複（-239，MN2、MN6）
    pub async fn add(
        &self,
        actor: &Actor,
        menu: &str,
        input: MenuItemInput,
    ) -> Result<Vec<MenuItemDto>, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?.add_row()?;
            let menu = self.menu(menu).await?;
            let child = self.program(&input.program_no).await?;
            let (m, c) = (menu.program_no.as_str(), child.program_no.as_str());
            let items = self.menus.menu_items(m).await.map_err(infra)?;
            if items.iter().any(|i| i.program == c) {
                return Err(DomainError::MenuItemDuplicate.into());
            }
            let all = self.menus.all_menu_items().await.map_err(infra)?;
            if menu::creates_cycle(&all, m, c) || depth(&all, m) + 1 > MAX_DEPTH {
                return Err(DomainError::MenuCycle.into());
            }
            let seq = menu::next_seq(&items, input.seq)?;
            let item = MenuItem {
                menu: m.to_owned(),
                seq,
                program: c.to_owned(),
            };
            match self.menus.insert_menu_item(&item).await {
                Ok(()) => {}
                Err(RepoError::Conflict) => return Err(DomainError::MenuSeqDuplicate.into()),
                Err(e) => return Err(infra(e)),
            }
            self.item_dtos(m, &Lang::default_lang()).await
        })
        .await
    }

    /// 移除單身（單身刪除列）：子項是還有單身的目錄時拒絕（MN17，避免整組作業從側欄消失）
    pub async fn remove(&self, actor: &Actor, menu: &str, program: &str) -> Result<(), AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?.delete_row()?;
            let menu = self.menu(menu).await?;
            if !self
                .menus
                .menu_items(program.trim())
                .await
                .map_err(infra)?
                .is_empty()
            {
                return Err(DomainError::MenuNotEmpty.into());
            }
            if self
                .menus
                .remove_menu_item(menu.program_no.as_str(), program.trim())
                .await
                .map_err(infra)?
            {
                Ok(())
            } else {
                Err(DomainError::MenuItemNotFound.into())
            }
        })
        .await
    }

    /// 上移（`up`）或下移（TTRS 新增，MN6）
    pub async fn shift(
        &self,
        actor: &Actor,
        menu: &str,
        program: &str,
        up: bool,
    ) -> Result<Vec<MenuItemDto>, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let menu = self.menu(menu).await?;
            let m = menu.program_no.as_str();
            let mut items = self.menus.menu_items(m).await.map_err(infra)?;
            menu::shift(&mut items, program.trim(), up)?;
            self.menus.set_menu_seqs(m, &items).await.map_err(infra)?;
            self.item_dtos(m, &Lang::default_lang()).await
        })
        .await
    }

    /// 重新編號（p_zm_b_y，p_zm.4gl:931-960）：10、20、30…
    pub async fn renumber(&self, actor: &Actor, menu: &str) -> Result<Vec<MenuItemDto>, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let menu = self.menu(menu).await?;
            let m = menu.program_no.as_str();
            let mut items = self.menus.menu_items(m).await.map_err(infra)?;
            menu::renumber(&mut items);
            self.menus.set_menu_seqs(m, &items).await.map_err(infra)?;
            self.item_dtos(m, &Lang::default_lang()).await
        })
        .await
    }

    /// 複製目錄（p_zm.4gl:1290-1345，MN7）：新目錄須為已登錄的 MENU 程式且還沒有單身
    pub async fn copy(&self, actor: &Actor, from: &str, to: &str) -> Result<(), AppError> {
        crate::change_log::run(actor, PROGRAM, "reproduce", async {
            self.authorize(actor, "reproduce").await?;
            let from = self.menu(from).await?;
            let to = self.menu(to).await?;
            let t = to.program_no.as_str();
            if !self.menus.menu_items(t).await.map_err(infra)?.is_empty() {
                return Err(DomainError::MenuAlreadyHasItems.into());
            }
            let items = self
                .menus
                .menu_items(from.program_no.as_str())
                .await
                .map_err(infra)?;
            let all = self.menus.all_menu_items().await.map_err(infra)?;
            for i in &items {
                if menu::creates_cycle(&all, t, &i.program) {
                    return Err(DomainError::MenuCycle.into());
                }
            }
            for i in items {
                let item = MenuItem {
                    menu: t.to_owned(),
                    ..i
                };
                self.menus.insert_menu_item(&item).await.map_err(infra)?;
            }
            Ok(())
        })
        .await
    }

    /// 刪除目錄的單身（p_zm.4gl:604-636）：仍是使用者的選單、權限類別的預設選單或其他目錄的子項時拒絕（MN8）
    pub async fn delete(&self, actor: &Actor, menu: &str) -> Result<(), AppError> {
        crate::change_log::run(actor, PROGRAM, "delete", async {
            self.authorize(actor, "delete").await?;
            let menu = self.menu(menu).await?;
            let m = menu.program_no.as_str();
            let all = self.menus.all_menu_items().await.map_err(infra)?;
            let used_by_user = self
                .users
                .list()
                .await
                .map_err(infra)?
                .iter()
                .any(|u| u.attrs.menu == m);
            let used_by_class = self
                .classes
                .list()
                .await
                .map_err(infra)?
                .iter()
                .any(|c| c.attrs.default_menu.as_deref() == Some(m));
            if used_by_user || used_by_class || all.iter().any(|i| i.program == m) {
                return Err(DomainError::MenuInUse.into());
            }
            self.menus.clear_menu(m).await.map_err(infra)
        })
        .await
    }

    /// 側欄樹狀選單（cl_create_4sm.4gl:190-246）：所有登入的使用者都可取得，不檢查功能權限
    pub async fn tree(&self, actor: &Actor, lang: &str) -> Result<Vec<MenuNodeDto>, AppError> {
        let lang = Lang::parse(lang)?;
        let root = self.root(actor).await?;
        let all = self.menus.all_menu_items().await.map_err(infra)?;
        let programs = self.programs.list().await.map_err(infra)?;
        let map = menu::children_map(&all);
        let mut cache = Cache {
            programs,
            visible: Vec::new(),
        };
        self.nodes(actor, &map, &root, &lang, 0, &mut cache).await
    }

    /// 根節點：使用者的選單 → 權限類別的預設選單 → menu-main（MN9）
    async fn root(&self, actor: &Actor) -> Result<String, AppError> {
        let user = match UserId::parse(&actor.user_id) {
            Ok(id) => self.users.get(&id).await.map_err(infra)?,
            Err(_) => None,
        };
        if let Some(m) = user.as_ref().and_then(|u| non_empty(u.attrs.menu.trim())) {
            return Ok(m);
        }
        let class = match ClassCode::parse(&actor.permission_class) {
            Ok(code) => self.classes.get(&code).await.map_err(infra)?,
            Err(_) => None,
        };
        Ok(class
            .and_then(|c| c.attrs.default_menu)
            .and_then(|m| non_empty(m.trim()))
            .unwrap_or_else(|| DEFAULT_MENU.to_owned()))
    }

    async fn nodes(
        &self,
        actor: &Actor,
        map: &std::collections::BTreeMap<&str, Vec<&MenuItem>>,
        menu: &str,
        lang: &Lang,
        level: usize,
        cache: &mut Cache,
    ) -> Result<Vec<MenuNodeDto>, AppError> {
        let mut out = Vec::new();
        if level >= MAX_DEPTH {
            return Ok(out);
        }
        for item in map.get(menu).map(Vec::as_slice).unwrap_or_default() {
            let Some(p) = cache
                .programs
                .iter()
                .find(|p| p.program_no.as_str() == item.program)
                .cloned()
            else {
                continue;
            };
            let no = p.program_no.as_str().to_owned();
            let name = self.name(&p.program_no, lang).await?.unwrap_or(no.clone());
            if p.module.is_menu() {
                // 子目錄：遞迴；沒有可顯示的子項時不顯示（MN10）
                let children =
                    Box::pin(self.nodes(actor, map, &no, lang, level + 1, cache)).await?;
                if !children.is_empty() {
                    out.push(MenuNodeDto {
                        program_no: no,
                        name,
                        module_code: p.module.as_str().to_owned(),
                        children,
                    });
                }
            } else if self.visible(actor, &no, cache).await? {
                out.push(MenuNodeDto {
                    program_no: no,
                    name,
                    module_code: p.module.as_str().to_owned(),
                    children: Vec::new(),
                });
            }
        }
        Ok(out)
    }

    /// 程式節點是否顯示：已移植（MN11）且有執行權限（MN10）
    async fn visible(&self, actor: &Actor, no: &str, cache: &mut Cache) -> Result<bool, AppError> {
        if let Some((_, v)) = cache.visible.iter().find(|(n, _)| n == no) {
            return Ok(*v);
        }
        let ported = PORTED_PROGRAMS.iter().any(|(p, ..)| *p == no);
        let v = ported && self.authz.effective(actor, no).await?.is_some();
        cache.visible.push((no.to_owned(), v));
        Ok(v)
    }

    async fn item_dtos(&self, menu: &str, lang: &Lang) -> Result<Vec<MenuItemDto>, AppError> {
        let mut out = Vec::new();
        for i in self.menus.menu_items(menu).await.map_err(infra)? {
            let no = ProgramNo::parse(&i.program).ok();
            let program = match &no {
                Some(n) => self.programs.get(n).await.map_err(infra)?,
                None => None,
            };
            out.push(MenuItemDto {
                seq: i.seq,
                name: match &no {
                    Some(n) => self.name(n, lang).await?,
                    None => None,
                },
                is_menu: program.is_some_and(|p| p.module.is_menu()),
                program_no: i.program,
            });
        }
        Ok(out)
    }

    async fn name(&self, no: &ProgramNo, lang: &Lang) -> Result<Option<String>, AppError> {
        let names = self.programs.names(no).await.map_err(infra)?;
        Ok(localized(&names, lang, |n| non_empty(&n.name)))
    }

    /// 目錄：已登錄、模組 MENU 的程式（MN3）
    async fn menu(&self, menu: &str) -> Result<Program, AppError> {
        let no = ProgramNo::parse(menu).map_err(|_| DomainError::MenuNotFound)?;
        self.programs
            .get(&no)
            .await
            .map_err(infra)?
            .filter(|p| p.module.is_menu())
            .ok_or_else(|| DomainError::MenuNotFound.into())
    }

    /// 子項：已登錄的程式（MN3）
    async fn program(&self, no: &str) -> Result<Program, AppError> {
        let no = ProgramNo::parse(no).map_err(|_| DomainError::ProgramNotFound)?;
        self.programs
            .get(&no)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::ProgramNotFound.into())
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
    }
}

/// 組樹狀時的快取：程式資料、程式是否顯示
struct Cache {
    programs: Vec<Program>,
    visible: Vec<(String, bool)>,
}

/// 目錄所在的層數（根為 0）：往上找包含它的目錄
fn depth(all: &[MenuItem], menu: &str) -> usize {
    let mut level = 0;
    let mut current = vec![menu.to_owned()];
    while level < MAX_DEPTH {
        let parents: Vec<String> = all
            .iter()
            .filter(|i| current.contains(&i.program))
            .map(|i| i.menu.clone())
            .collect();
        if parents.is_empty() {
            break;
        }
        current = parents;
        level += 1;
    }
    level
}

fn infra(e: RepoError) -> AppError {
    match e {
        RepoError::Stale => DomainError::RecordChanged.into(),
        e => AppError::Infra(e.to_string()),
    }
}
