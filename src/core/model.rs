//! 도메인 모델.

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};

use super::error::PhError;
use super::id;

/// 저장소 범위.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// 프로젝트 전용 (`.ph/prompts/`)
    Local,
    /// 개인 전역
    Global,
}

impl Scope {
    /// `"local"` / `"global"` 문자열.
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::Local => "local",
            Scope::Global => "global",
        }
    }

    /// 반대편 scope.
    pub fn other(self) -> Scope {
        match self {
            Scope::Local => Scope::Global,
            Scope::Global => Scope::Local,
        }
    }
}

/// 검증된 prompt id. 생성자가 SPEC 2절 규칙(NFC, 금지 문자, 예약어, 100바이트)을 강제한다.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PromptId(String);

impl PromptId {
    /// 기존 id 문자열을 검증한다. NFC 로 정규화하며, 공백이 있으면 에러다.
    pub fn parse(s: &str) -> Result<Self, PhError> {
        let nfc: String = unicode_normalization::UnicodeNormalization::nfc(s).collect();
        id::validate_normalized(s, &nfc)?;
        Ok(PromptId(nfc))
    }

    /// title 에서 id 를 만든다. 공백만 '-' 로 바꾸고 금지 문자는 에러로 처리한다.
    pub fn from_title(title: &str) -> Result<Self, PhError> {
        let slug = id::slug_from_title(title);
        id::validate_normalized(title, &slug)?;
        Ok(PromptId(slug))
    }

    /// 중복 회피용 접미사를 붙인 새 id (`이름-2`). 길이 한도를 넘으면 앞부분을 자른다.
    pub fn with_suffix(&self, n: u32) -> Result<Self, PhError> {
        let s = id::with_suffix_str(&self.0, n);
        id::validate_normalized(&s, &s)?;
        Ok(PromptId(s))
    }

    /// id 문자열.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 대소문자를 무시한 비교용 키.
    pub fn fold_key(&self) -> String {
        self.0.to_lowercase()
    }
}

impl std::fmt::Display for PromptId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// prompt 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    /// scope 안에서 고유한 id
    pub id: PromptId,
    /// 저장 위치. 파일에는 없고 `Storage` 가 채운다
    pub scope: Scope,
    /// 표시 이름
    pub title: String,
    /// 본문
    pub body: String,
    /// 태그
    pub tags: Vec<String>,
    /// 짧은 설명
    pub description: Option<String>,
    /// 생성 시각
    pub created_at: DateTime<FixedOffset>,
    /// 수정 시각
    pub updated_at: DateTime<FixedOffset>,
}

/// 새 prompt 입력.
#[derive(Debug, Clone, Default)]
pub struct NewPrompt {
    /// 표시 이름 (id 의 원본)
    pub title: String,
    /// 본문
    pub body: String,
    /// 태그
    pub tags: Vec<String>,
    /// 설명
    pub description: Option<String>,
}

/// 부분 수정. `None` 인 필드는 바꾸지 않는다.
#[derive(Debug, Clone, Default)]
pub struct PromptPatch {
    /// 새 title (id 는 바뀌지 않는다)
    pub title: Option<String>,
    /// 새 본문
    pub body: Option<String>,
    /// 새 태그 목록 (전체 교체)
    pub tags: Option<Vec<String>>,
    /// 설명. `Some(None)` 은 설명 삭제
    pub description: Option<Option<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_title_examples() {
        for (t, want) in [
            ("코드 리뷰", "코드-리뷰"),
            ("Code Review", "Code-Review"),
            ("PR 리뷰  요청", "PR-리뷰-요청"),
        ] {
            assert_eq!(PromptId::from_title(t).unwrap().as_str(), want);
        }
    }

    #[test]
    fn rejects_forbidden_and_bad_endings() {
        for t in [
            "a/b", "a\\b", "a:b", "a*b", "a?b", "a\"b", "a<b", "a>b", "a|b", "a\tb", "x.", "",
            "   ", "CON", "com1",
        ] {
            assert!(PromptId::from_title(t).is_err(), "{t:?}");
        }
    }

    #[test]
    fn length_is_bytes() {
        assert!(PromptId::parse(&"가".repeat(33)).is_ok()); // 99
        assert!(PromptId::parse(&"가".repeat(34)).is_err()); // 102
        assert!(PromptId::parse(&"a".repeat(100)).is_ok());
        assert!(PromptId::parse(&"a".repeat(101)).is_err());
    }

    #[test]
    fn nfc_normalization() {
        let nfd = "\u{1112}\u{1161}\u{11ab}"; // 한 (NFD)
        let id = PromptId::parse(nfd).unwrap();
        assert_eq!(id.as_str(), "한");
    }

    #[test]
    fn fold_key_ignores_case() {
        let a = PromptId::parse("Code-Review").unwrap();
        let b = PromptId::parse("code-review").unwrap();
        assert_ne!(a, b);
        assert_eq!(a.fold_key(), b.fold_key());
    }

    #[test]
    fn suffix() {
        let id = PromptId::parse("이름").unwrap();
        assert_eq!(id.with_suffix(2).unwrap().as_str(), "이름-2");
    }
}
