//! CLI 통합: `get`, `list`, `search`, 종료 코드, `--json` 스키마 v1, stdout/stderr 분리.

mod common;

use common::cli::*;
use std::process::Stdio;

const BAD: &str = "그냥 텍스트";

// ---------- get ----------

#[test]
fn get_prints_only_body_on_stdout_and_nothing_on_stderr() {
    let sb = Sandbox::new();
    sb.run(&["add", "g", "--body", "안녕 {{x}}\n둘째"]).ok();
    let o = sb.run(&["get", "g"]).ok();
    assert_eq!(o.stdout, "안녕 {{x}}\n둘째");
    assert!(o.stderr.is_empty(), "{o:?}");
}

#[test]
fn get_is_case_insensitive_on_id() {
    let sb = Sandbox::new();
    sb.add("Code Review", None);
    assert_eq!(
        sb.run(&["get", "code-review"]).ok().stdout,
        "Code Review 본문"
    );
}

#[test]
fn get_missing_id_is_exit_3_with_empty_stdout() {
    let sb = Sandbox::new();
    let o = sb.run(&["get", "none"]).code(3);
    assert!(o.stdout.is_empty());
    assert!(o.stderr.starts_with("오류:"), "{o:?}");
    assert!(o.stderr.contains("none"));
}

#[test]
fn get_invalid_id_is_exit_1() {
    let sb = Sandbox::new();
    let o = sb.run(&["get", "a/b"]).code(1);
    assert!(o.stdout.is_empty());
}

#[test]
fn get_ambiguous_returns_local_and_warns_on_stderr_only() {
    let sb = Sandbox::with_local();
    sb.run(&["add", "dup", "--body", "GLOBAL", "--global"]).ok();
    sb.run(&["add", "dup", "--body", "LOCAL", "--local"]).ok();
    let o = sb.run(&["get", "dup"]).ok();
    assert_eq!(o.stdout, "LOCAL");
    assert!(o.stderr.contains("경고"), "{o:?}");
    assert!(o.stderr.contains("양쪽"), "{o:?}");
    assert!(o.stderr.contains("--global"), "{o:?}");
}

#[test]
fn get_with_scope_flag_selects_scope_and_has_no_warning() {
    let sb = Sandbox::with_local();
    sb.run(&["add", "dup", "--body", "GLOBAL", "--global"]).ok();
    sb.run(&["add", "dup", "--body", "LOCAL", "--local"]).ok();
    let g = sb.run(&["get", "dup", "--global"]).ok();
    assert_eq!((g.stdout.as_str(), g.stderr.as_str()), ("GLOBAL", ""));
    let l = sb.run(&["get", "dup", "--local"]).ok();
    assert_eq!((l.stdout.as_str(), l.stderr.as_str()), ("LOCAL", ""));
}

#[test]
fn get_local_flag_without_local_is_exit_1() {
    let sb = Sandbox::new();
    sb.add("x", None);
    let o = sb.run(&["get", "x", "--local"]).code(1);
    assert!(o.stdout.is_empty());
    assert!(o.stderr.contains("ph init"), "{o:?}");
}

#[test]
fn get_global_only_prompt_from_local_project_merges() {
    let sb = Sandbox::with_local();
    sb.run(&["add", "only-g", "--body", "G", "--global"]).ok();
    let o = sb.run(&["get", "only-g"]).ok();
    assert_eq!((o.stdout.as_str(), o.stderr.as_str()), ("G", ""));
}

