//! 이식성 검증 (SPEC 2절 id 규칙, 6절 이식성 규칙, ARCHITECTURE 8절).

mod common;

use std::path::{Path, PathBuf};

use common::*;
use ph::core::error::PhError;
use ph::core::format;
use ph::core::model::{Prompt, PromptId, Scope};

// ---------- id 규칙 ----------

#[test]
fn from_title_replaces_only_whitespace_and_keeps_case_and_hangul() {
    for (title, want) in [
        ("코드 리뷰", "코드-리뷰"),
        ("Code Review", "Code-Review"),
        ("PR 리뷰  요청", "PR-리뷰-요청"),
        ("  앞뒤 공백  ", "앞뒤-공백"),
        ("a\u{3000}b", "a-b"),         // 전각 공백
        ("a\u{00a0}\u{00a0}b", "a-b"), // NBSP 연속
        ("MiXed_Case.v2", "MiXed_Case.v2"),
        ("한", "한"),
    ] {
        assert_eq!(
            PromptId::from_title(title).unwrap().as_str(),
            want,
            "{title:?}"
        );
    }
}

#[test]
fn from_title_rejects_each_forbidden_char_without_silent_change() {
    for c in ['/', '\\', ':', '*', '?', '"', '<', '>', '|'] {
        let title = format!("a{c}b");
        assert!(
            matches!(PromptId::from_title(&title), Err(PhError::InvalidId { .. })),
            "{c:?}"
        );
        assert!(PromptId::parse(&title).is_err(), "{c:?}");
    }
}

#[test]
fn from_title_rejects_control_chars_inside() {
    for c in ['\u{0}', '\u{1f}', '\t', '\n', '\r', '\u{7f}'] {
        let title = format!("a{c}b");
        assert!(PromptId::from_title(&title).is_err(), "{c:?}");
    }
}

#[test]
fn from_title_rejects_empty_and_blank() {
    for t in ["", " ", "   ", "\u{3000}"] {
        assert!(PromptId::from_title(t).is_err(), "{t:?}");
    }
}

#[test]
fn ids_ending_with_dot_are_rejected_but_inner_dot_is_ok() {
    assert!(PromptId::from_title("end.").is_err());
    assert!(PromptId::from_title("end...").is_err());
    assert!(PromptId::parse("v1.2").is_ok());
}

#[test]
fn windows_reserved_names_rejected_case_insensitively() {
    let mut names: Vec<String> = ["CON", "PRN", "AUX", "NUL"].map(String::from).to_vec();
    for i in 1..=9 {
        names.push(format!("COM{i}"));
        names.push(format!("LPT{i}"));
    }
    for n in names {
        for variant in [
            n.clone(),
            n.to_lowercase(),
            format!("{}{}", &n[..1], n[1..].to_lowercase()),
        ] {
            assert!(PromptId::parse(&variant).is_err(), "{variant}");
            assert!(PromptId::from_title(&variant).is_err(), "{variant}");
        }
    }
}

#[test]
fn reserved_names_with_extension_like_suffix_are_rejected() {
    // Windows 는 `CON.txt` 도 장치로 취급한다.
    for n in ["con.txt", "NUL.md", "Com3.x"] {
        assert!(PromptId::parse(n).is_err(), "{n}");
    }
}

#[test]
fn names_that_only_resemble_reserved_are_accepted() {
    for n in [
        "COM0", "COM10", "LPT0", "CONSOLE", "CON-1", "my-con", "NULL", "AUX2", "PRN1",
    ] {
        assert!(PromptId::parse(n).is_ok(), "{n}");
    }
}

#[test]
fn reserved_title_with_space_that_becomes_valid_slug_is_ok() {
    // "CON 1" -> "CON-1" 은 예약어가 아니다.
    assert_eq!(PromptId::from_title("CON 1").unwrap().as_str(), "CON-1");
}

#[test]
fn id_length_limit_is_100_utf8_bytes() {
    assert!(PromptId::parse(&"a".repeat(100)).is_ok());
    assert!(PromptId::parse(&"a".repeat(101)).is_err());
    assert!(PromptId::parse(&"가".repeat(33)).is_ok()); // 99
    assert!(PromptId::parse(&"가".repeat(34)).is_err()); // 102
                                                         // 99 + ASCII 1 = 100 (경계)
    assert!(PromptId::parse(&format!("{}a", "가".repeat(33))).is_ok());
    assert!(PromptId::from_title(&"가 ".repeat(40)).is_err()); // 슬러그 후에도 너무 김
}

