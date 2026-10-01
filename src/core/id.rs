//! id 생성과 검증 (SPEC 2절 id 규칙).

use unicode_normalization::UnicodeNormalization;

use super::error::PhError;

/// id 최대 길이 (UTF-8 바이트 기준).
pub const MAX_ID_BYTES: usize = 100;

const FORBIDDEN: [char; 9] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
const RESERVED: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];

fn invalid(input: &str, reason: impl Into<String>) -> PhError {
    PhError::InvalidId {
        input: input.to_string(),
        reason: reason.into(),
    }
}

/// Windows 예약 이름인지 (대소문자 무관). 확장자처럼 붙은 `CON.foo` 도 예약으로 본다.
fn is_reserved(id: &str) -> bool {
    let stem = id.split('.').next().unwrap_or(id).to_uppercase();
    if RESERVED.contains(&stem.as_str()) {
        return true;
    }
    for prefix in ["COM", "LPT"] {
        if let Some(rest) = stem.strip_prefix(prefix) {
            if matches!(rest.as_bytes(), [b'1'..=b'9']) {
                return true;
            }
        }
    }
    false
}

/// 이미 NFC 로 정규화된 문자열이 id 규칙을 지키는지 검사한다. `input` 은 에러 표시용 원문이다.
pub(super) fn validate_normalized(input: &str, id: &str) -> Result<(), PhError> {
    if id.is_empty() {
        return Err(invalid(input, "비어 있습니다. 글자가 있는 제목을 쓰세요"));
    }
    if let Some(c) = id.chars().find(|c| FORBIDDEN.contains(c)) {
        return Err(invalid(
            input,
            format!("사용할 수 없는 문자 '{c}' 가 있습니다 (금지: / \\ : * ? \" < > |). 제목에서 빼세요"),
        ));
    }
    if id.chars().any(char::is_control) {
        return Err(invalid(input, "제어 문자가 있습니다. 제목에서 빼세요"));
    }
    if id.chars().any(char::is_whitespace) {
        return Err(invalid(
            input,
            "공백이 있습니다. id 에서는 공백을 '-' 로 써야 합니다",
        ));
    }
    if id.ends_with('.') {
        return Err(invalid(input, "'.' 로 끝날 수 없습니다"));
    }
    if is_reserved(id) {
        return Err(invalid(
            input,
            "Windows 예약 이름(CON, PRN, AUX, NUL, COM1~9, LPT1~9)입니다. 다른 이름을 쓰세요",
        ));
    }
    if id.len() > MAX_ID_BYTES {
        return Err(invalid(
            input,
            format!(
                "UTF-8 기준 {MAX_ID_BYTES}바이트를 넘습니다 ({}바이트). 더 짧게 쓰세요",
                id.len()
            ),
        ));
    }
    Ok(())
}

/// title 을 id 문자열로 바꾼다. 앞뒤 공백 제거, 연속 공백은 '-' 하나. 검증은 하지 않는다.
pub(super) fn slug_from_title(title: &str) -> String {
    let nfc: String = title.nfc().collect();
    let mut out = String::with_capacity(nfc.len());
    let mut pending_gap = false;
    for c in nfc.trim().chars() {
        // 제어 문자(탭, 개행 포함)는 공백으로 취급하지 않고 그대로 두어 검증에서 에러가 되게 한다.
        if c.is_whitespace() && !c.is_control() {
            pending_gap = true;
            continue;
        }
        if pending_gap {
            out.push('-');
            pending_gap = false;
        }
        out.push(c);
    }
    out
}

/// 접미사를 붙이면서 전체 길이가 한도를 넘지 않도록 base 를 글자 경계에서 자른다.
pub(super) fn with_suffix_str(base: &str, n: u32) -> String {
    let suffix = format!("-{n}");
    let mut end = base.len().min(MAX_ID_BYTES.saturating_sub(suffix.len()));
    while !base.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{suffix}", &base[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_replaces_only_whitespace() {
        assert_eq!(slug_from_title("코드 리뷰"), "코드-리뷰");
        assert_eq!(slug_from_title("Code Review"), "Code-Review");
        assert_eq!(slug_from_title("  PR 리뷰  요청 "), "PR-리뷰-요청");
    }

    #[test]
    fn reserved_names() {
        for n in ["con", "PRN", "Aux", "nul", "COM1", "lpt9", "CON.txt"] {
            assert!(is_reserved(n), "{n}");
        }
        for n in ["COM0", "COM10", "CONSOLE", "console-x"] {
            assert!(!is_reserved(n), "{n}");
        }
    }

    #[test]
    fn suffix_respects_limit_on_char_boundary() {
        let base = "가".repeat(33); // 99 bytes
        let s = with_suffix_str(&base, 2);
        assert!(s.len() <= MAX_ID_BYTES);
        assert!(s.ends_with("-2"));
    }
}
