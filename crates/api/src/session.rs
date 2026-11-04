//! 登入連線（ADR 0009 決策 7）：以使用者代號＋Web 密碼登入後建立伺服器端連線，瀏覽器只持有 HttpOnly cookie。
//! 閒置 30 分鐘或登入超過 12 小時即失效（W11）；每 10 分鐘重新檢查帳號（sign_in.feature SQ3）。
//! 連線存在伺服器記憶體，API 重啟即需重新登入。

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::http::{HeaderMap, HeaderValue, header};
use ttrs_app::{Actor, ActorKind, AppError, SignInService};
use ttrs_domain::DomainError;
use ttrs_shared::SessionDto;

/// 連線 cookie 名稱
pub const COOKIE: &str = "ttrs_session";
/// 登入中的連線定期重新檢查帳號（SQ3）
const REVALIDATE_EVERY: Duration = Duration::from_secs(10 * 60);
/// 閒置與最長登入時間（W11）
const IDLE_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const MAX_LIFETIME: Duration = Duration::from_secs(12 * 60 * 60);

struct Entry {
    actor: Actor,
    created: Instant,
    last_seen: Instant,
    checked: Instant,
}

#[derive(Clone, Default)]
pub struct Sessions(Arc<Mutex<HashMap<String, Entry>>>);

impl Sessions {
    /// 登入：回傳新連線的代碼
    pub fn open(&self, session: &SessionDto) -> String {
        let token = new_token();
        let now = Instant::now();
        let mut map = self.lock();
        // 順便清掉已失效的連線
        map.retain(|_, e| !expired(e, now));
        map.insert(
            token.clone(),
            Entry {
                actor: actor_of(session),
                created: now,
                last_seen: now,
                checked: now,
            },
        );
        token
    }

    pub fn close(&self, token: &str) {
        self.lock().remove(token);
    }

    /// 連線的操作者；閒置或超過最長時間即失效，超過檢查間隔時重新檢查帳號
    pub async fn actor(&self, token: &str, sign_in: &SignInService) -> Result<Actor, AppError> {
        let (actor, due) = {
            let mut map = self.lock();
            let now = Instant::now();
            let e = map.get_mut(token).ok_or(DomainError::NotSignedIn)?;
            if expired(e, now) {
                map.remove(token);
                return Err(DomainError::NotSignedIn.into());
            }
            e.last_seen = now;
            (e.actor.clone(), e.checked.elapsed() >= REVALIDATE_EVERY)
        };
        if !due {
            return Ok(actor);
        }
        match sign_in.revalidate(&actor.user_id).await {
            Ok(s) => {
                let actor = actor_of(&s);
                if let Some(e) = self.lock().get_mut(token) {
                    e.actor = actor.clone();
                    e.checked = Instant::now();
                }
                Ok(actor)
            }
            Err(err) => {
                self.close(token);
                Err(err)
            }
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Entry>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn expired(e: &Entry, now: Instant) -> bool {
    now.duration_since(e.last_seen) >= IDLE_TIMEOUT || now.duration_since(e.created) >= MAX_LIFETIME
}

/// 連線資訊 → 操作者（部門、權限類別取自使用者資料）
fn actor_of(s: &SessionDto) -> Actor {
    Actor {
        user_id: s.user_id.clone(),
        dept: s.dept_code.clone().unwrap_or_default(),
        permission_class: s.permission_class.clone(),
        kind: ActorKind::Person,
    }
}

fn new_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("系統亂數");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// 請求帶的連線代碼
pub fn token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, _)| *k == COOKIE)
        .map(|(_, v)| v.to_owned())
}

/// 設定連線 cookie；`token` 為 None 時清除。`secure`：正式環境經 HTTPS 時加 Secure（TTRS_COOKIE_SECURE=1）
pub fn set_cookie(token: Option<&str>, secure: bool) -> HeaderValue {
    let flags = if secure {
        "HttpOnly; SameSite=Lax; Path=/; Secure"
    } else {
        "HttpOnly; SameSite=Lax; Path=/"
    };
    let v = match token {
        Some(t) => format!("{COOKIE}={t}; {flags}"),
        None => format!("{COOKIE}=; {flags}; Max-Age=0"),
    };
    HeaderValue::from_str(&v).expect("cookie")
}
