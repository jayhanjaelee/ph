//! CLI 통합: `init`, `add` (SPEC 4절, ARCHITECTURE 10.4절).

mod common;

use common::cli::*;
use std::path::Path;

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap()
}

// ---------- init ----------

#[test]
fn init_creates_prompts_dir_and_empty_gitkeep_and_prints_path_on_stdout() {
    let sb = Sandbox::new();
    let o = sb.run(&["init"]).ok();
    assert!(sb.local_dir().is_dir());
    let keep = sb.local_dir().join(".gitkeep");
    assert_eq!(std::fs::metadata(&keep).unwrap().len(), 0);
    assert_eq!(o.stdout_lines().len(), 1);
    assert!(same_path(&o.stdout, &sb.proj.join(".ph")), "{o:?}");
    assert!(o.stderr.contains("local 저장소를 만들었습니다"), "{o:?}");
}

#[test]
fn init_twice_is_ok_and_keeps_existing_prompts() {
    let sb = Sandbox::new();
    sb.run(&["init"]).ok();
    sb.add("보존", Some("--local"));
    let o = sb.run(&["init"]).ok();
    assert!(o.stderr.contains("이미 초기화되어 있습니다"), "{o:?}");
    assert!(same_path(&o.stdout, &sb.proj.join(".ph")));
    assert!(sb.local_dir().join("보존.md").is_file());
}

#[test]
fn init_json_reports_created_flag() {
    let sb = Sandbox::new();
    let v = sb.run(&["init", "--json"]).ok().json();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["created"], true);
    assert!(same_path(v["path"].as_str().unwrap(), &sb.proj.join(".ph")));
    let v = sb.run(&["init", "--json"]).ok().json();
    assert_eq!(v["created"], false);
}

#[test]
fn init_in_home_directory_is_usage_error_exit_2() {
    let sb = Sandbox::new();
    let o = sb.run_in(&sb.home, &["init"]).code(2);
    assert!(o.stdout.is_empty(), "{o:?}");
    assert!(o.stderr.starts_with("오류:"), "{o:?}");
    assert!(!sb.home.join(".ph").exists());
}

#[test]
fn init_does_not_create_global_directory() {
    let sb = Sandbox::new();
    sb.run(&["init"]).ok();
    assert!(!sb.ph_home.exists());
}

#[test]
fn init_in_subdirectory_of_existing_local_notes_parent() {
    let sb = Sandbox::with_local();
    let sub = sb.proj.join("sub");
    std::fs::create_dir(&sub).unwrap();
    let o = sb.run_in(&sub, &["init"]).ok();
    assert!(sub.join(".ph").join("prompts").is_dir());
    assert!(o.stderr.contains("참고: 상위"), "{o:?}");
}

#[test]
fn init_fails_with_exit_1_when_dot_ph_is_a_file() {
    let sb = Sandbox::new();
    std::fs::write(sb.proj.join(".ph"), "x").unwrap();
    let o = sb.run(&["init"]).code(1);
    assert!(o.stdout.is_empty());
    assert!(o.stderr.starts_with("오류:"));
}

#[test]
fn local_is_found_from_subdirectory_after_init() {
    let sb = Sandbox::with_local();
    let sub = sb.proj.join("a").join("b");
    std::fs::create_dir_all(&sub).unwrap();
    let o = sb
        .exec(&sub, &["add", "깊은곳", "--body", "x"], None, &[])
        .ok();
    assert_eq!(o.stdout, "깊은곳\n");
    assert!(sb.local_dir().join("깊은곳.md").is_file());
}

// ---------- add: 본문 소스 ----------

#[test]
fn add_body_prints_id_on_stdout_and_creates_file_in_global_when_no_local() {
    let sb = Sandbox::new();
    let o = sb.run(&["add", "Code Review", "--body", "본문"]).ok();
    assert_eq!(o.stdout, "Code-Review\n");
    assert!(o.stderr.contains("저장됨: [G] Code-Review"), "{o:?}");
    let text = read(&sb.global_dir().join("Code-Review.md"));
    assert!(text.starts_with("+++\n"));
    assert!(text.ends_with("+++\n본문"));
}

#[test]
fn add_hangul_title_becomes_hangul_file_name() {
    let sb = Sandbox::new();
    let o = sb.run(&["add", "코드 리뷰", "--body", "x"]).ok();
    assert_eq!(o.stdout, "코드-리뷰\n");
    assert!(sb.global_dir().join("코드-리뷰.md").is_file());
    let v = sb.run(&["get", "코드-리뷰", "--json"]).ok().json();
    assert_eq!(v["prompt"]["title"], "코드 리뷰");
}