#[test]
fn get_json_schema_v1() {
    let sb = Sandbox::with_local();
    sb.run(&[
        "add",
        "코드 리뷰",
        "--body",
        "본문\n",
        "--tag",
        "a",
        "--desc",
        "d",
        "--local",
    ])
    .ok();
    let o = sb.run(&["get", "코드-리뷰", "--json"]).ok();
    assert_eq!(o.stdout_lines().len(), 1);
    assert!(o.stdout.ends_with('\n'));
    let v = o.json();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["ambiguous"], false);
    let p = &v["prompt"];
    assert_eq!(p["id"], "코드-리뷰");
    assert_eq!(p["scope"], "local");
    assert_eq!(p["title"], "코드 리뷰");
    assert_eq!(p["description"], "d");
    assert_eq!(p["body"], "본문\n");
    assert_eq!(p["tags"], serde_json::json!(["a"]));
    assert_eq!(p["shadowed"], false);
    let keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
    assert!(keys.contains(&"prompt".to_string()) && keys.contains(&"ambiguous".to_string()));
}

#[test]
fn get_json_ambiguous_flag_true_and_warning_still_on_stderr() {
    let sb = Sandbox::with_local();
    sb.run(&["add", "dup", "--body", "G", "--global"]).ok();
    sb.run(&["add", "dup", "--body", "L", "--local"]).ok();
    let o = sb.run(&["get", "dup", "--json"]).ok();
    let v = o.json();
    assert_eq!(v["ambiguous"], true);
    assert_eq!(v["prompt"]["scope"], "local");
    assert!(o.stderr.contains("경고"));
}

#[test]
fn get_broken_file_is_invalid_format_exit_1_not_not_found() {
    let sb = Sandbox::new();
    sb.write_file(&sb.global_dir(), "bad.md", BAD);
    let o = sb.run(&["get", "bad"]);
    assert_eq!(
        o.code, 1,
        "깨진 파일 get 은 NotFound(3) 가 아니라 InvalidFormat(1): {o:?}"
    );
    assert!(o.stdout.is_empty());
    assert!(o.stderr.contains("파일 형식 오류"), "{o:?}");
}

#[test]
fn get_broken_file_json_error_kind_is_invalid_format() {
    let sb = Sandbox::new();
    sb.write_file(&sb.global_dir(), "bad.md", "+++\ntitle = \n+++\nx");
    let o = sb.run(&["get", "bad", "--json"]);
    assert!(o.stdout.is_empty(), "{o:?}");
    assert_eq!(o.err_json()["error"]["kind"], "invalid_format", "{o:?}");
    assert_eq!(o.code, 1);
}

// ---------- list ----------

#[test]
fn list_empty_is_exit_0_with_empty_stdout_and_creates_nothing() {
    let sb = Sandbox::new();
    let o = sb.run(&["list"]).ok();
    assert!(o.stdout.is_empty() && o.stderr.is_empty(), "{o:?}");
    assert!(!sb.ph_home.exists(), "읽기 명령이 디스크를 바꿨다");
}

#[test]
fn list_text_lines_have_badge_id_title_tags_and_shadowed_marker() {
    let sb = Sandbox::with_local();
    sb.run(&[
        "add",
        "코드 리뷰",
        "--body",
        "b",
        "--tag",
        "review",
        "--tag",
        "rust",
        "--local",
    ])
    .ok();
    sb.run(&["add", "Code Review", "--body", "b", "--global"])
        .ok();
    sb.run(&["add", "Code Review", "--body", "b2", "--local"])
        .ok();
    let o = sb.run(&["list"]).ok();
    let lines = o.stdout_lines();
    assert_eq!(lines.len(), 3, "{o:?}");
    assert!(lines.iter().any(|l| l.starts_with("[L] 코드-리뷰")
        && l.contains("코드 리뷰")
        && l.contains("#review")
        && l.contains("#rust")));
    let shadowed: Vec<_> = lines.iter().filter(|l| l.contains("(shadowed)")).collect();
    assert_eq!(shadowed.len(), 1);
    assert!(shadowed[0].starts_with("[G] Code-Review"));
    assert!(o.stderr.is_empty());
}

#[test]
fn list_sorted_by_id_ignoring_case() {
    let sb = Sandbox::new();
    for t in ["banana", "Apple", "cherry"] {
        sb.add(t, None);
    }
    let o = sb.run(&["list"]).ok();
    let ids: Vec<_> = o
        .stdout_lines()
        .iter()
        .map(|l| l.split_whitespace().nth(1).unwrap().to_string())
        .collect();
    assert_eq!(ids, ["Apple", "banana", "cherry"]);
}

