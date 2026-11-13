//! EPUB 轉 PDF 的排版（TTRS 新增的工具作業，TipTop 無對應）
//! 規格：specs/features/azz/epub_to_pdf.feature（EP5、EP6、EP8）
//!
//! 版面：A5 直式、四邊留白 15 mm、內文 11 pt、行距 1.5 倍；第一頁為書名與作者；每章從新的一頁開始，
//! 章名（章節第一行，40 字以內）用較大字體並建立 PDF 書籤；每頁底部置中頁碼；圖片略過。
//! 字型：主字型（含中文）＋選用的第二字型（補英數字與全形標點）；逐字挑第一個有該字的字型。

use std::path::PathBuf;

use printpdf::{
    FontId, Mm, Op, PageAnnotId, ParsedFont, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions,
    PdfWarnMsg, Point, Pt, TextItem,
};
use ttrs_domain::DomainError;

/// 一個字型檔（TTF／OTF 或 TTC 的第幾個字型）
#[derive(Debug, Clone)]
pub struct FontFile {
    pub bytes: Vec<u8>,
    pub index: u32,
}

/// 取得 PDF 要用的字型（EP6）：第一個是含中文的主字型，其餘為補字用
pub trait PdfFonts: Send + Sync {
    fn load(&self) -> Result<Vec<FontFile>, DomainError>;
}

/// 伺服器上的字型檔：環境變數 TTRS_PDF_FONT（主字型）、TTRS_PDF_FONT_LATIN（補字）優先，
/// 否則依序找常見路徑（Noto Sans CJK、Droid Sans Fallback、文泉驛、微軟正黑體）
#[derive(Debug, Clone, Default)]
pub struct SystemFonts;

/// 含中文的字型候選（路徑、TTC 內第幾個；Noto Sans CJK 的 TTC 順序為 JP、KR、SC、TC、HK）
const CJK_CANDIDATES: &[(&str, u32)] = &[
    ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 3),
    ("/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc", 3),
    ("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", 3),
    (
        "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
        0,
    ),
    ("/usr/share/fonts/truetype/wqy/wqy-microhei.ttc", 0),
    ("/mnt/c/Windows/Fonts/msjh.ttc", 0),
    ("C:\\Windows\\Fonts\\msjh.ttc", 0),
    ("/System/Library/Fonts/PingFang.ttc", 0),
];

/// 補英數字與標點的字型候選
const LATIN_CANDIDATES: &[(&str, u32)] = &[
    ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 0),
    (
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        0,
    ),
    ("/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf", 0),
    ("/mnt/c/Windows/Fonts/arial.ttf", 0),
    ("C:\\Windows\\Fonts\\arial.ttf", 0),
];

fn read(path: &str, index: u32) -> Option<FontFile> {
    let bytes = std::fs::read(PathBuf::from(path)).ok()?;
    Some(FontFile { bytes, index })
}

fn env_font(var: &str) -> Option<FontFile> {
    let path = std::env::var(var).ok()?;
    let path = path.trim();
    if path.is_empty() {
        return None;
    }
    // TTRS_PDF_FONT=/path/font.ttc#3 可指定 TTC 內第幾個
    let (p, i) = path
        .rsplit_once('#')
        .map_or((path, 0), |(p, i)| (p, i.parse().unwrap_or(0)));
    read(p, i)
}

impl PdfFonts for SystemFonts {
    fn load(&self) -> Result<Vec<FontFile>, DomainError> {
        let main = env_font("TTRS_PDF_FONT")
            .or_else(|| CJK_CANDIDATES.iter().find_map(|(p, i)| read(p, *i)))
            .ok_or(DomainError::PdfFontMissing)?;
        let mut fonts = vec![main];
        if let Some(f) = env_font("TTRS_PDF_FONT_LATIN")
            .or_else(|| LATIN_CANDIDATES.iter().find_map(|(p, i)| read(p, *i)))
        {
            fonts.push(f);
        }
        Ok(fonts)
    }
}

