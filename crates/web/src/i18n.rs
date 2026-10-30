//! 介面多語言（繁中 zh-TW、英文 en、簡中 zh-CN）
//! 規格：specs/features/azz/program_maintenance_i18n.feature L3、L4、L6
//!
//! - 以繁中原文為鍵：`t("程式代號")`，找不到翻譯時直接顯示繁中（新畫面不翻也能用）
//! - 語系存在瀏覽器 localStorage（L3）；切換時外框以語系為 key 重新載入畫面
//! - 錯誤訊息依錯誤代碼翻譯，未翻譯的代碼退回伺服器回傳的繁中訊息（L4）
//! - 使用者輸入的 QBE 條件值（如「明細表」）屬資料，不翻譯

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiLang {
    ZhTw,
    En,
    ZhCn,
}

impl UiLang {
    pub const ALL: [Self; 3] = [Self::ZhTw, Self::En, Self::ZhCn];

    /// 與後端語系代碼相同（DICTIONARY_LANGS）
    pub fn code(self) -> &'static str {
        match self {
            Self::ZhTw => "zh-TW",
            Self::En => "en",
            Self::ZhCn => "zh-CN",
        }
    }

    /// 切換選單上的名稱（固定以各自語言顯示）
    pub fn native_name(self) -> &'static str {
        match self {
            Self::ZhTw => "繁體中文",
            Self::En => "English",
            Self::ZhCn => "简体中文",
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.code() == code)
    }

    fn index(self) -> u8 {
        match self {
            Self::ZhTw => 0,
            Self::En => 1,
            Self::ZhCn => 2,
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
const STORAGE_KEY: &str = "ttrs.lang";

pub fn current() -> UiLang {
    match CURRENT.load(Ordering::Relaxed) {
        1 => UiLang::En,
        2 => UiLang::ZhCn,
        _ => UiLang::ZhTw,
    }
}

/// 切換語系並記在瀏覽器
pub fn set(lang: UiLang) {
    CURRENT.store(lang.index(), Ordering::Relaxed);
    #[cfg(target_arch = "wasm32")]
    if let Some(s) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = s.set_item(STORAGE_KEY, lang.code());
    }
}

/// 啟動時決定語系：網址 `?lang=en` 優先（方便分享連結），其次瀏覽器記住的語系，預設繁中
pub fn load() -> UiLang {
    let lang = saved().unwrap_or(UiLang::ZhTw);
    set(lang);
    lang
}

#[cfg(target_arch = "wasm32")]
fn saved() -> Option<UiLang> {
    let window = web_sys::window()?;
    let from_url = window.location().search().ok().and_then(|q| {
        q.trim_start_matches('?')
            .split('&')
            .find_map(|kv| kv.strip_prefix("lang="))
            .and_then(UiLang::from_code)
    });
    from_url.or_else(|| {
        window
            .local_storage()
            .ok()
            .flatten()
            .and_then(|s| s.get_item(STORAGE_KEY).ok().flatten())
            .and_then(|c| UiLang::from_code(&c))
    })
}

/// 桌面/行動版目前沒有持久化設定，固定繁中（導入登入後改存使用者設定，L3）
#[cfg(not(target_arch = "wasm32"))]
fn saved() -> Option<UiLang> {
    None
}

/// 翻譯：以繁中原文為鍵
pub fn t(zh: &'static str) -> &'static str {
    let lang = current();
    if lang == UiLang::ZhTw {
        return zh;
    }
    TEXTS
        .iter()
        .find(|(k, _, _)| *k == zh)
        .map(|(_, en, cn)| if lang == UiLang::En { *en } else { *cn })
        .unwrap_or(zh)
}

/// 含 `{}` 佔位的翻譯，依序代入參數
pub fn tf(zh: &'static str, args: &[&str]) -> String {
    let mut out = String::new();
    let mut rest = t(zh);
    for a in args {
        match rest.split_once("{}") {
            Some((head, tail)) => {
                out.push_str(head);
                out.push_str(a);
                rest = tail;
            }
            None => break,
        }
    }
    out.push_str(rest);
    out
}

/// 錯誤訊息：英文、簡中依錯誤代碼翻譯；繁中與未翻譯的代碼使用伺服器訊息（較具體）
pub fn error_text(code: &str, server_message: &str) -> String {
    let lang = current();
    if lang == UiLang::ZhTw {
        return server_message.to_owned();
    }
    ERRORS
        .iter()
        .find(|(c, _, _)| *c == code)
        .map(|(_, en, cn)| (if lang == UiLang::En { *en } else { *cn }).to_owned())
        .unwrap_or_else(|| server_message.to_owned())
}

/// (繁中, English, 简体中文)
const TEXTS: &[(&str, &str, &str)] = &[
    // 外框
    ("首頁", "Home", "首页"),
    ("庫存管理 (aim)", "Inventory (aim)", "库存管理 (aim)"),
    (
        "aimi100 料件基本資料",
        "aimi100 Item Master",
        "aimi100 料件基本资料",
    ),
    ("系統管理 (azz)", "System Admin (azz)", "系统管理 (azz)"),
    ("程式維護", "Program Maintenance", "程序维护"),
    ("資料字典維護", "Data Dictionary", "数据字典维护"),
    ("語言", "Language", "语言"),
    // 動作
    ("確定", "OK", "确定"),
    ("放棄", "Cancel", "放弃"),
    ("查詢", "Query", "查询"),
    ("新增", "New", "新增"),
    ("修改", "Edit", "修改"),
    ("刪除", "Delete", "删除"),
    ("複製", "Copy", "复制"),
    ("複製為新代號", "Copy as new ID", "复制为新代号"),
    ("跳至", "Go", "跳至"),
    ("跳至第幾筆", "Go to record #", "跳至第几笔"),
    ("第一筆", "First", "第一笔"),
    ("上一筆", "Previous", "上一笔"),
    ("下一筆", "Next", "下一笔"),
    ("最後一筆", "Last", "最后一笔"),
    ("上一頁", "Previous page", "上一页"),
    ("下一頁", "Next page", "下一页"),
    ("設定權限", "Grant", "设定权限"),
    ("載入中…", "Loading…", "加载中…"),
    // 訊息
    ("已儲存 {}", "Saved {}", "已保存 {}"),
    (
        "查無符合條件的資料",
        "No matching records",
        "查无符合条件的数据",
    ),
    (
        "共 {} 筆符合條件",
        "{} matching records",
        "共 {} 笔符合条件",
    ),
    (
        "已刪除 {}（含名稱、權限、選單）",
        "Deleted {} (with names, permissions and menus)",
        "已删除 {}（含名称、权限、菜单）",
    ),
    ("已將 {} 複製為 {}", "Copied {} to {}", "已将 {} 复制为 {}"),
    (
        "已為 {} 設定權限",
        "Permission granted for {}",
        "已为 {} 设定权限",
    ),
    ("第 {} / {} 筆", "Record {} of {}", "第 {} / {} 笔"),
    ("共 {} 筆", "{} records", "共 {} 笔"),
    (
        "請輸入 1 到 {} 之間的筆數",
        "Enter a record number from 1 to {}",
        "请输入 1 到 {} 之间的笔数",
    ),
    (
        "依查詢條件共 {} 筆",
        "{} records matching the query",
        "依查询条件共 {} 笔",
    ),
    ("全部共 {} 筆", "{} records in total", "全部共 {} 笔"),
    (
        "，點選一筆以檢視或修改",
        " — click a row to view or edit",
        "，点选一笔以查看或修改",
    ),
    (
        "無法載入程式：{}",
        "Failed to load programs: {}",
        "无法加载程序：{}",
    ),
    ("查無資料", "No data", "查无数据"),
    ("第 {} / {} 頁", "Page {} of {}", "第 {} / {} 页"),
    (
        "尚未授權任何權限類別",
        "No permission class granted yet",
        "尚未授权任何权限类别",
    ),
    (
        "系統管理 / Program Maintenance",
        "System Admin / Program Maintenance",
        "系统管理 / Program Maintenance",
    ),
    (
        "查詢模式：在欄位輸入條件後按「確定」。例：aim*（開頭）、aimi10?（單一字元）、a|b（或）、100:200（範圍）、>x、<>x、=（空值）；不輸入即查全部。",
        "Query mode: enter conditions in the fields, then press OK. e.g. aim* (prefix), aimi10? (one char), a|b (or), 100:200 (range), >x, <>x, = (empty). Leave blank to list all.",
        "查询模式：在栏位输入条件后按「确定」。例：aim*（开头）、aimi10?（单一字元）、a|b（或）、100:200（范围）、>x、<>x、=（空值）；不输入即查全部。",
    ),
    // 頁籤與群組
    ("基本資料", "Basic", "基本资料"),
    ("資料擁有者", "Owner", "数据拥有者"),
    ("資料、權限設定", "Data & Permissions", "数据、权限设定"),
    ("列印設定", "Print", "打印设定"),
    ("閒置設定", "Idle", "闲置设定"),
    ("其他設定", "Other", "其他设定"),
    ("可執行功能", "Executable Actions", "可执行功能"),
    ("設定", "Settings", "设定"),
    ("執行設定", "Execution", "执行设定"),
    ("群組權限", "Group Permissions", "群组权限"),
    ("執行選項", "Options", "执行选项"),
    ("其他備註", "Memo", "其他备注"),
    ("程式清單", "Programs", "程序清单"),
    // 欄位
    ("程式代號", "Program ID", "程序代号"),
    ("模組代號", "Module", "模块代号"),
    ("模組", "Module", "模块"),
    ("程式名稱", "Program Name", "程序名称"),
    ("資料所有者", "Created by", "数据所有者"),
    ("最後修改者", "Modified by", "最后修改者"),
    ("所屬部門", "Department", "所属部门"),
    ("建立/修改日期", "Modified date", "建立/修改日期"),
    ("功能代碼", "Action codes", "功能代码"),
    (
        "功能代碼（不可查詢）",
        "Action codes (not queryable)",
        "功能代码（不可查询）",
    ),
    ("程式群組", "Program Group", "程序群组"),
    ("視窗型態", "Window Style", "窗口型态"),
    ("程式類別", "Program Category", "程序类别"),
    ("類別", "Category", "类别"),
    ("依代號自動帶入", "From program ID", "依代号自动带入"),
    ("設計日期", "Design Date", "设计日期"),
    ("UNIX 指令", "UNIX Command", "UNIX 指令"),
    (
        "標準代號自動產生",
        "Generated for standard IDs",
        "标准代号自动产生",
    ),
    ("固定列印條件", "Fixed Print Condition", "固定打印条件"),
    ("固定列印方式", "Fixed Print Method", "固定打印方式"),
    (
        "傳遞帳別參數給程式",
        "Pass book no. to program",
        "传递账别参数给程序",
    ),
    (
        "傳遞帳別參數給程式（僅總帳模組）",
        "Pass book no. to program (GL modules only)",
        "传递账别参数给程序（仅总账模块）",
    ),
    ("權限類別", "Permission Class", "权限类别"),
    ("報表類型", "Report Type", "报表类型"),
    ("報表抬頭", "Report Title", "报表抬头"),
    ("報表副檔名", "Report File Extension", "报表扩展名"),
    (
        "固定副檔名（*.out）",
        "Fixed (*.out)",
        "固定扩展名（*.out）",
    ),
    (
        "動態副檔名（*.xxr）",
        "Dynamic (*.xxr)",
        "动态扩展名（*.xxr）",
    ),
    ("報表寬度", "Report Width", "报表宽度"),
    ("已列印次數", "Printed Times", "已打印次数"),
    ("延後列印次數", "Delayed Prints", "延后打印次数"),
    (
        "報表結束時列印查詢條件",
        "Print query conditions at report end",
        "报表结束时打印查询条件",
    ),
    ("閒置控制", "Idle Control", "闲置控制"),
    ("不控制", "No control", "不控制"),
    ("由程式設定閒置時間", "Set by program", "由程序设定闲置时间"),
    ("依整體系統設定", "System setting", "依整体系统设定"),
    ("閒置時間（秒）", "Idle Time (sec)", "闲置时间（秒）"),
    ("閒置處理", "Idle Process", "闲置处理"),
    ("僅提出警告", "Warning only", "仅提出警告"),
    ("警告後關閉程式", "Warn, then close", "警告后关闭程序"),
    ("直接關閉程式", "Close program", "直接关闭程序"),
    (
        "錯誤訊息寫入記錄檔",
        "Log error messages to file",
        "错误信息写入日志文件",
    ),
    (
        "維護作業可修改鍵值",
        "Allow key update in maintenance",
        "维护作业可修改键值",
    ),
    (
        "SQL 執行計畫（Set explain on）",
        "SQL explain (Set explain on)",
        "SQL 执行计划（Set explain on）",
    ),
    ("備註", "Memo", "备注"),
    ("最後修改日期", "Last Modified", "最后修改日期"),
    // 選項（下拉顯示文字；QBE 輸入值不翻）
    ("I：輸入/建立", "I: Input", "I：输入/建立"),
    ("M：選單", "M: Menu", "M：菜单"),
    ("O：其他", "O: Other", "O：其他"),
    ("P：處理/批次", "P: Batch", "P：处理/批次"),
    ("Q：查詢", "Q: Query", "Q：查询"),
    ("R：報表", "R: Report", "R：报表"),
    ("S：系統參數", "S: Parameter", "S：系统参数"),
    ("T：交易", "T: Transaction", "T：交易"),
    ("U：更新", "U: Update", "U：更新"),
    ("憑證", "Voucher", "凭证"),
    ("明細表", "Detail", "明细表"),
    ("統計表", "Statistics", "统计表"),
    ("追蹤表", "Tracking", "追踪表"),
    ("無", "None", "无"),
    ("Y 或 N", "Y or N", "Y 或 N"),
    (
        "例：132、100:200、>80",
        "e.g. 132, 100:200, >80",
        "例：132、100:200、>80",
    ),
    (
        "2026/10/29 或 2026/10/01:2026/10/31",
        "2026/10/29 or 2026/10/01:2026/10/31",
        "2026/10/29 或 2026/10/01:2026/10/31",
    ),
];

/// (錯誤代碼, English, 简体中文)；繁中直接用伺服器訊息
const ERRORS: &[(&str, &str, &str)] = &[
    (
        "PROGRAM_NO_EMPTY",
        "Program ID is required.",
        "程序代号不可空白。",
    ),
    (
        "PROGRAM_NO_TOO_LONG",
        "Program ID is too long (max 20).",
        "程序代号长度不可超过 20 码。",
    ),
    (
        "PROGRAM_ALREADY_EXISTS",
        "This program ID already exists.",
        "程序已存在。",
    ),
    ("PROGRAM_NOT_FOUND", "Program not found.", "程序不存在。"),
    ("MODULE_EMPTY", "Module is required.", "所属模块不可空白。"),
    ("MODULE_NOT_FOUND", "Module not found.", "模块不存在。"),
    (
        "MODULE_NOT_ALLOWED",
        "Shared library modules cannot own a program.",
        "共用程序模块不可作为作业程序的所属模块。",
    ),
    (
        "REPORT_TYPE_REQUIRED",
        "Report programs must have a report type.",
        "报表程序必须指定报表类型。",
    ),
    (
        "MENU_HAS_NO_PERMISSION",
        "Menus are not executable and cannot have permissions.",
        "菜单不是可执行程序，不可设定权限。",
    ),
    (
        "PERMISSION_CLASS_NOT_FOUND",
        "Permission class not found.",
        "权限类别不存在。",
    ),
    (
        "QUERY_DEFINITION_NOT_FOUND",
        "Programs run via p_query need a query definition first.",
        "经由 p_query 执行的程序必须先建立查询定义。",
    ),
    (
        "FIELD_TOO_LONG",
        "A field exceeds its maximum length.",
        "栏位长度超过上限。",
    ),
    (
        "LANGUAGE_NOT_SUPPORTED",
        "Unsupported language (zh-TW, en, zh-CN).",
        "不支持的语系（zh-TW、en、zh-CN）。",
    ),
    (
        "QBE_INVALID_SYNTAX",
        "Invalid query condition.",
        "查询条件格式错误。",
    ),
    (
        "QBE_INVALID_NUMBER",
        "Query condition is not a number.",
        "查询条件不是数字。",
    ),
    (
        "QBE_INVALID_DATE",
        "Query condition is not a valid date (YYYY/MM/DD).",
        "查询条件不是正确的日期（YYYY/MM/DD）。",
    ),
    (
        "QBE_FIELD_NOT_ALLOWED",
        "This field cannot be queried.",
        "此栏位不可作为查询条件。",
    ),
    (
        "INFRA_ERROR",
        "System error. Please try again later.",
        "系统错误，请稍后再试。",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_duplicate_keys() {
        for (i, (k, _, _)) in TEXTS.iter().enumerate() {
            assert!(
                !TEXTS[i + 1..].iter().any(|(k2, _, _)| k2 == k),
                "重複的翻譯鍵：{k}"
            );
        }
    }

    #[test]
    fn placeholders_match() {
        for (k, en, cn) in TEXTS {
            let n = k.matches("{}").count();
            assert_eq!(en.matches("{}").count(), n, "{k}");
            assert_eq!(cn.matches("{}").count(), n, "{k}");
        }
    }
}