#[test]
fn list_tag_filter_and_scope_filters() {
    let sb = Sandbox::with_local();
    sb.run(&["add", "a", "--body", "b", "--tag", "x", "--global"])
        .ok();
    sb.run(&["add", "b", "--body", "b", "--local"]).ok();
    assert_eq!(sb.run(&["list", "--tag", "X"]).ok().stdout_lines().len(), 1);
    assert_eq!(sb.run(&["list", "--tag", "none"]).ok().stdout, "");
    let g = sb.run(&["list", "--global"]).ok();
    assert_eq!(g.stdout_lines().len(), 1);
    assert!(g.stdout.starts_with("[G] a"));
    let l = sb.run(&["list", "--local"]).ok();
    assert!(l.stdout.starts_with("[L] b") && l.stdout_lines().len() == 1);
}

#[test]
fn list_local_without_local_is_exit_1() {
    let sb = Sandbox::new();
    let o = sb.run(&["list", "--local"]).code(1);
    assert!(o.stdout.is_empty());
}

#[test]
fn list_json_schema_v1_and_field_order() {
    let sb = Sandbox::with_local();
    sb.run(&["add", "g", "--body", "b", "--global", "--tag", "t"])
        .ok();
    sb.run(&["add", "g", "--body", "b", "--local"]).ok();
    let o = sb.run(&["list", "--json"]).ok();
    assert_eq!(o.stdout_lines().len(), 1);
    let s = &o.stdout;
    let pos = |k: &str| {
        s.find(&format!("\"{k}\""))
            .unwrap_or_else(|| panic!("{k} 없음: {s}"))
    };
    assert!(
        pos("schema_version") < pos("count")
            && pos("count") < pos("prompts")
            && pos("prompts") < pos("warnings")
    );
    let v = o.json();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["count"], 2);
    assert_eq!(v["warnings"], serde_json::json!([]));
    let ps = v["prompts"].as_array().unwrap();
    assert_eq!(ps.len(), 2);
    assert_eq!(ps[0]["scope"], "local");
    assert_eq!(ps[0]["shadowed"], false);
    assert_eq!(ps[1]["scope"], "global");
    assert_eq!(ps[1]["shadowed"], true);
    for p in ps {
        assert!(p.get("body").is_none(), "요약에는 body 가 없어야 한다");
        assert!(p["tags"].is_array());
        assert!(p["description"].is_null());
        for k in [
            "id",
            "scope",
            "title",
            "created_at",
            "updated_at",
            "shadowed",
        ] {
            assert!(p.get(k).is_some(), "{k}");
        }
    }
}

#[test]
fn list_json_empty_still_valid_object_exit_0() {
    let sb = Sandbox::new();
    let v = sb.run(&["list", "--json"]).ok().json();
    assert_eq!(
        (v["count"].as_u64(), v["prompts"].as_array().map(Vec::len)),
        (Some(0), Some(0))
    );
}

#[test]
fn list_warns_about_skipped_files_on_stderr_and_still_exits_0() {
    let sb = Sandbox::new();
    sb.add("ok", None);
    sb.write_file(&sb.global_dir(), "bad.md", BAD);
    let o = sb.run(&["list"]).ok();
    assert_eq!(o.stdout_lines().len(), 1, "{o:?}");
    assert!(o.stdout.starts_with("[G] ok"));
    assert!(
        o.stderr.contains("경고: 읽지 못한 파일 [G] bad.md"),
        "{o:?}"
    );
}