#[test]
fn id_is_nfc_normalized_for_parse_and_from_title() {
    let nfd = "\u{1112}\u{1161}\u{11ab}\u{1100}\u{1173}\u{11af}"; // 한글 (NFD)
    let nfc = "한글";
    assert_ne!(nfd, nfc);
    assert_eq!(PromptId::parse(nfd).unwrap().as_str(), nfc);
    assert_eq!(PromptId::from_title(nfd).unwrap().as_str(), nfc);
    assert_eq!(PromptId::parse(nfd).unwrap(), PromptId::parse(nfc).unwrap());
    // 라틴 결합 문자: e + U+0301 -> é
    assert_eq!(PromptId::parse("e\u{301}").unwrap().as_str(), "\u{e9}");
}

#[test]
fn nfd_length_is_measured_after_normalization() {
    // NFD 로는 33*3*... 바이트가 넘지만 NFC 로는 99바이트.
    let nfd: String = "\u{1112}\u{1161}\u{11ab}".repeat(33);
    assert!(nfd.len() > 100);
    assert!(PromptId::parse(&nfd).is_ok());
}

#[test]
fn parse_rejects_whitespace_inside_id() {
    assert!(PromptId::parse("a b").is_err());
    assert!(PromptId::parse(" a").is_err());
}

#[test]
fn fold_key_is_case_insensitive_but_ids_keep_case() {
    let a = PromptId::parse("Code-Review").unwrap();
    let b = PromptId::parse("CODE-review").unwrap();
    assert_ne!(a, b);
    assert_eq!(a.fold_key(), b.fold_key());
    assert_eq!(a.as_str(), "Code-Review");
}

#[test]
fn with_suffix_stays_within_limit_and_valid() {
    for base in [
        "a".repeat(100),
        "가".repeat(33),
        format!("{}a", "가".repeat(33)),
    ] {
        let id = PromptId::parse(&base).unwrap();
        for n in [2, 10, 100, 12345] {
            let s = id.with_suffix(n).unwrap();
            assert!(s.as_str().len() <= 100, "{} bytes", s.as_str().len());
            assert!(s.as_str().ends_with(&format!("-{n}")));
            assert!(PromptId::parse(s.as_str()).is_ok());
        }
    }
}

#[test]
fn service_add_duplicate_suffix_is_case_insensitive_and_length_safe() {
    let s = memory_service(false);
    let long = "가".repeat(33);
    let a = s
        .add(new_prompt(&long), ph::core::service::WriteTarget::Auto)
        .unwrap();
    let b = s
        .add(new_prompt(&long), ph::core::service::WriteTarget::Auto)
        .unwrap();
    assert_ne!(a.prompt.id, b.prompt.id);
    assert!(b.prompt.id.as_str().len() <= 100);
    let c = s
        .add(new_prompt("Hello"), ph::core::service::WriteTarget::Auto)
        .unwrap();
    let d = s
        .add(new_prompt("HELLO"), ph::core::service::WriteTarget::Auto)
        .unwrap();
    assert_eq!(c.prompt.id.as_str(), "Hello");
    assert_eq!(d.prompt.id.as_str(), "HELLO-2");
}

// ---------- CRLF / BOM ----------

fn sample() -> Prompt {
    let mut p = prompt("코드-리뷰", Scope::Global);
    p.title = "코드 리뷰".into();
    p.body = "첫 줄 {{focus}}\n둘째 줄\n\n마지막".into();
    p
}

fn parse_p(raw: &str) -> Result<Prompt, PhError> {
    format::parse(raw, &PromptId::parse("코드-리뷰").unwrap(), Scope::Global)
}

#[test]
fn parse_accepts_lf_crlf_bom_and_mixed_combinations() {
    let p = sample();
    let lf = format::serialize(&p).unwrap();
    let crlf = lf.replace('\n', "\r\n");
    let bom = "\u{feff}";
    // 앞쪽 줄은 CRLF, 본문은 LF 인 혼합
    let mixed = lf.replacen('\n', "\r\n", 3);
    for (name, raw) in [
        ("lf", lf.clone()),
        ("crlf", crlf.clone()),
        ("bom+lf", format!("{bom}{lf}")),
        ("bom+crlf", format!("{bom}{crlf}")),
        ("mixed", mixed),
    ] {
        let got = parse_p(&raw).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(got, p, "{name}");
    }
}

