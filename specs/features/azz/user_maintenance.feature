# language: zh-TW
# TipTop program: p_zx 部門使用者資料維護作業 (department user maintenance) / zx_file (users), zxy_file (allowed plants per user)
# New system name: User Maintenance 使用者維護
# Rule source: tiptop/azz/4gl/p_zx.4gl, tiptop/azz/4fd/p_zx.4fd (p_zx.per), tiptop/azz/sch/{zx,zxy}_file.sch;
#           no topcust override. Line numbers are noted on each scenario.
# Login and permission framework: docs/adr/0006-authentication-authorization.md (company SSO; TipTop password fields not ported)
# Status: U1–U9 confirmed (Key User, 2026-10-30; all as proposed, U9 keeps TipTop's direct delete); entering Red
#
# Scope:
#   - Maintain users: user code, name, department, permission class, user menu, plants, company account, email,
#     preferred language, expiry date, active code; add / query / modify / delete / copy / invalidate
#   - The user's allowed plants (zxy): kept in sync automatically when the default plant is set (TipTop p_zx behavior);
#     full allowed-plant maintenance (TipTop p_zxy) is U7
#   - Not ported (ADR 0006): password zx10, password changed on zx16, password use count zx17,
#     force change at next login zx19, failed attempts zx20, unlock button (account policy gbt_file);
#     these are handled by the company IdP
#   - Not ported (TipTop desktop-specific): user flow zx11 (udm tree gbk), refresh menu by permission zx12,
#     dynamic language switching zx15, multilingual function zx14, menu file regeneration (cl_create_4sm, redo_all_menu),
#     MDM integration (aws_mdmdata), Express report permission
#   - Data permissions on query (only own / same department / same permission group, p_zx.4gl:194-204) are enforced by the shared
#     Authorizer (ADR 0006 §4), not specced separately for this program
#   - When the permission class changes, the user's permission rows with the old class (zxw) move to the new class (p_zx.4gl:1003-1018):
#     implemented together with p_zxw user permissions
#
# Decisions (all ✅ confirmed, 2026-10-30):
#   U1. User code cannot be changed: TipTop allows renaming the key on modify only when system parameter g_chkey = Y (p_zx.4gl:1514-1520).
#       The new system stores the user code in created_by and audit of every table, so I recommend never allowing it to change (USER_ID_IMMUTABLE).
#   U2. Company account (UPN) is optional: a user with no company account can be created but can't log in.
#       The account must be unique (IDP_ACCOUNT_IN_USE). The bulk-binding rule waits for IT's answer to Q2 in ADR 0006.
#   U3. Department: TipTop only warns when the department isn't in the department master gem_file and still saves (p_zx.4gl:566-580).
#       Superseded 2026-11-04: the department must exist and be active in the user's default plant
#       (user_maintenance_department.feature UD1–UD6).
#   U4. Permission class must exist (PERMISSION_CLASS_NOT_FOUND). TipTop also rejects inactive classes (azz-218, p_zx.4gl:597-602);
#       the new permission class has no active code yet, so that check is added when p_zw is ported.
#       Defaulting the user menu from the class's default menu zw03 (p_zx.4gl:585-588) is likewise added with p_zw.
#   U5. User name is not multilingual: TipTop can maintain zx02 per language (p_itemname_update),
#       but a person's name is usually not translated, so the new system stores one name (no ADR 0005 _text table).
#   U6. Preferred language: zx06 0/1/2 → zh-TW / en / zh-CN (ADR 0005 L5); after login it becomes the user's UI language
#       (ADR 0005's "user setting" step).
#   U7. Allowed plants: TipTop maintains them in a separate p_zxy program. I recommend a lower "allowed plants" tab
#       inside user maintenance (the p_zxy spec is written separately and specs that tab's add/remove rules).
#   U8. Email: TipTop has no format check. I recommend a basic format check (USER_EMAIL_INVALID), since notifications and SSO matching will use it.
#   U9. Delete: TipTop deletes zx together with zxw and zxy directly (p_zx.4gl:1246-1249) and doesn't check whether the user has data.
#       I recommend keeping that behavior (data created_by keeps the original code for audit) and pointing out on screen that invalidate is preferred;
#       or should deleting a user who has logged in be refused?