#[test]
fn list_json_puts_skipped_into_warnings_and_stderr() {
    let sb = Sandbox::with_local();
    sb.add("ok", None);
    sb.write_file(&sb.local_dir(), "bad.md", BAD);
    let o = sb.run(&["list", "--json"]).ok();
    let v = o.json();
    assert_eq!(v["count"], 1);
    let w = &v["warnings"][0];
    assert_eq!(
        (w["kind"].as_str(), w["scope"].as_str(), w["name"].as_str()),
        (Some("skipped"), Some("local"), Some("bad.md"))
    );
    assert!(w["reason"].as_str().is_some_and(|r| !r.is_empty()));
    assert!(o.stderr.contains("bad.md"));
}

#[test]
fn list_closed_stdout_pipe_exits_0_silently() {
    let sb = Sandbox::new();
    for i in 0..50 {
        sb.add(&format!("p{i}"), None);
    }
    let mut c = sb.command(&sb.proj);
    c.arg("list")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    let mut child = c.spawn().unwrap();
    drop(child.stdout.take());
    let o = child.wait_with_output().unwrap();
    assert_eq!(o.status.code(), Some(0));
    assert!(
        o.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
}

// ---------- search ----------

#[test]
fn search_matches_id_title_body_tag_case_insensitively() {
    let sb = Sandbox::new();
    sb.run(&["add", "Alpha", "--body", "Rust 코드", "--tag", "Review"])
        .ok();
    sb.run(&["add", "베타", "--body", "other"]).ok();
    for (q, n) in [
        ("rust", 1),
        ("RUST", 1),
        ("alph", 1),
        ("review", 1),
        ("베", 1),
        ("other", 1),
        ("zzz", 0),
    ] {
        let o = sb.run(&["search", q]).ok();
        assert_eq!(o.stdout_lines().len(), n, "{q}: {o:?}");
    }
}

#[test]
fn search_no_result_is_exit_0_not_3() {
    let sb = Sandbox::new();
    let o = sb.run(&["search", "zzz"]).ok();
    assert!(o.stdout.is_empty() && o.stderr.is_empty(), "{o:?}");
}

#[test]
fn search_empty_query_is_usage_error_exit_2() {
    let sb = Sandbox::new();
    let o = sb.run(&["search", ""]).code(2);
    assert!(o.stdout.is_empty());
    sb.run(&["search", "   "]).code(2);
}

#[test]
fn search_tag_and_scope_filter_and_json() {
    let sb = Sandbox::with_local();
    sb.run(&["add", "a", "--body", "needle", "--tag", "x", "--global"])
        .ok();
    sb.run(&["add", "b", "--body", "needle", "--local"]).ok();
    assert_eq!(
        sb.run(&["search", "needle", "--tag", "x"])
            .ok()
            .stdout_lines()
            .len(),
        1
    );
    assert_eq!(
        sb.run(&["search", "needle", "--local"])
            .ok()
            .stdout_lines()
            .len(),
        1
    );
    let v = sb.run(&["search", "needle", "--json"]).ok().json();
    assert_eq!(
        (v["schema_version"].as_u64(), v["count"].as_u64()),
        (Some(1), Some(2))
    );
    assert!(v["prompts"][0].get("body").is_none());
    assert!(v["warnings"].is_array());
}

#[test]
fn search_local_without_local_is_exit_1() {
    let sb = Sandbox::new();
    sb.run(&["search", "x", "--local"]).code(1);
}

// ---------- 에러 출력 / 진입점 ----------

#[test]
fn json_error_is_single_line_on_stderr_and_stdout_is_empty() {
    let sb = Sandbox::new();
    let o = sb.run(&["get", "nope", "--json"]).code(3);
    assert!(o.stdout.is_empty(), "{o:?}");
    assert_eq!(o.stderr_lines().len(), 1, "{o:?}");
    let v = o.err_json();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["error"]["kind"], "not_found");
    assert!(v["error"]["message"].as_str().unwrap().contains("nope"));
}