#[test]
fn add_goes_to_local_when_local_exists_and_says_so_on_stderr() {
    let sb = Sandbox::with_local();
    let o = sb.run(&["add", "x", "--body", "b"]).ok();
    assert!(sb.local_dir().join("x.md").is_file());
    assert!(!sb.global_dir().join("x.md").exists());
    assert!(o.stderr.contains("저장됨: [L] x"), "{o:?}");
}

#[test]
fn add_with_explicit_scope_is_silent_on_stderr_and_respected() {
    let sb = Sandbox::with_local();
    let o = sb.run(&["add", "g", "--body", "b", "--global"]).ok();
    assert!(o.stderr.is_empty(), "{o:?}");
    assert!(sb.global_dir().join("g.md").is_file());
    let o = sb.run(&["add", "l", "--body", "b", "--local"]).ok();
    assert!(o.stderr.is_empty(), "{o:?}");
    assert!(sb.local_dir().join("l.md").is_file());
}

#[test]
fn add_file_reads_body_and_strips_bom_and_crlf_is_stored_as_lf() {
    let sb = Sandbox::new();
    let f = sb.root.path().join("body.txt");
    std::fs::write(&f, "\u{feff}첫줄\r\n둘째줄\r\n").unwrap();
    sb.run(&["add", "f", "--file", f.to_str().unwrap()]).ok();
    let got = sb.run(&["get", "f"]).ok();
    assert_eq!(got.stdout, "첫줄\n둘째줄\n");
    assert!(!read(&sb.global_dir().join("f.md")).contains('\r'));
}

#[test]
fn add_stdin_reads_until_eof_including_large_input() {
    let sb = Sandbox::new();
    let big = "가나다라\n".repeat(200_000);
    sb.run_stdin(&["add", "s", "--stdin"], big.as_bytes()).ok();
    assert_eq!(sb.run(&["get", "s"]).ok().stdout, big);
}

#[test]
fn add_stdin_empty_input_creates_empty_body() {
    let sb = Sandbox::new();
    sb.run_stdin(&["add", "e", "--stdin"], b"").ok();
    let o = sb.run(&["get", "e"]).ok();
    assert_eq!(o.stdout, "");
}

#[test]
fn add_empty_body_is_allowed() {
    let sb = Sandbox::new();
    sb.run(&["add", "e", "--body", ""]).ok();
    assert_eq!(sb.run(&["get", "e"]).ok().stdout, "");
}

#[test]
fn add_without_any_body_source_is_usage_error_exit_2_and_writes_nothing() {
    let sb = Sandbox::new();
    let o = sb.run(&["add", "x"]).code(2);
    assert!(o.stdout.is_empty(), "{o:?}");
    assert!(!o.stderr.is_empty());
    assert!(!sb.ph_home.exists());
}

#[test]
fn add_multiple_body_sources_is_usage_error_exit_2() {
    let sb = Sandbox::new();
    sb.run_stdin(&["add", "x", "--body", "a", "--stdin"], b"z")
        .code(2);
    sb.run(&["add", "x", "--body", "a", "--file", "f"]).code(2);
    assert!(!sb.ph_home.exists());
}

#[test]
fn add_missing_file_is_io_error_exit_1() {
    let sb = Sandbox::new();
    let missing = sb.root.path().join("nope.txt");
    let o = sb
        .run(&["add", "x", "--file", missing.to_str().unwrap()])
        .code(1);
    assert!(o.stdout.is_empty());
    assert!(o.stderr.starts_with("오류:"), "{o:?}");
    assert!(!sb.ph_home.exists());
}

#[test]
fn add_non_utf8_file_is_rejected_without_writing() {
    let sb = Sandbox::new();
    let f = sb.root.path().join("bin.dat");
    std::fs::write(&f, [0xff, 0xfe, 0x00, 0x80]).unwrap();
    let o = sb.run(&["add", "x", "--file", f.to_str().unwrap()]);
    assert_ne!(o.code, 0, "{o:?}");
    assert!(o.stdout.is_empty());
    assert!(!sb.global_dir().join("x.md").exists());
}

