//! ttrs-admin 伺服器指令（ADR 0010 §B；specs/features/azz/emergency_password_reset.feature）
//!
//! 只在伺服器上執行（`src/bin/ttrs-admin.rs`），沒有 HTTP API（R11）。這裡只做參數解析與輸出文字，
//! 規則在 `ttrs_app::EmergencyService`。臨時密碼只出現在回傳的輸出，不寫入程式紀錄（R12）。

use ttrs_app::{AppError, EmergencyService};

/// 支援的指令
pub const COMMANDS: [&str; 2] = ["reset-password", "unlock"];

pub const USAGE: &str = "\
用法：
  ttrs-admin reset-password <使用者代號> [--reactivate]
      重設為隨機臨時密碼（只顯示這一次），下次登入須改密碼，並解除鎖定、結束已登入的連線。
      --reactivate：帳號已停用或過期時同時恢復有效。
  ttrs-admin unlock <使用者代號>
      只解除鎖定（試誤次數歸零），不改密碼。
需要環境變數 DATABASE_URL。";

/// 指令輸出
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminOutput {
    /// 顯示在終端機的文字
    pub message: String,
    /// reset-password 產生的臨時密碼（已包含在 message 中）
    pub temp_password: Option<String>,
}

/// 參數錯誤（顯示用法）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageError(pub String);

/// 指令失敗：參數錯誤或業務錯誤
#[derive(Debug)]
pub enum AdminError {
    Usage(String),
    App(AppError),
}

impl AdminError {
    /// 錯誤代碼（同 DomainError::code；參數錯誤為 USAGE）
    pub fn code(&self) -> &'static str {
        match self {
            Self::Usage(_) => "USAGE",
            Self::App(e) => e.code(),
        }
    }
}

impl std::fmt::Display for AdminError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage(msg) => write!(f, "{msg}\n\n{USAGE}"),
            Self::App(e) => write!(f, "{}（{}）", e, e.code()),
        }
    }
}

impl From<AppError> for AdminError {
    fn from(e: AppError) -> Self {
        Self::App(e)
    }
}

/// 執行指令；`args` 不含程式名稱，`operator` 為作業系統使用者（寫入稽核紀錄）
pub async fn run(
    emergency: &EmergencyService,
    args: &[&str],
    operator: &str,
) -> Result<AdminOutput, AdminError> {
    match parse(args) {
        Ok(Command::ResetPassword { user, reactivate }) => {
            let temp = emergency.reset_password(user, operator, reactivate).await?;
            Ok(AdminOutput {
                message: format!(
                    "已重設使用者 {user} 的密碼。\n臨時密碼（只顯示這一次）：{temp}\n請以安全的方式交給本人；登入後須立即變更密碼。"
                ),
                temp_password: Some(temp),
            })
        }
        Ok(Command::Unlock { user }) => {
            emergency.unlock(user, operator).await?;
            Ok(AdminOutput {
                message: format!("已解除使用者 {user} 的鎖定。"),
                temp_password: None,
            })
        }
        Err(UsageError(msg)) => Err(AdminError::Usage(msg)),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Command<'a> {
    ResetPassword { user: &'a str, reactivate: bool },
    Unlock { user: &'a str },
}

fn parse<'a>(args: &[&'a str]) -> Result<Command<'a>, UsageError> {
    let (flags, words): (Vec<&str>, Vec<&str>) = args.iter().partition(|a| a.starts_with("--"));
    let reactivate = flags.contains(&"--reactivate");
    if let Some(f) = flags.iter().find(|f| **f != "--reactivate") {
        return Err(UsageError(format!("不認得的選項：{f}")));
    }
    match words.as_slice() {
        ["reset-password", user] => Ok(Command::ResetPassword { user, reactivate }),
        ["unlock", user] if !reactivate => Ok(Command::Unlock { user }),
        ["unlock", _] => Err(UsageError("unlock 不接受 --reactivate".into())),
        [] => Err(UsageError("請指定指令".into())),
        [cmd, ..] if !COMMANDS.contains(cmd) => Err(UsageError(format!("不認得的指令：{cmd}"))),
        _ => Err(UsageError("請指定一個使用者代號".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_commands() {
        assert_eq!(
            parse(&["reset-password", "admin"]),
            Ok(Command::ResetPassword {
                user: "admin",
                reactivate: false
            })
        );
        assert_eq!(
            parse(&["reset-password", "admin", "--reactivate"]),
            Ok(Command::ResetPassword {
                user: "admin",
                reactivate: true
            })
        );
        assert_eq!(
            parse(&["unlock", "admin"]),
            Ok(Command::Unlock { user: "admin" })
        );
        assert!(parse(&[]).is_err());
        assert!(parse(&["drop", "admin"]).is_err());
        assert!(parse(&["reset-password"]).is_err());
        assert!(parse(&["reset-password", "a", "b"]).is_err());
        assert!(parse(&["reset-password", "a", "--force"]).is_err());
    }
}