#[test]
fn parse_normalizes_crlf_in_body_to_lf() {
    let raw = "+++\r\ntitle = \"t\"\r\ncreated_at = \"2026-09-30T12:00:00+09:00\"\r\nupdated_at = \"2026-09-30T12:00:00+09:00\"\r\n+++\r\na\r\nb\r\n";
    let p = parse_p(raw).unwrap();
    assert_eq!(p.body, "a\nb\n");
    assert!(!p.body.contains('\r'));
}

#[test]
fn serialize_always_writes_lf_only_and_no_bom() {
    let mut p = sample();
    p.body = "a\r\nb\r\n".into();
    let text = format::serialize(&p).unwrap();
    assert!(!text.contains('\r'));
    assert!(!text.starts_with('\u{feff}'));
    assert!(text.starts_with("+++\n"));
}

#[test]
fn parse_with_bom_only_at_start_and_missing_delimiters_fail_with_error() {
    assert!(parse_p("\u{feff}hello").is_err());
    assert!(parse_p("").is_err());
    assert!(parse_p("\u{feff}").is_err());
    // 여는 +++ 앞에 빈 줄
    assert!(parse_p("\n+++\ntitle=\"a\"\n+++\n").is_err());
}

#[test]
fn parse_empty_body_and_body_containing_delimiter_like_text() {
    let base = "+++\ntitle = \"t\"\ncreated_at = \"2026-09-30T12:00:00+09:00\"\nupdated_at = \"2026-09-30T12:00:00+09:00\"\n+++\n";
    assert_eq!(parse_p(base).unwrap().body, "");
    let with_body = format!("{base}본문\n+++\n뒤\n");
    // 닫는 구분자는 첫 번째 +++ 줄이므로 본문의 +++ 는 그대로 보존된다.
    assert_eq!(parse_p(&with_body).unwrap().body, "본문\n+++\n뒤\n");
}

#[test]
fn roundtrip_keeps_unicode_very_long_body_and_special_text() {
    let mut p = sample();
    p.body = format!(
        "{}\n{{{{a}}}} \"quote\" 'single' \\ backslash\n",
        "가나다 ".repeat(50_000)
    );
    p.title = "제목 \"따옴표\" \\ 역슬래시 🚀".into();
    p.description = Some("설명: 한 줄".into());
    let text = format::serialize(&p).unwrap();
    assert_eq!(parse_p(&text).unwrap(), p);
}

#[test]
fn broken_frontmatter_reports_invalid_format_with_line() {
    let raw = "+++\ntitle = \"a\"\nnot toml here\n+++\nbody";
    match parse_p(raw) {
        Err(PhError::InvalidFormat { line, .. }) => assert_eq!(line, Some(3)),
        other => panic!("unexpected: {other:?}"),
    }
    // 필수 필드 누락
    assert!(matches!(
        parse_p("+++\ntitle = \"a\"\n+++\nbody"),
        Err(PhError::InvalidFormat { .. })
    ));
    // 잘못된 시각
    assert!(matches!(
        parse_p("+++\ntitle = \"a\"\ncreated_at = \"어제\"\nupdated_at = \"x\"\n+++\n"),
        Err(PhError::InvalidFormat { .. })
    ));
}

#[test]
fn multiline_description_containing_delimiter_line_roundtrips() {
    // 설명/제목에 개행이 들어가도 저장 후 다시 읽을 수 있어야 한다 (update 는 개행을 막지 않는다).
    let mut p = sample();
    p.description = Some("앞\n+++\n뒤".into());
    let text = format::serialize(&p).unwrap();
    let back = parse_p(&text);
    assert_eq!(back.unwrap(), p, "직렬화 결과:\n{text}");
}

