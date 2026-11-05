//! SMTP 寄信（TipTop: cl_jmail；ADR 0010 決策 A10、R9）
//!
//! 設定由環境變數取得，沒有 `TTRS_SMTP_HOST` 時不寄信（忘記密碼不開放）：
//! - `TTRS_SMTP_HOST`：主機（必要）
//! - `TTRS_SMTP_FROM`：寄件人，例如 `TTRS <ttrs@example.com>`（必要）
//! - `TTRS_SMTP_PORT`：連接埠（預設依加密方式：starttls 587、tls 465、none 25）
//! - `TTRS_SMTP_TLS`：`starttls`（預設）、`tls`、`none`（只限內部轉信主機）
//! - `TTRS_SMTP_USER`、`TTRS_SMTP_PASSWORD`：帳號密碼（未設定時不驗證）

use async_trait::async_trait;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Mailbox, header::ContentType},
    transport::smtp::authentication::Credentials,
};
use ttrs_app::{
    error::RepoError,
    mail::{Mail, Mailer},
};

pub struct SmtpMailer {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

impl SmtpMailer {
    /// 讀環境變數；沒有設定主機時回 None，設定不完整時回錯誤說明
    pub fn from_env() -> Result<Option<Self>, String> {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        let Some(host) = var("TTRS_SMTP_HOST") else {
            return Ok(None);
        };
        let from: Mailbox = var("TTRS_SMTP_FROM")
            .ok_or("TTRS_SMTP_FROM 未設定")?
            .parse()
            .map_err(|e| format!("TTRS_SMTP_FROM 格式不正確：{e}"))?;
        let tls = var("TTRS_SMTP_TLS").unwrap_or_else(|| "starttls".into());
        let mut builder = match tls.as_str() {
            "starttls" => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host),
            "tls" => AsyncSmtpTransport::<Tokio1Executor>::relay(&host),
            "none" => Ok(AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&host).port(25)),
            other => return Err(format!("TTRS_SMTP_TLS 只能是 starttls、tls、none：{other}")),
        }
        .map_err(|e| format!("SMTP 設定錯誤：{e}"))?;
        if let Some(port) = var("TTRS_SMTP_PORT") {
            let port = port
                .parse()
                .map_err(|_| format!("TTRS_SMTP_PORT 不是數字：{port}"))?;
            builder = builder.port(port);
        }
        if let Some(user) = var("TTRS_SMTP_USER") {
            let password = var("TTRS_SMTP_PASSWORD").unwrap_or_default();
            builder = builder.credentials(Credentials::new(user, password));
        }
        Ok(Some(Self {
            transport: builder.build(),
            from,
        }))
    }
}

#[async_trait]
impl Mailer for SmtpMailer {
    async fn send(&self, mail: &Mail) -> Result<(), RepoError> {
        let to: Mailbox = mail
            .to
            .parse()
            .map_err(|e| RepoError::Other(format!("收件人格式不正確：{e}")))?;
        let message = Message::builder()
            .from(self.from.clone())
            .to(to)
            .subject(&mail.subject)
            .header(ContentType::TEXT_PLAIN)
            .body(mail.body.clone())
            .map_err(|e| RepoError::Other(format!("信件內容：{e}")))?;
        self.transport
            .send(message)
            .await
            .map_err(|e| RepoError::Other(format!("寄信失敗：{e}")))?;
        Ok(())
    }
}
