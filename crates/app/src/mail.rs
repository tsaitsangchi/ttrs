//! 系統寄信（TipTop: cl_jmail，p_passnoty 寄到 zx09；ADR 0010 決策 A10、R15）
//!
//! 信件文字放 `crates/app/locales/<語系>/mail.ftl`，依收件者的語系（zx06）寄送，找不到時退回 zh-TW。
//! 只支援本專案用到的 Fluent 子集：`id = 文字`、縮排的續行、`{ $變數 }`。

use std::sync::Arc;

use async_trait::async_trait;
use ttrs_domain::lang::Lang;

use crate::error::RepoError;

/// 信件種類
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailTemplate {
    /// 重設密碼連結
    PasswordReset,
    /// 密碼已透過重設連結變更（R8）
    PasswordChanged,
}

impl MailTemplate {
    fn id(self) -> &'static str {
        match self {
            Self::PasswordReset => "mail-password-reset",
            Self::PasswordChanged => "mail-password-changed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mail {
    pub to: String,
    pub lang: Lang,
    pub template: MailTemplate,
    pub subject: String,
    pub body: String,
}

impl Mail {
    /// 依語系套用信件範本；`args` 為（變數, 值）
    pub fn render(to: &str, lang: &Lang, template: MailTemplate, args: &[(&str, &str)]) -> Self {
        let text = |suffix: &str| {
            let id = format!("{}-{suffix}", template.id());
            let raw = message(lang.as_str(), &id)
                .or_else(|| message(ttrs_domain::lang::DEFAULT_LANG, &id))
                .unwrap_or_default();
            args.iter()
                .fold(raw, |acc, (k, v)| acc.replace(&format!("{{ ${k} }}"), v))
        };
        Self {
            to: to.to_owned(),
            lang: lang.clone(),
            template,
            subject: text("subject"),
            body: text("body"),
        }
    }
}

/// 寄信（infra 以 SMTP 實作；記憶體後端為 [`crate::memory::Outbox`]）
#[async_trait]
pub trait Mailer: Send + Sync {
    async fn send(&self, mail: &Mail) -> Result<(), RepoError>;
}

/// 自助重設密碼需要的設定（R4、R9）；沒有設定時不開放忘記密碼
#[derive(Clone)]
pub struct MailSettings {
    pub mailer: Arc<dyn Mailer>,
    /// 對外網址（TTRS_PUBLIC_URL），重設連結以此為開頭
    pub public_url: String,
}

const MAIL_FTL: [(&str, &str); 3] = [
    ("zh-TW", include_str!("../locales/zh-TW/mail.ftl")),
    ("en", include_str!("../locales/en/mail.ftl")),
    ("zh-CN", include_str!("../locales/zh-CN/mail.ftl")),
];

fn message(lang: &str, id: &str) -> Option<String> {
    let (_, src) = MAIL_FTL.iter().find(|(l, _)| *l == lang)?;
    parse(src)
        .into_iter()
        .find(|(k, _)| k == id)
        .map(|(_, v)| v)
}

/// `id = 值`；值可空白，接著的縮排行（或空行）為續行
fn parse(src: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for line in src.lines() {
        if line.starts_with('#') {
            continue;
        }
        if line.starts_with(' ') || line.trim().is_empty() {
            if let Some((_, lines)) = out.last_mut() {
                lines.push(line.trim().to_owned());
            }
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let first = v.trim();
            let lines = if first.is_empty() {
                Vec::new()
            } else {
                vec![first.to_owned()]
            };
            out.push((k.trim().to_owned(), lines));
        }
    }
    out.into_iter()
        .map(|(k, lines)| (k, lines.join("\n").trim().to_owned()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(src: &str) -> Vec<String> {
        parse(src).into_iter().map(|(k, _)| k).collect()
    }

    #[test]
    fn locales_have_same_ids() {
        let base = ids(MAIL_FTL[0].1);
        assert!(!base.is_empty());
        for (lang, src) in &MAIL_FTL[1..] {
            assert_eq!(ids(src), base, "{lang} 的信件 ID 與 zh-TW 不一致");
        }
    }

    #[test]
    fn render_substitutes_and_falls_back() {
        let m = Mail::render(
            "a@example.com",
            &Lang::parse("en").unwrap(),
            MailTemplate::PasswordReset,
            &[
                ("name", "Ann"),
                ("user", "u001"),
                ("link", "https://x/?token=abc"),
            ],
        );
        assert_eq!(m.subject, "Reset your TTRS password");
        assert!(m.body.starts_with("Hello Ann,"));
        assert!(m.body.contains("https://x/?token=abc"));
        assert!(m.body.contains("\n\n"), "保留空行");
        assert!(!m.body.contains("{ $"), "變數都已替換");
    }
}