#[test]
fn add_body_preserves_exact_text_without_added_newline() {
    let sb = Sandbox::new();
    sb.run(&["add", "a", "--body", "줄1\n줄2"]).ok();
    sb.run(&["add", "b", "--body", "줄1\n줄2\n"]).ok();
    assert_eq!(sb.run(&["get", "a"]).ok().stdout, "줄1\n줄2");
    assert_eq!(sb.run(&["get", "b"]).ok().stdout, "줄1\n줄2\n");
}

// ---------- add: 중복, 검증 ----------

#[test]
fn add_duplicate_title_gets_numeric_suffix_case_insensitively() {
    let sb = Sandbox::new();
    assert_eq!(sb.run(&["add", "Dup", "--body", "1"]).ok().stdout, "Dup\n");
    assert_eq!(
        sb.run(&["add", "Dup", "--body", "2"]).ok().stdout,
        "Dup-2\n"
    );
    assert_eq!(
        sb.run(&["add", "dup", "--body", "3"]).ok().stdout,
        "dup-3\n"
    );
    assert_eq!(sb.run(&["get", "Dup"]).ok().stdout, "1");
}

#[test]
fn add_same_title_in_other_scope_does_not_get_suffix() {
    let sb = Sandbox::with_local();
    assert_eq!(
        sb.run(&["add", "x", "--body", "g", "--global"]).ok().stdout,
        "x\n"
    );
    assert_eq!(
        sb.run(&["add", "x", "--body", "l", "--local"]).ok().stdout,
        "x\n"
    );
}

#[test]
fn add_invalid_titles_fail_with_exit_1_and_write_nothing() {
    let sb = Sandbox::new();
    for t in ["a/b", "a:b", "CON", "nul", "끝.", "", "   ", "a\tb"] {
        let o = sb.run(&["add", t, "--body", "x"]);
        assert_eq!(o.code, 1, "{t:?}: {o:?}");
        assert!(o.stdout.is_empty(), "{t:?}");
        assert!(o.stderr.starts_with("오류:"), "{t:?}: {o:?}");
    }
    assert!(!sb.ph_home.exists());
}

#[test]
fn add_title_over_100_bytes_is_rejected() {
    let sb = Sandbox::new();
    let long = "가".repeat(34);
    sb.run(&["add", &long, "--body", "x"]).code(1);
    sb.run(&["add", &"가".repeat(33), "--body", "x"]).ok();
}

#[test]
fn add_tags_and_description_are_stored() {
    let sb = Sandbox::new();
    sb.run(&[
        "add", "t", "--body", "b", "--tag", "review", "--tag", "rust", "--desc", "설명",
    ])
    .ok();
    let v = sb.run(&["get", "t", "--json"]).ok().json();
    assert_eq!(v["prompt"]["tags"], serde_json::json!(["review", "rust"]));
    assert_eq!(v["prompt"]["description"], "설명");
}

#[test]
fn add_local_without_local_store_fails_exit_1_and_does_not_fall_back_to_global() {
    let sb = Sandbox::new();
    let o = sb.run(&["add", "x", "--body", "b", "--local"]).code(1);
    assert!(o.stdout.is_empty());
    assert!(o.stderr.contains("ph init"), "{o:?}");
    assert!(!sb.ph_home.exists());
}

#[test]
fn add_local_and_global_together_is_usage_error() {
    let sb = Sandbox::with_local();
    sb.run(&["add", "x", "--body", "b", "--local", "--global"])
        .code(2);
}

#[test]
fn add_json_schema_v1() {
    let sb = Sandbox::with_local();
    let o = sb
        .run(&[
            "add",
            "코드 리뷰",
            "--body",
            "본문 {{v}}",
            "--tag",
            "a",
            "--json",
        ])
        .ok();
    assert_eq!(o.stdout_lines().len(), 1);
    let v = o.json();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["auto_selected"], true);
    let p = &v["prompt"];
    assert_eq!(p["id"], "코드-리뷰");
    assert_eq!(p["scope"], "local");
    assert_eq!(p["title"], "코드 리뷰");
    assert_eq!(p["body"], "본문 {{v}}");
    assert!(p["description"].is_null());
    assert_eq!(p["tags"], serde_json::json!(["a"]));
    assert_eq!(p["shadowed"], false);
    for k in ["created_at", "updated_at"] {
        chrono::DateTime::parse_from_rfc3339(p[k].as_str().unwrap()).unwrap();
    }
    let v = sb
        .run(&["add", "y", "--body", "b", "--global", "--json"])
        .ok()
        .json();
    assert_eq!(v["auto_selected"], false);
    assert_eq!(v["prompt"]["scope"], "global");
}
