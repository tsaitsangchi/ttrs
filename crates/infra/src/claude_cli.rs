//! 翻譯引擎：呼叫伺服器上已登入的 Claude Code（`claude -p`），用訂閱額度、不另外付 API 費用
//! （epub_translate.feature EL2）。
//!
//! 呼叫方式（實測可用）：`claude -p <提示> --model haiku --effort low --tools "" --no-session-persistence
//! --system-prompt <翻譯員指示> --output-format json`；空工具與自訂系統提示讓每次呼叫的固定開銷約 800 token。
//! 執行檔路徑放 .env 的 TTRS_CLAUDE_BIN（預設 `claude`）；模型放 TTRS_CLAUDE_MODEL（預設 haiku）。

use std::{process::Stdio, time::Duration};

use async_trait::async_trait;
use serde_json::Value;
use tokio::process::Command;
use ttrs_app::ports::{TranslateRequest, TranslateResponse, TranslationEngine};
use ttrs_domain::DomainError;

/// 每次呼叫的逾時
const TIMEOUT: Duration = Duration::from_secs(300);

const SYSTEM_PROMPT: &str = "You are a professional literary translator from English to Traditional Chinese \
(Taiwan usage, 繁體中文). You receive a JSON object with `lines` (an array of paragraphs, in order) and \
optional `glossary` (an array of [english, chinese] name pairs you must follow). Translate every paragraph \
faithfully and naturally, keep the tone, keep numbers and untranslatable symbols as they are. \
Reply with ONLY one JSON object, no markdown fences, no commentary: \
{\"lines\": [<the translation of each input line, same count and order>], \
\"names\": [[english, chinese], ...]} where `names` lists the people and place names you translated \
in this batch (empty array if none).";

#[derive(Debug, Clone)]
pub struct ClaudeCliEngine {
    bin: String,
    model: String,
}

impl ClaudeCliEngine {
    pub fn from_env() -> Self {
        let get = |k: &str, d: &str| {
            std::env::var(k)
                .ok()
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| d.to_owned())
        };
        Self {
            bin: get("TTRS_CLAUDE_BIN", "claude"),
            model: get("TTRS_CLAUDE_MODEL", "haiku"),
        }
    }

    pub fn new(bin: &str, model: &str) -> Self {
        Self {
            bin: bin.to_owned(),
            model: model.to_owned(),
        }
    }

    fn command(&self, prompt: &str) -> Command {
        let mut c = Command::new(&self.bin);
        c.arg("-p")
            .arg(prompt)
            .args(["--model", &self.model])
            .args(["--effort", "low"])
            .args(["--tools", ""])
            .arg("--no-session-persistence")
            .args(["--system-prompt", SYSTEM_PROMPT])
            .args(["--output-format", "json"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        c
    }
}

fn not_configured(why: impl ToString) -> DomainError {
    DomainError::TranslateNotConfigured(why.to_string())
}

/// 從輸出文字抓出第一個完整的 JSON 物件（模型偶爾會多包一層說明或程式碼區塊）
fn extract_json(text: &str) -> Option<Value> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    serde_json::from_str(text.get(start..=end)?).ok()
}