/// 沒有任何字型（BDD 驗證 PDF_FONT_MISSING）
#[derive(Debug, Clone, Default)]
pub struct NoFonts;

impl PdfFonts for NoFonts {
    fn load(&self) -> Result<Vec<FontFile>, DomainError> {
        Err(DomainError::PdfFontMissing)
    }
}

// ----- 版面常數（整數，單位 1/1000 pt；專案禁用浮點數運算，只在交給 printpdf 時轉成 f32） -----

/// 毫米轉 1/1000 pt（1 mm = 72/25.4 pt）
const fn mm(v: i64) -> i64 {
    v * 720_000 / 254
}

const PAGE_W: i64 = mm(148);
const PAGE_H: i64 = mm(210);
const MARGIN: i64 = mm(15);
const BODY: i64 = 11_000;
const BODY_LH: i64 = 16_500;
const HEADING: i64 = 16_000;
const HEADING_LH: i64 = 24_000;
const TITLE: i64 = 24_000;
const TITLE_LH: i64 = 36_000;
const AUTHOR: i64 = 14_000;
const AUTHOR_LH: i64 = 21_000;
const FOOTER: i64 = 9_000;
/// 章名最長字數（超過的第一行視為內文，書籤改用前 20 字）
const HEADING_MAX_CHARS: usize = 40;
/// 段落之間多空的高度
const PARA_GAP: i64 = 4_000;
/// 章名之後多空的高度
const HEADING_GAP: i64 = 10_000;
/// 書名頁的書名從頁面上緣往下的位置
const TITLE_TOP: i64 = mm(60);
/// 書名與作者之間的距離
const TITLE_GAP: i64 = 12_000;
/// 不可出現在行首的標點（避頭點：允許該行多放一個字）
const NO_LINE_START: &str = "，。、；：？！）」』】》〕…—,.;:?!)]}%";

/// 1/1000 pt 轉 printpdf 的 Pt（printpdf 的介面只收 f32）
#[allow(clippy::float_arithmetic, clippy::cast_precision_loss)]
fn to_pt(v: i64) -> Pt {
    Pt(v as f32 / 1000.0)
}

fn to_mm(v: i64) -> Mm {
    Mm::from(to_pt(v))
}

/// 一本書的內容：書名、作者與各章（每章為若干行文字）
pub struct Book<'a> {
    pub title: &'a str,
    pub author: &'a str,
    pub chapters: &'a [Vec<String>],
}

struct Loaded {
    id: FontId,
    font: ParsedFont,
}

impl Loaded {
    /// 該字在此字型的寬度（1/1000 pt）；字型沒有這個字回 None
    fn width(&self, ch: char, size: i64) -> Option<i64> {
        let gid = self.font.lookup_glyph_index(ch as u32)?;
        let w = i64::from(self.font.get_glyph_width(gid).unwrap_or(0));
        Some(w * size / i64::from(self.font.units_per_em.max(1)))
    }
}

/// 一個字：用哪個字型、寬度
#[derive(Clone, Copy)]
struct Glyph {
    ch: char,
    font: usize,
    w: i64,
}

type Run = (usize, String);

struct Writer {
    fonts: Vec<Loaded>,
    pages: Vec<Vec<Op>>,
    /// 目前頁面已用高度（從上邊界起算，1/1000 pt）
    used: i64,
}

impl Writer {
    fn glyphs(&self, text: &str, size: i64) -> Vec<Glyph> {
        text.chars()
            .filter(|c| !c.is_control())
            .filter_map(|ch| {
                if ch == ' ' || ch == '\u{3000}' {
                    // 空白：用第一個有空白的字型，沒有就用主字型的 1/3 em
                    let (font, w) = self
                        .fonts
                        .iter()
                        .enumerate()
                        .find_map(|(i, f)| f.width(ch, size).map(|w| (i, w)))
                        .unwrap_or((0, size / 3));
                    return Some(Glyph { ch, font, w });
                }
                self.fonts
                    .iter()
                    .enumerate()
                    .find_map(|(font, f)| f.width(ch, size).map(|w| Glyph { ch, font, w }))
            })
            .collect()
    }