功能: 使用者維護（User Maintenance）
  身為 系統管理員
  我想要 維護可登入 TTRS 的使用者、所屬部門、權限類別與可使用的營運中心
  以便 公司帳號登入後依 TipTop 相同的權限規則使用系統

  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"
    而且 權限類別 "CLASS1" 已存在
    而且 營運中心 "TW01" 已啟用
    而且 營運中心 "CN01" 已啟用
    而且 選單程式 "menu-main" 已存在
    而且 "TW01" 已有部門 "SAL"
    而且 "CN01" 已有部門 "SAL"

  # p_zx.4gl:441-455 新增預設：語言別 0、非多營運中心、資料有效；資料所有者／部門＝目前使用者／部門
  # p_zx.4gl:490-501 預設營運中心不在允許清單時自動加入
  場景: 新增使用者帶入預設值並加入預設營運中心
    當 使用者新增使用者 "u001"，名稱 "王小明"，部門 "SAL"，權限類別 "CLASS1"，選單 "menu-main"，預設營運中心 "TW01"
    那麼 操作應成功
    而且 使用者 "u001" 應為:
      | 欄位           | 值      |
      | 偏好語言       | zh-TW   |
      | 多營運中心     | 否      |
      | 資料有效       | 是      |
      | 資料所有者     | tiptop  |
      | 資料所有部門   | MIS     |
      | 建立日期       | 今天    |
    而且 使用者 "u001" 可使用的營運中心應為 "TW01"

  # p_zx.4gl:548-559 鍵值不可重複（-239）
  場景: 使用者代號不可重複
    假設 已有使用者 "u001"
    當 使用者新增使用者 "u001"，名稱 "李大華"，部門 "SAL"，權限類別 "CLASS1"，選單 "menu-main"，預設營運中心 "TW01"
    那麼 應出現錯誤 "USER_ALREADY_EXISTS"

  # p_zx.per:106 zx01 NOT NULL；zx_file.sch zx01 varchar2(10)
  場景大綱: 使用者代號必填且最多 10 碼
    當 使用者新增使用者 "<代號>"，名稱 "測試"，部門 "SAL"，權限類別 "CLASS1"，選單 "menu-main"，預設營運中心 "TW01"
    那麼 應出現錯誤 "<錯誤>"

    例子:
      | 代號        | 錯誤               |
      |             | USER_ID_EMPTY      |
      | u0123456789 | USER_ID_TOO_LONG   |

  # p_zx.4gl:582-596 權限類別必須存在
  場景: 權限類別必須存在
    當 使用者新增使用者 "u002"，名稱 "測試"，部門 "SAL"，權限類別 "NOCLASS"，選單 "menu-main"，預設營運中心 "TW01"
    那麼 應出現錯誤 "PERMISSION_CLASS_NOT_FOUND"

  # p_zx.4gl:611-619 自定目錄必須是模組 MENU 的程式
  場景: 使用者選單必須是選單程式
    假設 模組 "AIM" 已存在
    而且 已有程式 "item-maintenance" 模組 "AIM"
    當 使用者新增使用者 "u002"，名稱 "測試"，部門 "SAL"，權限類別 "CLASS1"，選單 "item-maintenance"，預設營運中心 "TW01"
    那麼 應出現錯誤 "USER_MENU_INVALID"

  # p_zx.4gl:630-638 預設營運中心必須存在（azp_file）
  場景: 預設營運中心必須存在
    當 使用者新增使用者 "u002"，名稱 "測試"，部門 "SAL"，權限類別 "CLASS1"，選單 "menu-main"，預設營運中心 "XX99"
    那麼 應出現錯誤 "PLANT_NOT_FOUND"

  # p_zx.4gl:621-628 非多營運中心使用者不可有多個允許的營運中心（azz-057）
  場景: 有多個營運中心的使用者不可改為非多營運中心
    假設 已有多營運中心使用者 "u001"，可使用營運中心 "TW01"、"CN01"
    當 使用者將使用者 "u001" 改為非多營運中心
    那麼 應出現錯誤 "USER_SINGLE_PLANT"

  # p_zx.4gl:1039-1060 非多營運中心：改預設營運中心即取代唯一的允許營運中心
  場景: 非多營運中心使用者變更預設營運中心
    假設 已有使用者 "u001"
    當 使用者將使用者 "u001" 的預設營運中心改為 "CN01"
    那麼 操作應成功
    而且 使用者 "u001" 可使用的營運中心應為 "CN01"

  # p_zx.4gl:1027-1038 多營運中心：新的預設營運中心不在清單時加入
  場景: 多營運中心使用者變更預設營運中心
    假設 已有多營運中心使用者 "u001"，可使用營運中心 "TW01"
    當 使用者將使用者 "u001" 的預設營運中心改為 "CN01"
    那麼 操作應成功
    而且 使用者 "u001" 可使用的營運中心應為 "TW01、CN01"

  # p_zx.4gl:977-981 存檔時依失效日期決定資料有效碼
  場景大綱: 存檔時依失效日期決定是否有效
    假設 已有使用者 "u001"
    當 使用者將使用者 "u001" 的失效日期設為 "<失效日期>"
    那麼 使用者 "u001" 的資料有效應為 "<有效>"

    例子:
      | 失效日期 | 有效 |
      | 昨天     | 否   |
      | 今天     | 否   |
      | 明天     | 是   |
      |          | 是   |

  # p_zx.4gl:1123-1135 無效／有效切換：停用時失效日期設為今天，啟用時清空
  場景: 停用與重新啟用使用者
    假設 已有使用者 "u001"
    當 使用者停用使用者 "u001"
    那麼 使用者 "u001" 的資料有效應為 "否"
    而且 使用者 "u001" 的失效日期應為 "今天"
    當 使用者啟用使用者 "u001"
    那麼 使用者 "u001" 的資料有效應為 "是"
    而且 使用者 "u001" 的失效日期應為 ""

  # p_zx.4gl:953-954 修改時記錄修改者與日期
  場景: 修改使用者記錄修改者與日期
    假設 已有使用者 "u001"
    當 使用者將使用者 "u001" 的名稱改為 "王大明"
    那麼 使用者 "u001" 的最後修改者應為 "tiptop"
    而且 使用者 "u001" 的最後修改日期應為 "今天"

  # U1：使用者代號不可修改
  場景: 使用者代號不可修改
    假設 已有使用者 "u001"
    當 使用者將使用者 "u001" 的代號改為 "u009"
    那麼 應出現錯誤 "USER_ID_IMMUTABLE"

  # p_zx.4gl:1246-1249 刪除使用者連同允許營運中心（及使用者權限 zxw）
  場景: 刪除使用者連同可使用的營運中心
    假設 已有多營運中心使用者 "u001"，可使用營運中心 "TW01"、"CN01"
    當 使用者刪除使用者 "u001"
    那麼 操作應成功
    而且 使用者 "u001" 應不存在
    而且 使用者 "u001" 應沒有可使用的營運中心

  # p_zx.4gl:1343-1349、1360-1379 複製：新代號、名稱清空、資料所有者為目前使用者、加入預設營運中心
  場景: 複製使用者
    假設 已有使用者 "u001"，公司帳號 "u001@example.com"
    當 使用者將使用者 "u001" 複製為 "u002"
    那麼 操作應成功
    而且 使用者 "u002" 的名稱應為 ""
    而且 使用者 "u002" 的公司帳號應為 ""
    而且 使用者 "u002" 的權限類別應為 "CLASS1"
    而且 使用者 "u002" 的資料所有者應為 "tiptop"
    而且 使用者 "u002" 可使用的營運中心應為 "TW01"

  # ADR 0006 §2、U2：公司帳號不可重複
  場景: 公司帳號不可重複綁定
    假設 已有使用者 "u001"，公司帳號 "u001@example.com"
    當 使用者新增使用者 "u002"，名稱 "測試"，部門 "SAL"，權限類別 "CLASS1"，選單 "menu-main"，預設營運中心 "TW01"，公司帳號 "U001@example.com"
    那麼 應出現錯誤 "IDP_ACCOUNT_IN_USE"

  # U6：偏好語言只能是支援的語系
  場景: 偏好語言必須是支援的語系
    假設 已有使用者 "u001"
    當 使用者將使用者 "u001" 的偏好語言改為 "ja"
    那麼 應出現錯誤 "LANGUAGE_NOT_SUPPORTED"

  # U8：電子郵件格式
  場景: 電子郵件格式不正確
    假設 已有使用者 "u001"
    當 使用者將使用者 "u001" 的電子郵件改為 "not-an-email"
    那麼 應出現錯誤 "USER_EMAIL_INVALID"
