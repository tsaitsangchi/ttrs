//! EPUB 英文翻譯成繁體中文 — specs/features/azz/epub_translate.feature（EL；假翻譯引擎，不連外網）

use cucumber::{gherkin::Step, given, then, when};
use ttrs_app::{Content, Format};

use crate::{
    ErpWorld,
    steps::{azz_epub_to_pdf::parse_pdf_bytes, azz_epub_to_text::build, common::rows},
};

fn status_text(s: &str) -> &'static str {
    match s {
        "queued" => "排隊中",
        "running" => "翻譯中",
        "done" => "完成",
        "failed" => "失敗",
        other => panic!("未知狀態：{other}"),
    }
}

fn book_bytes(w: &ErpWorld) -> Vec<u8> {
    let (t, a, ch, sp) = w.epub_book.clone().expect("尚未建立 EPUB");
    build(&t, &a, &ch, &sp, false)
}

fn job(w: &ErpWorld) -> String {
    w.translate_job.clone().expect("尚未提交翻譯工作")
}

#[given(expr = "翻譯服務為假引擎（把英文加上「【譯】」前綴）")]
async fn fake_engine(_w: &mut ErpWorld) {}

#[given(regex = r#"^有一本英文 EPUB，書名 "([^"]+)"，作者 "([^"]+)"，閱讀順序為:$"#)]
async fn english_book(w: &mut ErpWorld, title: String, author: String, step: &Step) {
    let chapters: Vec<(String, String)> = rows(step)
        .into_iter()
        .map(|r| (r[0].clone(), r[1].clone()))
        .collect();
    let spine = chapters.iter().map(|(f, _)| f.clone()).collect();
    w.epub_book = Some((title, author, chapters, spine));
}

#[given(regex = r#"^有一本英文 EPUB，書名 "([^"]+)"，作者 "([^"]+)"，其中一章有 (\d+) 個段落$"#)]
async fn english_long(w: &mut ErpWorld, title: String, author: String, n: usize) {
    let body: String = (1..=n)
        .map(|i| format!("<p>Paragraph number {i} of the long chapter.</p>"))
        .collect();
    let ch = vec![("c1.xhtml".to_owned(), format!("<h1>Chapter One</h1>{body}"))];
    w.epub_book = Some((title, author, ch, vec!["c1.xhtml".to_owned()]));
}

#[given(regex = r"^有一本英文 EPUB，內容共 (\d+) 個字元$")]
async fn english_chars(w: &mut ErpWorld, n: usize) {
    // 分成多段，每段 1000 字元，最後一段補到剛好 n 字元
    let mut left = n;
    let mut body = String::new();
    while left > 0 {
        let k = left.min(1000);
        body.push_str(&format!("<p>{}</p>", "a".repeat(k)));
        left -= k;
    }
    let ch = vec![("c1.xhtml".to_owned(), body)];
    w.epub_book = Some((
        "Big".to_owned(),
        "Tom".to_owned(),
        ch,
        vec!["c1.xhtml".to_owned()],
    ));
}

#[given(regex = r#"^翻譯引擎每批回報人名 "(\S+)" 譯為 "(\S+)"$"#)]
async fn engine_name(w: &mut ErpWorld, en: String, zh: String) {
    w.translator.set_name(&en, &zh);
}

#[given(regex = r"^翻譯引擎前 (\d+) 次呼叫會失敗$")]
async fn engine_fail_first(w: &mut ErpWorld, n: usize) {
    w.translator.set_fail_first(n);
}

#[given("翻譯引擎一直失敗")]
async fn engine_fail(w: &mut ErpWorld) {
    w.translator.set_always_fail();
}

#[given("翻譯服務尚未設定")]
async fn engine_unset(w: &mut ErpWorld) {
    w.translator.set_not_configured();
}

#[given(regex = r"^翻譯引擎在第 (\d+) 次呼叫回報用量已達上限$")]
async fn engine_quota(w: &mut ErpWorld, n: usize) {
    w.translator.set_quota_at(Some(n));
}