    /// 斷行：CJK 逐字斷、英文盡量在空白處斷；行首不放標點
    fn wrap(&self, glyphs: &[Glyph], first_indent: i64, max_w: i64) -> Vec<Vec<Glyph>> {
        let mut lines: Vec<Vec<Glyph>> = Vec::new();
        let mut cur: Vec<Glyph> = Vec::new();
        let mut width = first_indent;
        for &g in glyphs {
            if width + g.w > max_w
                && !cur.is_empty()
                && g.ch != ' '
                && !NO_LINE_START.contains(g.ch)
            {
                // 英文單字：退到最後一個空白後斷行
                let split = cur
                    .iter()
                    .rposition(|x| x.ch == ' ')
                    .filter(|&i| i > 0 && g.ch.is_ascii_alphanumeric());
                let carry: Vec<Glyph> = match split {
                    Some(i) => {
                        let tail = cur.split_off(i + 1);
                        cur.pop();
                        tail
                    }
                    None => Vec::new(),
                };
                lines.push(std::mem::take(&mut cur));
                width = carry.iter().map(|x| x.w).sum::<i64>();
                cur = carry;
            }
            if cur.is_empty() && g.ch == ' ' {
                continue;
            }
            width += g.w;
            cur.push(g);
        }
        if !cur.is_empty() {
            lines.push(cur);
        }
        lines
    }

    fn runs(line: &[Glyph]) -> Vec<Run> {
        let mut runs: Vec<Run> = Vec::new();
        for g in line {
            match runs.last_mut() {
                Some((f, s)) if *f == g.font => s.push(g.ch),
                _ => runs.push((g.font, g.ch.to_string())),
            }
        }
        runs
    }

    fn new_page(&mut self) {
        self.pages.push(Vec::new());
        self.used = 0;
    }

    fn body_height() -> i64 {
        PAGE_H - MARGIN * 2
    }

    /// 在目前頁面放一行（x 為左邊界起算的位移）；放不下就先換頁
    fn put_line(&mut self, line: &[Glyph], size: i64, lh: i64, x: i64) {
        if self.pages.is_empty() || self.used + lh > Self::body_height() {
            self.new_page();
        }
        let baseline = PAGE_H - MARGIN - self.used - size;
        self.used += lh;
        let ops = self.pages.last_mut().expect("page");
        ops.push(Op::StartTextSection);
        ops.push(Op::SetTextCursor {
            pos: Point::new(to_mm(MARGIN + x), to_mm(baseline)),
        });
        for (f, text) in Self::runs(line) {
            ops.push(Op::SetFont {
                font: PdfFontHandle::External(self.fonts[f].id.clone()),
                size: to_pt(size),
            });
            ops.push(Op::ShowText {
                items: vec![TextItem::Text(text)],
            });
        }
        ops.push(Op::EndTextSection);
    }

    /// 一個段落（可能多行）；首行縮排 first_indent
    fn paragraph(&mut self, text: &str, size: i64, lh: i64, first_indent: i64) {
        let glyphs = self.glyphs(text, size);
        let max_w = PAGE_W - MARGIN * 2;
        for (i, line) in self
            .wrap(&glyphs, first_indent, max_w)
            .into_iter()
            .enumerate()
        {
            let x = if i == 0 { first_indent } else { 0 };
            self.put_line(&line, size, lh, x);
        }
    }

