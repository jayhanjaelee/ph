//! 출력 형식: JSON DTO (외부 계약, 스키마 버전 1)와 text 렌더러.

use serde::Serialize;

use crate::core::model::{Prompt, Scope};
use crate::core::service::{Entry, ScopedSkip};

/// JSON 스키마 버전. DTO 를 바꾸면 올린다.
pub const SCHEMA_VERSION: u32 = 1;

/// 배지 (`[L]` / `[G]`).
pub fn badge(scope: Scope) -> &'static str {
    match scope {
        Scope::Local => "[L]",
        Scope::Global => "[G]",
    }
}

/// prompt 요약 (본문 제외).
#[derive(Serialize)]
pub struct PromptSummaryJson {
    /// id
    pub id: String,
    /// `"local"` / `"global"`
    pub scope: &'static str,
    /// title
    pub title: String,
    /// 설명
    pub description: Option<String>,
    /// 태그
    pub tags: Vec<String>,
    /// RFC3339
    pub created_at: String,
    /// RFC3339
    pub updated_at: String,
    /// list/search 에서만 의미가 있다
    pub shadowed: bool,
}

/// 요약 + 본문.
#[derive(Serialize)]
pub struct PromptJson {
    #[serde(flatten)]
    /// 요약
    pub summary: PromptSummaryJson,
    /// 본문
    pub body: String,
}

/// 경고.
#[derive(Serialize)]
pub struct WarningJson {
    /// 항상 `"skipped"`
    pub kind: &'static str,
    /// scope
    pub scope: &'static str,
    /// 파일 이름
    pub name: String,
    /// 사유
    pub reason: String,
}

impl PromptSummaryJson {
    /// `Prompt` 에서 만든다.
    pub fn new(p: &Prompt, shadowed: bool) -> Self {
        Self {
            id: p.id.as_str().to_string(),
            scope: p.scope.as_str(),
            title: p.title.clone(),
            description: p.description.clone(),
            tags: p.tags.clone(),
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
            shadowed,
        }
    }
}

impl PromptJson {
    /// `Prompt` 에서 만든다.
    pub fn new(p: &Prompt) -> Self {
        Self {
            summary: PromptSummaryJson::new(p, false),
            body: p.body.clone(),
        }
    }
}

impl WarningJson {
    /// 건너뛴 항목에서 만든다.
    pub fn from_skip(s: &ScopedSkip) -> Self {
        Self {
            kind: "skipped",
            scope: s.scope.as_str(),
            name: s.entry.name.clone(),
            reason: s.entry.reason.clone(),
        }
    }
}

/// `list`/`search` JSON.
#[derive(Serialize)]
pub struct ListJson {
    /// 스키마 버전
    pub schema_version: u32,
    /// 항목 수
    pub count: usize,
    /// 항목
    pub prompts: Vec<PromptSummaryJson>,
    /// 경고
    pub warnings: Vec<WarningJson>,
}

/// `get` JSON.
#[derive(Serialize)]
pub struct GetJson {
    /// 스키마 버전
    pub schema_version: u32,
    /// prompt
    pub prompt: PromptJson,
    /// 양쪽 scope 에 있었는가
    pub ambiguous: bool,
}

/// `add` JSON.
#[derive(Serialize)]
pub struct AddJson {
    /// 스키마 버전
    pub schema_version: u32,
    /// prompt
    pub prompt: PromptJson,
    /// scope 가 자동 선택되었는가
    pub auto_selected: bool,
}

/// `rm` JSON.
#[derive(Serialize)]
pub struct RmJson {
    /// 스키마 버전
    pub schema_version: u32,
    /// 삭제된 항목
    pub removed: RemovedJson,
}

/// 삭제된 항목.
#[derive(Serialize)]
pub struct RemovedJson {
    /// id
    pub id: String,
    /// scope
    pub scope: &'static str,
}

/// `move` JSON.
#[derive(Serialize)]
pub struct MoveJson {
    /// 스키마 버전
    pub schema_version: u32,
    /// prompt
    pub prompt: PromptJson,
    /// 원래 scope
    pub from: &'static str,
    /// 옮긴 scope
    pub to: &'static str,
}

/// `init` JSON.
#[derive(Serialize)]
pub struct InitJson {
    /// 스키마 버전
    pub schema_version: u32,
    /// `.ph` 경로
    pub path: String,
    /// 새로 만들었는가
    pub created: bool,
}

/// 한 줄 JSON + 개행.
pub fn to_json_line<T: Serialize>(v: &T) -> Result<String, crate::core::error::PhError> {
    serde_json::to_string(v)
        .map(|mut s| {
            s.push('\n');
            s
        })
        .map_err(|e| crate::core::error::PhError::io("JSON 직렬화", e.into()))
}

/// list/search 의 text 한 줄.
pub fn entry_line(e: &Entry) -> String {
    let p = &e.prompt;
    let mut s = format!("{} {}  {}", badge(p.scope), p.id, p.title);
    if !p.tags.is_empty() {
        let tags: Vec<String> = p.tags.iter().map(|t| format!("#{t}")).collect();
        s.push_str("  ");
        s.push_str(&tags.join(" "));
    }
    if e.shadowed {
        s.push_str("  (shadowed)");
    }
    s
}

/// list/search 결과를 stdout 문자열(text 또는 JSON)로 만든다.
pub fn render_entries(
    entries: &[Entry],
    skipped: &[ScopedSkip],
    json: bool,
) -> Result<String, crate::core::error::PhError> {
    if json {
        return to_json_line(&ListJson {
            schema_version: SCHEMA_VERSION,
            count: entries.len(),
            prompts: entries
                .iter()
                .map(|e| PromptSummaryJson::new(&e.prompt, e.shadowed))
                .collect(),
            warnings: skipped.iter().map(WarningJson::from_skip).collect(),
        });
    }
    let mut out = String::new();
    for e in entries {
        out.push_str(&entry_line(e));
        out.push('\n');
    }
    Ok(out)
}

/// skipped 경고 문구.
pub fn skip_warning(s: &ScopedSkip) -> String {
    format!(
        "경고: 읽지 못한 파일 {} {}: {}",
        badge(s.scope),
        s.entry.name,
        s.entry.reason
    )
}

/// ambiguous 경고 문구.
pub fn ambiguous_warning(id: &str) -> String {
    format!(
        "경고: id '{id}' 가 local 과 global 양쪽에 있습니다. local 을 사용합니다. global 을 쓰려면 --global 을 지정하세요"
    )
}

/// 깨진 파일 대체 경고 문구 (`get` 만).
pub fn fallback_warning(id: &str, f: &crate::core::service::BrokenFallback) -> String {
    format!(
        "경고: {} 의 '{id}' 가 깨져 있어 {} 을 사용합니다 ({})",
        f.broken_scope.as_str(),
        f.broken_scope.other().as_str(),
        f.reason
    )
}