// ---------- OS 종속 코드 grep 점검 ----------

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            rs_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// `platform/` 밖의 프로덕션 코드(첫 `#[cfg(test)]` 이전)를 (상대 경로, 줄 번호, 줄) 로 돌려준다.
fn non_platform_prod_lines() -> Vec<(String, Vec<(usize, String)>)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rs_files(&root, &mut files);
    let mut out = Vec::new();
    for f in files {
        let rel = f.strip_prefix(&root).unwrap();
        if rel
            .components()
            .next()
            .is_some_and(|c| c.as_os_str() == "platform")
        {
            continue;
        }
        let text = std::fs::read_to_string(&f).unwrap();
        let mut lines = Vec::new();
        for (i, l) in text.lines().enumerate() {
            if l.trim() == "#[cfg(test)]" {
                break;
            }
            let t = l.trim_start();
            if t.starts_with("//") {
                continue;
            }
            lines.push((i + 1, l.to_string()));
        }
        out.push((rel.display().to_string(), lines));
    }
    out
}

fn assert_no_match(patterns: &[&str], why: &str) {
    let mut hits = Vec::new();
    for (file, lines) in non_platform_prod_lines() {
        for (n, l) in lines {
            if patterns.iter().any(|p| l.contains(p)) {
                hits.push(format!("{file}:{n}: {l}"));
            }
        }
    }
    assert!(hits.is_empty(), "{why}:\n{}", hits.join("\n"));
}

#[test]
fn no_os_specific_code_outside_platform() {
    assert_no_match(
        &[
            "cfg(unix)",
            "cfg(windows)",
            "target_os",
            "target_family",
            "std::os::",
            "libc",
        ],
        "OS 종속 코드는 platform/ 안에만 둔다",
    );
}

#[test]
fn no_direct_env_or_home_dirs_outside_platform() {
    assert_no_match(
        &[
            "XDG_",
            "\"HOME\"",
            "USERPROFILE",
            "APPDATA",
            "std::env",
            "env::var",
            "directories::",
            "home_dir",
            "current_dir",
        ],
        "환경변수와 홈/데이터 디렉터리는 platform 에서만 다룬다",
    );
}

#[test]
fn no_hardcoded_path_separators_or_tilde_outside_platform() {
    assert_no_match(
        &[
            "\"/\"",
            "\"\\\\\"",
            "\"~",
            "'~'",
            "MAIN_SEPARATOR",
            "\"/tmp",
            "\"C:",
        ],
        "경로는 Path/PathBuf 로만 다룬다",
    );
}

#[test]
fn core_does_not_depend_on_io_or_other_layers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("core");
    let mut files = Vec::new();
    rs_files(&root, &mut files);
    let mut hits = Vec::new();
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        for (i, l) in text.lines().enumerate() {
            if l.trim() == "#[cfg(test)]" {
                break;
            }
            for p in [
                "crate::platform",
                "crate::storage",
                "crate::cli",
                "crate::tui",
                "crate::skill",
                "std::fs",
                "std::process",
                "std::env",
                "std::net",
            ] {
                if l.contains(p) {
                    hits.push(format!("{}:{}: {l}", f.display(), i + 1));
                }
            }
        }
    }
    assert!(hits.is_empty(), "core 의존 방향 위반:\n{}", hits.join("\n"));
}

#[test]
fn platform_does_not_depend_on_core_or_storage() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("platform");
    let mut files = Vec::new();
    rs_files(&root, &mut files);
    let mut hits = Vec::new();
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        for (i, l) in text.lines().enumerate() {
            if l.trim() == "#[cfg(test)]" {
                break;
            }
            for p in [
                "crate::core",
                "crate::storage",
                "crate::cli",
                "crate::tui",
                "ph::core",
            ] {
                if l.contains(p) {
                    hits.push(format!("{}:{}: {l}", f.display(), i + 1));
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "platform 의존 방향 위반:\n{}",
        hits.join("\n")
    );
}

#[test]
fn production_code_has_no_unwrap_or_expect() {
    // ARCHITECTURE 11절: 테스트 밖 unwrap/expect 금지.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rs_files(&root, &mut files);
    let mut hits = Vec::new();
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        for (i, l) in text.lines().enumerate() {
            if l.trim() == "#[cfg(test)]" {
                break;
            }
            if l.trim_start().starts_with("//") {
                continue;
            }
            if l.contains(".unwrap()") || l.contains(".expect(") {
                hits.push(format!("{}:{}: {l}", f.display(), i + 1));
            }
        }
    }
    assert!(hits.is_empty(), "{}", hits.join("\n"));
}