    /// 置中的一行（書名頁）
    fn centered(&mut self, text: &str, size: i64, lh: i64) {
        let glyphs = self.glyphs(text, size);
        let max_w = PAGE_W - MARGIN * 2;
        for line in self.wrap(&glyphs, 0, max_w) {
            let w: i64 = line.iter().map(|g| g.w).sum();
            self.put_line(&line, size, lh, (max_w - w) / 2);
        }
    }

    /// 每頁底部置中頁碼
    fn footers(&mut self) {
        let total = self.pages.len();
        for n in 0..total {
            let glyphs = self.glyphs(&(n + 1).to_string(), FOOTER);
            let w: i64 = glyphs.iter().map(|g| g.w).sum();
            let x = (PAGE_W - w) / 2;
            let mut ops = vec![
                Op::StartTextSection,
                Op::SetTextCursor {
                    pos: Point::new(to_mm(x), to_mm(MARGIN / 2)),
                },
            ];
            for (f, text) in Self::runs(&glyphs) {
                ops.push(Op::SetFont {
                    font: PdfFontHandle::External(self.fonts[f].id.clone()),
                    size: to_pt(FOOTER),
                });
                ops.push(Op::ShowText {
                    items: vec![TextItem::Text(text)],
                });
            }
            ops.push(Op::EndTextSection);
            self.pages[n].extend(ops);
        }
    }
}

/// 章名與內文：第一行在 40 字以內視為章名
fn split_heading(lines: &[String]) -> (Option<&str>, &[String]) {
    match lines.first() {
        Some(first) if first.chars().count() <= HEADING_MAX_CHARS => {
            (Some(first.as_str()), &lines[1..])
        }
        _ => (None, lines),
    }
}

/// 產生 PDF 位元組
pub fn render(book: &Book<'_>, files: &[FontFile]) -> Result<Vec<u8>, DomainError> {
    if files.is_empty() {
        return Err(DomainError::PdfFontMissing);
    }
    let mut doc = PdfDocument::new(book.title);
    let mut fonts = Vec::new();
    for f in files {
        let mut warnings = Vec::new();
        let font = ParsedFont::from_bytes(&f.bytes, f.index as usize, &mut warnings)
            .ok_or_else(|| DomainError::PdfGenerationFailed("字型檔無法讀取".to_owned()))?;
        let id = doc.add_font(&font);
        fonts.push(Loaded { id, font });
    }
    let mut w = Writer {
        fonts,
        pages: Vec::new(),
        used: 0,
    };

    // 書名頁
    w.new_page();
    w.used = TITLE_TOP;
    w.centered(book.title, TITLE, TITLE_LH);
    w.used += TITLE_GAP;
    if !book.author.is_empty() {
        w.centered(book.author, AUTHOR, AUTHOR_LH);
    }

    // 各章：每章從新的一頁開始
    let mut order: Vec<(usize, String)> = Vec::new();
    for lines in book.chapters {
        w.new_page();
        let page = w.pages.len() - 1;
        let (heading, body) = split_heading(lines);
        let name = match heading {
            Some(h) => h.to_owned(),
            None => {
                let first = lines.first().map_or("", String::as_str);
                let head: String = first.chars().take(20).collect();
                format!("{head}…")
            }
        };
        order.push((page, name));
        if let Some(h) = heading {
            w.paragraph(h, HEADING, HEADING_LH, 0);
            w.used += HEADING_GAP;
        }
        for line in body {
            w.paragraph(line, BODY, BODY_LH, BODY * 2);
            w.used += PARA_GAP;
        }
    }
    w.footers();

    let pages: Vec<PdfPage> = w
        .pages
        .into_iter()
        .map(|ops| PdfPage::new(Mm(148.0), Mm(210.0), ops))
        .collect();
    for (page, name) in &order {
        let _: PageAnnotId = doc.add_bookmark(name, *page);
    }
    let mut warnings: Vec<PdfWarnMsg> = Vec::new();
    Ok(doc
        .with_pages(pages)
        .save(&PdfSaveOptions::default(), &mut warnings))
}