fn pairs(v: &Value) -> Vec<(String, String)> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|p| {
                    let p = p.as_array()?;
                    Some((
                        p.first()?.as_str()?.to_owned(),
                        p.get(1)?.as_str()?.to_owned(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// 解析 `claude -p --output-format json` 的輸出：外層是結果信封，`result` 是模型回覆文字
fn parse_output(stdout: &str) -> Result<TranslateResponse, DomainError> {
    let env: Value = serde_json::from_str(stdout.trim())
        .map_err(|e| DomainError::TranslateFailed(format!("無法解析 Claude Code 輸出：{e}")))?;
    let text = env["result"].as_str().unwrap_or_default();
    if env["is_error"].as_bool().unwrap_or(false) {
        let lower = text.to_lowercase();
        if lower.contains("not logged in") || lower.contains("/login") {
            return Err(not_configured(
                "Claude Code 尚未登入（請在伺服器執行 claude 並登入）",
            ));
        }
        if lower.contains("limit") || lower.contains("usage") || lower.contains("quota") {
            return Err(DomainError::TranslateQuotaExhausted {
                reset: text.trim().to_owned(),
            });
        }
        return Err(DomainError::TranslateFailed(text.trim().to_owned()));
    }
    let body = extract_json(text)
        .ok_or_else(|| DomainError::TranslateFailed("模型沒有回傳 JSON".to_owned()))?;
    let lines = body["lines"]
        .as_array()
        .ok_or_else(|| DomainError::TranslateFailed("回傳缺少 lines".to_owned()))?
        .iter()
        .map(|l| l.as_str().unwrap_or_default().to_owned())
        .collect();
    let usage = &env["usage"];
    Ok(TranslateResponse {
        lines,
        names: pairs(&body["names"]),
        input_tokens: usage["input_tokens"].as_u64().unwrap_or(0)
            + usage["cache_creation_input_tokens"].as_u64().unwrap_or(0)
            + usage["cache_read_input_tokens"].as_u64().unwrap_or(0),
        output_tokens: usage["output_tokens"].as_u64().unwrap_or(0),
    })
}

#[async_trait]
impl TranslationEngine for ClaudeCliEngine {
    fn name(&self) -> String {
        format!("claude-cli:{}", self.model)
    }

    async fn check(&self) -> Result<(), DomainError> {
        // 只確認執行檔存在可執行；登入狀態在第一次翻譯時才會知道（未登入回 TRANSLATE_NOT_CONFIGURED）
        Command::new(&self.bin)
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map_err(|e| not_configured(format!("找不到 Claude Code（{}）：{e}", self.bin)))
            .and_then(|s| {
                s.success()
                    .then_some(())
                    .ok_or_else(|| not_configured("Claude Code 無法執行"))
            })
    }

    async fn translate(&self, req: &TranslateRequest) -> Result<TranslateResponse, DomainError> {
        let input = serde_json::json!({
            "lines": req.lines,
            "glossary": req.glossary.iter().map(|(e, z)| [e, z]).collect::<Vec<_>>(),
        });
        // 原文放在提示字串裡（--tools "" 沒有讀檔工具；一批約 2000 字元，遠低於參數長度上限）
        let child = self
            .command(&format!("Translate this JSON now:\n{input}"))
            .spawn()
            .map_err(|e| not_configured(format!("無法執行 Claude Code（{}）：{e}", self.bin)))?;
        let out = tokio::time::timeout(TIMEOUT, child.wait_with_output())
            .await
            .map_err(|_| DomainError::TranslateFailed("Claude Code 逾時".to_owned()))?
            .map_err(|e| DomainError::TranslateFailed(e.to_string()))?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        if stdout.trim().is_empty() {
            let err = String::from_utf8_lossy(&out.stderr);
            return Err(DomainError::TranslateFailed(format!(
                "Claude Code 沒有輸出（{}）",
                err.trim()
            )));
        }
        parse_output(&stdout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_success() {
        let out = r#"{"is_error":false,"result":"```json\n{\"lines\":[\"你好\"],\"names\":[[\"Tom\",\"湯姆\"]]}\n```","usage":{"input_tokens":2,"cache_creation_input_tokens":800,"output_tokens":9}}"#;
        let r = parse_output(out).unwrap();
        assert_eq!(r.lines, ["你好"]);
        assert_eq!(r.names, [("Tom".to_owned(), "湯姆".to_owned())]);
        assert_eq!((r.input_tokens, r.output_tokens), (802, 9));
    }

    #[test]
    fn maps_errors() {
        let e = parse_output(r#"{"is_error":true,"result":"Not logged in · Please run /login"}"#)
            .unwrap_err();
        assert_eq!(e.code(), "TRANSLATE_NOT_CONFIGURED");
        let e =
            parse_output(r#"{"is_error":true,"result":"5-hour usage limit reached"}"#).unwrap_err();
        assert_eq!(e.code(), "TRANSLATE_QUOTA_EXHAUSTED");
        let e = parse_output(r#"{"is_error":false,"result":"sorry"}"#).unwrap_err();
        assert_eq!(e.code(), "TRANSLATE_FAILED");
    }
}
