//! 報表輸出（docs/adr/0011-reports-and-printing.md）：ReportData → PDF（Typst，內嵌 Noto Sans TC／SC）、Excel（rust_xlsxwriter）。
//! 字型隨程式附在 crates/infra/reports/fonts（SIL OFL 1.1，DR9）。

use rust_xlsxwriter::{Format, Workbook};
use ttrs_app::report::ReportData;
use typst_as_lib::TypstEngine;
use typst_layout::PagedDocument;

static FONT_TC: &[u8] = include_bytes!("../reports/fonts/NotoSansTC-Regular.otf");
static FONT_SC: &[u8] = include_bytes!("../reports/fonts/NotoSansSC-Regular.otf");

/// Typst 字串常值（"…"）
fn lit(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// 頁次格式（含 {page}、{total}）→ Typst 內容；只在 context 中使用
fn page_expr(template: &str) -> String {
    let mut parts = Vec::new();
    let mut rest = template;
    while let Some(i) = rest.find('{') {
        if i > 0 {
            parts.push(lit(&rest[..i]));
        }
        let Some(j) = rest[i..].find('}') else { break };
        match &rest[i + 1..i + j] {
            "page" => parts.push("counter(page).display()".to_owned()),
            "total" => parts.push("str(counter(page).final().first())".to_owned()),
            other => parts.push(lit(other)),
        }
        rest = &rest[i + j + 1..];
    }
    if !rest.is_empty() {
        parts.push(lit(rest));
    }
    if parts.is_empty() {
        return "[]".to_owned();
    }
    parts.join(" + ")
}

fn text_lang(r: &ReportData) -> (&'static str, &'static str) {
    match r.lang.as_str() {
        "en" => ("en", "us"),
        "zh-CN" => ("zh", "cn"),
        _ => ("zh", "tw"),
    }
}

/// 報表的 Typst 原始碼（A4 直式，DR8）：表頭（報表名稱、營運中心）、表格（標題列每頁重複）、
/// 頁尾（列印人、列印時間、第幾頁／共幾頁）、zz05 查詢條件（DR6、DR7）
pub fn typst_source(r: &ReportData) -> String {
    let (lang, region) = text_lang(r);
    let l = &r.labels;
    let plant = format!("{}：{} {}", l.plant, r.plant, r.plant_name);
    let printed = format!(
        "{}：{}　{}：{}",
        l.printed_by, r.printed_by, l.printed_at, r.printed_at
    );
    let headers: Vec<String> = r
        .columns
        .iter()
        .map(|c| format!("[*#{}*]", lit(c)))
        .collect();
    let cells: Vec<String> = r
        .rows
        .iter()
        .flat_map(|row| row.iter().map(|v| format!("[#{}]", lit(v))))
        .collect();
    let mut src = format!(
        r#"#set page(
  paper: "a4",
  margin: (x: 1.5cm, top: 2.4cm, bottom: 2cm),
  header: grid(columns: (1fr, auto), align: (left + bottom, right + bottom),
    text(size: 13pt, weight: "bold")[#{title}], text(size: 9pt)[#{plant}]),
  footer: context grid(columns: (1fr, auto), align: (left, right),
    text(size: 8pt)[#{printed}], text(size: 8pt)[#({page})]),
)
#set text(font: ("Noto Sans TC", "Noto Sans SC"), size: 9pt, lang: "{lang}", region: "{region}")
#table(
  columns: (1fr,) * {n},
  stroke: 0.5pt + luma(160),
  inset: (x: 4pt, y: 3pt),
  fill: (_, y) => if y == 0 {{ luma(230) }},
  table.header({headers}),
  {cells}
)
"#,
        title = lit(&r.title),
        plant = lit(&plant),
        printed = lit(&printed),
        page = page_expr(&l.page),
        n = r.columns.len().max(1),
        headers = headers.join(", "),
        cells = cells.join(", "),
    );
    if let Some(c) = &r.condition {
        src.push_str(&format!(
            "#v(8pt)\n#text(size: 8pt)[#{}]\n",
            lit(&format!("{}：{c}", l.condition))
        ));
    }
    src
}

pub fn render_pdf(r: &ReportData) -> Result<Vec<u8>, String> {
    let engine = TypstEngine::builder()
        .main_file(typst_source(r))
        .fonts([FONT_TC, FONT_SC])
        .build();
    let compiled = engine.compile();
    // 字型名稱對不上時 Typst 只發警告並改用替代字型（中文會缺字），視為錯誤
    if let Some(w) = compiled
        .warnings
        .iter()
        .find(|w| w.message.contains("unknown font family"))
    {
        return Err(format!("報表字型設定錯誤：{}", w.message));
    }
    let doc: PagedDocument = compiled
        .output
        .map_err(|e| format!("報表排版失敗：{e:?}"))?;
    typst_pdf::pdf(&doc, &typst_pdf::PdfOptions::default())
        .map_err(|e| format!("產生 PDF 失敗：{e:?}"))
}

/// Excel：一列一筆、欄名為標題列、凍結標題列；有查詢條件時另放「條件」工作表（DR5、DR7）
pub fn render_xlsx(r: &ReportData) -> Result<Vec<u8>, String> {
    let err = |e: rust_xlsxwriter::XlsxError| format!("產生 Excel 失敗：{e}");
    let mut book = Workbook::new();
    let bold = Format::new().set_bold();
    let sheet = book.add_worksheet();
    sheet.set_name(sheet_name(&r.title)).map_err(err)?;
    for (c, h) in r.columns.iter().enumerate() {
        sheet
            .write_string_with_format(0, col(c), h, &bold)
            .map_err(err)?;
    }
    for (i, row) in r.rows.iter().enumerate() {
        let line = u32::try_from(i + 1).map_err(|_| "Excel 筆數過多".to_owned())?;
        for (c, v) in row.iter().enumerate() {
            sheet.write_string(line, col(c), v).map_err(err)?;
        }
    }
    sheet.set_freeze_panes(1, 0).map_err(err)?;
    sheet.autofit();
    if let Some(c) = &r.condition {
        let s = book.add_worksheet();
        s.set_name(sheet_name(&r.labels.condition)).map_err(err)?;
        s.write_string(0, 0, c).map_err(err)?;
    }
    book.save_to_buffer().map_err(err)
}

fn col(i: usize) -> u16 {
    u16::try_from(i).unwrap_or(u16::MAX)
}

/// 工作表名稱：最多 31 字、不可含 []:*?/\
fn sheet_name(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .filter(|c| !matches!(c, '[' | ']' | ':' | '*' | '?' | '/' | '\\'))
        .take(31)
        .collect();
    if cleaned.is_empty() {
        "Sheet1".to_owned()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ttrs_app::report::labels;
    use ttrs_domain::lang::Lang;

    fn sample(lang: &str) -> ReportData {
        let lang = Lang::parse(lang).unwrap();
        ReportData {
            program: "department-maintenance".into(),
            title: "部門資料".into(),
            plant: "TW01".into(),
            plant_name: "台灣總廠".into(),
            printed_by: "tiptop".into(),
            printed_at: "2026/11/05 10:00".into(),
            columns: vec!["部門編號".into(), "簡稱".into()],
            rows: vec![
                vec!["MIS".into(), "資訊部 \"A\"".into()],
                vec!["SAL".into(), "业务部".into()],
            ],
            condition: Some("部門編號 = S*".into()),
            labels: labels(&lang),
            lang,
        }
    }

    #[test]
    fn pdf_renders_with_cjk_text() {
        let pdf = render_pdf(&sample("zh-TW")).unwrap();
        assert!(pdf.starts_with(b"%PDF"));
        assert!(pdf.len() > 1000);
    }

    #[test]
    fn source_escapes_and_paginates() {
        let src = typst_source(&sample("en"));
        assert!(src.contains(r#"資訊部 \"A\""#));
        assert!(src.contains("counter(page).display()"));
        assert!(src.contains("counter(page).final()"));
    }

    #[test]
    fn xlsx_renders() {
        let x = render_xlsx(&sample("zh-TW")).unwrap();
        assert!(x.starts_with(b"PK"), "xlsx 為 zip");
    }
}