#[test]
fn json_error_kinds_for_various_failures() {
    let sb = Sandbox::new();
    let o = sb.run(&["add", "a/b", "--body", "x", "--json"]).code(1);
    assert_eq!(o.err_json()["error"]["kind"], "invalid_id");
    let o = sb
        .run(&["add", "x", "--body", "x", "--local", "--json"])
        .code(1);
    assert_eq!(o.err_json()["error"]["kind"], "local_not_initialized");
    let o = sb.run(&["search", "", "--json"]).code(2);
    assert_eq!(o.err_json()["error"]["kind"], "usage");
    let o = sb.run(&["list", "--local", "--json"]).code(1);
    assert!(o.stdout.is_empty());
}

#[test]
fn text_error_is_single_line_prefixed_and_on_stderr() {
    let sb = Sandbox::new();
    let o = sb.run(&["get", "nope"]).code(3);
    assert_eq!(o.stderr_lines().len(), 1);
    assert!(o.stderr.starts_with("오류: "));
}

#[test]
fn no_arguments_reports_tui_not_implemented_exit_1() {
    let sb = Sandbox::new();
    let o = sb.run(&[]).code(1);
    assert!(o.stdout.is_empty(), "{o:?}");
    assert!(o.stderr.contains("TUI"), "{o:?}");
    let o = sb.run(&["tui"]).code(1);
    assert!(o.stdout.is_empty() && o.stderr.contains("TUI"), "{o:?}");
    assert!(!sb.ph_home.exists());
}

#[test]
fn clap_usage_errors_are_exit_2_with_empty_stdout() {
    let sb = Sandbox::new();
    for args in [
        &["get"][..],
        &["nonsense"],
        &["list", "--bogus"],
        &["move", "x", "--to", "mars"],
        &["rm"],
        &["get", "x", "--local", "--global"],
    ] {
        let o = sb.run(args).code(2);
        assert!(o.stdout.is_empty(), "{args:?}: {o:?}");
        assert!(!o.stderr.is_empty(), "{args:?}");
    }
}

#[test]
fn help_and_version_exit_0_on_stdout() {
    let sb = Sandbox::new();
    let h = sb.run(&["--help"]).ok();
    assert!(h.stdout.contains("ph") && h.stderr.is_empty());
    for sub in ["init", "add", "get", "list", "search", "edit", "rm", "move"] {
        assert!(h.stdout.contains(sub), "help 에 {sub} 가 없다");
        sb.run(&[sub, "--help"]).ok();
    }
    let v = sb.run(&["--version"]).ok();
    assert!(v.stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn home_flag_overrides_ph_home_env() {
    let sb = Sandbox::new();
    let other = sb.root.path().join("other_home");
    let o = sb
        .run(&["--home", other.to_str().unwrap(), "add", "x", "--body", "b"])
        .ok();
    assert_eq!(o.stdout, "x\n");
    assert!(other.join("prompts").join("x.md").is_file());
    assert!(!sb.ph_home.exists());
}

#[cfg(target_os = "linux")]
#[test]
fn without_ph_home_global_dir_follows_xdg_data_home() {
    let sb = Sandbox::new();
    let xdg = sb.root.path().join("xdg");
    let mut c = std::process::Command::new(env!("CARGO_BIN_EXE_ph"));
    c.env_clear()
        .env("HOME", &sb.home)
        .env("XDG_DATA_HOME", &xdg)
        .current_dir(&sb.proj);
    c.args(["add", "x", "--body", "b"]);
    let o = c.output().unwrap();
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(xdg.join("ph").join("prompts").join("x.md").is_file());
}

#[test]
fn hand_edited_bom_crlf_file_is_readable_via_cli() {
    let sb = Sandbox::new();
    let raw = "\u{feff}+++\r\ntitle = \"수동\"\r\ncreated_at = \"2026-09-30T12:00:00+09:00\"\r\nupdated_at = \"2026-09-30T12:00:00+09:00\"\r\n+++\r\n본문\r\n";
    sb.write_file(&sb.global_dir(), "수동.md", raw);
    assert_eq!(sb.run(&["get", "수동"]).ok().stdout, "본문\n");
    assert_eq!(sb.run(&["list"]).ok().stdout_lines().len(), 1);
}