async fn submit(w: &mut ErpWorld, format: &str, consent: bool) {
    let bytes = book_bytes(w);
    let r = w
        .translate
        .submit(
            &w.actor,
            "book.epub",
            &bytes,
            Format::parse(format).expect("格式"),
            consent,
        )
        .await;
    if let Ok(id) = &r {
        w.translate_job = Some(id.clone());
    }
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者勾選同意並將這本 EPUB 翻譯成繁體中文，輸出 "(txt|pdf)"$"#)]
async fn submit_ok(w: &mut ErpWorld, format: String) {
    submit(w, &format, true).await;
}

#[when(regex = r#"^使用者沒有勾選同意就將這本 EPUB 翻譯成繁體中文，輸出 "(txt|pdf)"$"#)]
async fn submit_no_consent(w: &mut ErpWorld, format: String) {
    submit(w, &format, false).await;
}

#[given("使用者已提交一個尚未完成的翻譯工作")]
async fn pending_job(w: &mut ErpWorld) {
    submit(w, "txt", true).await;
    assert!(matches!(w.last, Some(Ok(()))), "第一次提交應成功");
}

#[when("翻譯工作完成")]
#[given("翻譯工作完成")]
async fn run_job(w: &mut ErpWorld) {
    let id = job(w);
    w.translate.process(&id).await.expect("執行翻譯工作");
}

#[when("翻譯引擎恢復後使用者重新提交同一本書")]
async fn resubmit(w: &mut ErpWorld) {
    w.translate_calls_before = w.translator.calls().len();
    w.translator.recover();
    let first = job(w);
    submit(w, "txt", true).await;
    assert_eq!(job(w), first, "應沿用同一個工作續翻");
}

#[when(regex = r"^經過 25 小時並執行清除$")]
#[given(regex = r"^經過 25 小時並執行清除$")]
async fn purge(w: &mut ErpWorld) {
    let later = chrono::Utc::now() + chrono::Duration::hours(25);
    w.translate.purge_expired(later).await.expect("清除");
}

#[then(regex = r#"^應取得翻譯工作編號，狀態為 "(\S+)"$"#)]
async fn job_created(w: &mut ErpWorld, status: String) {
    if let Some(Err(e)) = &w.last {
        panic!("提交失敗：{e}");
    }
    let s = w.translate.status(&w.actor, &job(w)).await.expect("狀態");
    assert_eq!(status_text(&s.status), status);
}

#[then(regex = r#"^翻譯工作狀態應為 "(\S+)"$"#)]
async fn job_status(w: &mut ErpWorld, status: String) {
    let s = w.translate.status(&w.actor, &job(w)).await.expect("狀態");
    assert_eq!(
        status_text(&s.status),
        status,
        "工作：{:?} {:?}",
        s.error_code,
        s.error_message
    );
}

#[then(regex = r#"^翻譯工作狀態應為 "(\S+)"，原因為 "(\S+)"$"#)]
async fn job_status_reason(w: &mut ErpWorld, status: String, code: String) {
    let s = w.translate.status(&w.actor, &job(w)).await.expect("狀態");
    assert_eq!(status_text(&s.status), status);
    assert_eq!(s.error_code.as_deref(), Some(code.as_str()));
}

#[then(expr = "翻譯工作的進度應為 {string}")]
async fn job_progress(w: &mut ErpWorld, text: String) {
    let s = w.translate.status(&w.actor, &job(w)).await.expect("狀態");
    assert_eq!(
        format!("已翻譯 {}／{} 段", s.done_lines, s.total_lines),
        text
    );
}

#[then(regex = r#"^翻譯結果的書名應為 "([^"]+)"，作者 "([^"]+)"$"#)]
async fn result_title(w: &mut ErpWorld, title: String, author: String) {
    let o = w.translate.download(&w.actor, &job(w)).await.expect("下載");
    assert_eq!(
        (o.title.as_deref(), o.author.as_deref()),
        (Some(title.as_str()), Some(author.as_str()))
    );
}

#[then("翻譯結果的文字應為:")]
async fn result_text(w: &mut ErpWorld, step: &Step) {
    let o = w.translate.download(&w.actor, &job(w)).await.expect("下載");
    let Content::Text(t) = o.content else {
        panic!("結果不是文字");
    };
    let expected = step
        .docstring
        .as_ref()
        .expect("需要文字")
        .trim_matches('\n')
        .replace('\n', "\r\n");
    assert_eq!(t, expected);
}

#[then(regex = r#"^翻譯結果應為 PDF，內容包含 "(.+)"$"#)]
async fn result_pdf(w: &mut ErpWorld, text: String) {
    let o = w.translate.download(&w.actor, &job(w)).await.expect("下載");
    let Content::Pdf(bytes) = o.content else {
        panic!("結果不是 PDF");
    };
    let all = parse_pdf_bytes(&bytes).pages.join("");
    assert!(all.contains(&text), "PDF 應包含「{text}」，實際：{all}");
}

#[then("不應有可下載的結果")]
async fn no_result(w: &mut ErpWorld) {
    assert!(w.translate.download(&w.actor, &job(w)).await.is_err());
}

#[then(regex = r"^翻譯引擎應被呼叫多次，每次不超過 (\d+) 字元$")]
async fn engine_calls(w: &mut ErpWorld, max: usize) {
    let calls = w.translator.calls();
    assert!(calls.len() > 1, "呼叫 {} 次", calls.len());
    assert!(
        calls.iter().all(|c| c.chars <= max),
        "有一次超過 {max} 字元：{:?}",
        calls.iter().map(|c| c.chars).max()
    );
}

#[then("翻譯引擎不應被呼叫")]
async fn engine_not_called(w: &mut ErpWorld) {
    assert!(w.translator.calls().is_empty());
}

#[then(regex = r#"^第 (\d+) 次呼叫翻譯引擎時應帶有對照表 "(\S+)" 譯為 "(\S+)"$"#)]
async fn glossary(w: &mut ErpWorld, n: usize, en: String, zh: String) {
    let calls = w.translator.calls();
    let call = calls.get(n - 1).expect("沒有這次呼叫");
    assert!(
        call.glossary.contains(&(en, zh)),
        "對照表：{:?}",
        call.glossary
    );
}

#[then(regex = r"^恢復後翻譯引擎的呼叫次數應比批次總數少 (\d+) 次$")]
async fn resume_calls(w: &mut ErpWorld, fewer: usize) {
    let s = w.translate.status(&w.actor, &job(w)).await.expect("狀態");
    let after = w.translator.calls().len() - w.translate_calls_before;
    assert_eq!(after, s.total_batches as usize - fewer);
}

#[then(regex = r#"^查詢該翻譯工作應出現錯誤 "(\S+)"$"#)]
async fn status_error(w: &mut ErpWorld, code: String) {
    let e = w
        .translate
        .status(&w.actor, &job(w))
        .await
        .expect_err("應查不到");
    assert_eq!(e.code(), code);
}

#[then(regex = r#"^其他使用者查詢該翻譯工作應出現錯誤 "(\S+)"$"#)]
async fn other_user_error(w: &mut ErpWorld, code: String) {
    let mut other = w.actor.clone();
    other.user_id = "someone-else".to_owned();
    let e = w
        .translate
        .status(&other, &job(w))
        .await
        .expect_err("應查不到");
    assert_eq!(e.code(), code);
}

#[then(expr = "稽核紀錄的內容不應包含 {string}")]
async fn audit_no_content(w: &mut ErpWorld, text: String) {
    let entries = w
        .programs
        .audit_entries(&w.actor, "book.epub")
        .await
        .unwrap();
    assert!(
        entries
            .iter()
            .all(|e| !e.detail.as_deref().unwrap_or_default().contains(&text)),
        "稽核內容不應含「{text}」"
    );
}
