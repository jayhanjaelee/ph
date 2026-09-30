//! 파일 텍스트 <-> `Prompt` 변환 (TOML frontmatter). 순수 함수만 둔다.
//!
//! 읽을 때는 UTF-8 BOM 과 CRLF 를 허용하고, 쓸 때는 LF 로 통일한다.

use chrono::{DateTime, FixedOffset};
use serde::Deserialize;

use super::error::PhError;
use super::model::{Prompt, PromptId, Scope};

const DELIM: &str = "+++";

#[derive(Deserialize)]
struct Frontmatter {
    /// id 는 파일 위치가 정한다. 키가 있으면 무시하되 있었다는 사실만 알린다.
    #[serde(default)]
    id: Option<toml::Value>,
    title: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    description: Option<String>,
    created_at: DateTime<FixedOffset>,
    updated_at: DateTime<FixedOffset>,
}

fn bad(id: &PromptId, line: Option<usize>, reason: impl Into<String>) -> PhError {
    PhError::InvalidFormat {
        id: Some(id.as_str().to_string()),
        line,
        reason: reason.into(),
    }
}

/// 파일 전체 텍스트를 `Prompt` 로 파싱한다. id 와 scope 는 파일 밖(위치)에서 정해진다.
pub fn parse(raw: &str, id: &PromptId, scope: Scope) -> Result<Prompt, PhError> {
    parse_detailed(raw, id, scope).map(|(p, _)| p)
}

/// `parse` 와 같고, frontmatter 에 (무시된) `id` 키가 있었는지도 돌려준다.
pub fn parse_detailed(raw: &str, id: &PromptId, scope: Scope) -> Result<(Prompt, bool), PhError> {
    let text = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let text = text.replace("\r\n", "\n");

    let mut lines = text.split_inclusive('\n');
    let first = lines.next().unwrap_or("");
    if first.trim_end() != DELIM {
        return Err(bad(
            id,
            Some(1),
            "첫 줄이 '+++' 여야 합니다. 파일 맨 위에 TOML frontmatter 를 '+++' 로 감싸 쓰세요",
        ));
    }

    let mut front = String::new();
    let mut consumed = first.len();
    let mut closed = false;
    for l in lines {
        consumed += l.len();
        if l.trim_end() == DELIM {
            closed = true;
            break;
        }
        front.push_str(l);
    }
    if !closed {
        return Err(bad(
            id,
            None,
            "frontmatter 를 닫는 '+++' 줄이 없습니다. 본문 앞에 '+++' 를 추가하세요",
        ));
    }
    let body = text[consumed..].to_string();

    let fm: Frontmatter = toml::from_str(&front).map_err(|e| {
        // frontmatter 는 파일의 2번째 줄부터 시작한다.
        let line = e.span().map(|s| {
            front[..s.start.min(front.len())]
                .bytes()
                .filter(|b| *b == b'\n')
                .count()
                + 2
        });
        bad(id, line, e.message().to_string())
    })?;

    let id_key = fm.id.is_some();
    let prompt = Prompt {
        id: id.clone(),
        scope,
        title: fm.title,
        body,
        tags: fm.tags,
        description: fm.description,
        created_at: fm.created_at,
        updated_at: fm.updated_at,
    };
    Ok((prompt, id_key))
}

/// TOML basic string 으로 쓴다. 개행 등 제어 문자는 모두 이스케이프하므로 값 안의 어떤 줄도
/// frontmatter 구분자(`+++`)와 겹치지 않는다.
fn toml_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `Prompt` 를 파일 텍스트로 직렬화한다 (LF).
pub fn serialize(prompt: &Prompt) -> Result<String, PhError> {
    let tags: Vec<String> = prompt.tags.iter().map(|t| toml_string(t)).collect();
    let mut out = format!("{DELIM}\ntitle = {}\n", toml_string(&prompt.title));
    out.push_str(&format!("tags = [{}]\n", tags.join(", ")));
    if let Some(d) = &prompt.description {
        out.push_str(&format!("description = {}\n", toml_string(d)));
    }
    out.push_str(&format!(
        "created_at = {}\nupdated_at = {}\n",
        toml_string(&prompt.created_at.to_rfc3339()),
        toml_string(&prompt.updated_at.to_rfc3339())
    ));
    out.push_str(DELIM);
    out.push('\n');
    out.push_str(&prompt.body.replace("\r\n", "\n"));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Prompt {
        let t = DateTime::parse_from_rfc3339("2026-09-30T12:00:00+09:00").unwrap();
        Prompt {
            id: PromptId::parse("코드-리뷰").unwrap(),
            scope: Scope::Global,
            title: "코드 리뷰".into(),
            body: "다음 diff: {{focus}}\n둘째 줄".into(),
            tags: vec!["review".into()],
            description: Some("PR".into()),
            created_at: t,
            updated_at: t,
        }
    }

    #[test]
    fn roundtrip() {
        let p = sample();
        let text = serialize(&p).unwrap();
        assert!(text.starts_with("+++\n"));
        assert!(!text.contains('\r'));
        let back = parse(&text, &p.id, Scope::Global).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn accepts_bom_and_crlf() {
        let p = sample();
        let text = format!("\u{feff}{}", serialize(&p).unwrap().replace('\n', "\r\n"));
        let back = parse(&text, &p.id, Scope::Global).unwrap();
        assert_eq!(back.body, p.body);
        assert_eq!(back.title, p.title);
    }

    #[test]
    fn reports_line_of_error() {
        let id = PromptId::parse("x").unwrap();
        let raw = "+++\ntitle = \"a\"\ntags = 3\ncreated_at = \"2026-09-30T12:00:00+09:00\"\nupdated_at = \"2026-09-30T12:00:00+09:00\"\n+++\nbody";
        match parse(raw, &id, Scope::Local) {
            Err(PhError::InvalidFormat { line, .. }) => assert_eq!(line, Some(3)),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn missing_delimiters() {
        let id = PromptId::parse("x").unwrap();
        assert!(parse("hello", &id, Scope::Local).is_err());
        assert!(parse("+++\ntitle = \"a\"\n", &id, Scope::Local).is_err());
    }
}
