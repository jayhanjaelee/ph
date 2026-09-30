//! CLI 통합: `rm`, `move`, scope 우선순위.

mod common;

use common::cli::*;

// ---------- rm ----------

#[test]
fn rm_without_yes_in_non_interactive_is_exit_2_and_keeps_file() {
    let sb = Sandbox::new();
    sb.add("x", None);
    let o = sb.run(&["rm", "x"]).code(2);
    assert!(o.stdout.is_empty());
    assert!(o.stderr.contains("--yes"), "{o:?}");
    assert!(
        !o.stderr.contains("[y/N]"),
        "프롬프트를 띄우면 안 된다: {o:?}"
    );
    assert!(sb.global_dir().join("x.md").is_file());
}

#[test]
fn rm_non_interactive_ignores_piped_yes_answer() {
    let sb = Sandbox::new();
    sb.add("x", None);
    sb.run_stdin(&["rm", "x"], b"y\n").code(2);
    assert!(sb.global_dir().join("x.md").is_file());
}

#[test]
fn rm_yes_deletes_and_reports_on_stderr_with_empty_stdout() {
    let sb = Sandbox::new();
    sb.add("x", None);
    let o = sb.run(&["rm", "x", "--yes"]).ok();
    assert!(o.stdout.is_empty(), "{o:?}");
    assert!(o.stderr.contains("삭제됨: [G] x"), "{o:?}");
    assert!(!sb.global_dir().join("x.md").exists());
    sb.run(&["get", "x"]).code(3);
}

#[test]
fn rm_short_flag_y_works() {
    let sb = Sandbox::new();
    sb.add("x", None);
    sb.run(&["rm", "x", "-y"]).ok();
}

#[test]
fn rm_missing_id_is_exit_3_even_with_yes() {
    let sb = Sandbox::new();
    let o = sb.run(&["rm", "none", "--yes"]).code(3);
    assert!(o.stdout.is_empty());
    sb.run(&["rm", "none"]).code(3);
}

#[test]
fn rm_json_prints_removed_object() {
    let sb = Sandbox::with_local();
    sb.add("x", Some("--local"));
    let o = sb.run(&["rm", "x", "--yes", "--json"]).ok();
    let v = o.json();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["removed"]["id"], "x");
    assert_eq!(v["removed"]["scope"], "local");
}

#[test]
fn rm_ambiguous_removes_local_only_warns_and_notes_remaining_global() {
    let sb = Sandbox::with_local();
    sb.add("dup", Some("--global"));
    sb.add("dup", Some("--local"));
    let o = sb.run(&["rm", "dup", "--yes"]).ok();
    assert!(o.stderr.contains("경고"), "{o:?}");
    assert!(o.stderr.contains("삭제됨: [L] dup"), "{o:?}");
    assert!(
        o.stderr
            .contains("참고: global 에 같은 id 가 남아 있습니다"),
        "{o:?}"
    );
    assert!(!sb.local_dir().join("dup.md").exists());
    assert!(sb.global_dir().join("dup.md").is_file());
}

#[test]
fn rm_global_flag_removes_global_and_keeps_local() {
    let sb = Sandbox::with_local();
    sb.add("dup", Some("--global"));
    sb.add("dup", Some("--local"));
    let o = sb.run(&["rm", "dup", "--yes", "--global"]).ok();
    assert!(!o.stderr.contains("경고"), "{o:?}");
    assert!(sb.local_dir().join("dup.md").is_file());
    assert!(!sb.global_dir().join("dup.md").exists());
}

#[test]
fn rm_local_without_local_is_exit_1() {
    let sb = Sandbox::new();
    sb.add("x", None);
    sb.run(&["rm", "x", "--yes", "--local"]).code(1);
    assert!(sb.global_dir().join("x.md").is_file());
}

#[test]
fn rm_broken_file_is_invalid_format_exit_1_not_not_found() {
    let sb = Sandbox::new();
    sb.write_file(&sb.global_dir(), "bad.md", "garbage");
    let o = sb.run(&["rm", "bad", "--yes"]);
    assert_eq!(o.code, 1, "{o:?}");
    assert!(o.stderr.contains("파일 형식 오류"), "{o:?}");
    assert!(sb.global_dir().join("bad.md").is_file());
}

// ---------- move ----------

#[test]
fn move_global_to_local_and_back_keeps_body_and_timestamps() {
    let sb = Sandbox::with_local();
    sb.run(&["add", "m", "--body", "본문\n", "--tag", "t", "--global"])
        .ok();
    let before = sb.run(&["get", "m", "--json"]).ok().json()["prompt"].clone();

    let o = sb.run(&["move", "m", "--to", "local"]).ok();
    assert_eq!(o.stdout, "m\n");
    assert!(o.stderr.contains("이동됨: [G] m → [L]"), "{o:?}");
    assert!(sb.local_dir().join("m.md").is_file());
    assert!(!sb.global_dir().join("m.md").exists());
    let mid = sb.run(&["get", "m", "--json"]).ok().json()["prompt"].clone();
    assert_eq!(mid["scope"], "local");
    for k in ["body", "created_at", "updated_at", "title", "tags"] {
        assert_eq!(mid[k], before[k], "{k}");
    }

    let o = sb.run(&["move", "m", "--to", "global"]).ok();
    assert!(o.stderr.contains("이동됨: [L] m → [G]"), "{o:?}");
    assert!(sb.global_dir().join("m.md").is_file());
    assert!(!sb.local_dir().join("m.md").exists());
}

#[test]
fn move_json_schema_v1() {
    let sb = Sandbox::with_local();
    sb.add("m", Some("--global"));
    let v = sb
        .run(&["move", "m", "--to", "local", "--json"])
        .ok()
        .json();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(
        (v["from"].as_str(), v["to"].as_str()),
        (Some("global"), Some("local"))
    );
    assert_eq!(v["prompt"]["id"], "m");
    assert_eq!(v["prompt"]["scope"], "local");
}

#[test]
fn move_to_local_without_local_is_exit_1_and_keeps_source() {
    let sb = Sandbox::new();
    sb.add("m", None);
    let o = sb.run(&["move", "m", "--to", "local"]).code(1);
    assert!(o.stdout.is_empty());
    assert!(o.stderr.contains("ph init"), "{o:?}");
    assert!(sb.global_dir().join("m.md").is_file());
}

#[test]
fn move_conflict_is_already_exists_exit_1_and_changes_nothing() {
    let sb = Sandbox::with_local();
    sb.run(&["add", "m", "--body", "G", "--global"]).ok();
    sb.run(&["add", "m", "--body", "L", "--local"]).ok();
    let o = sb.run(&["move", "m", "--to", "local"]).code(1);
    assert!(o.stdout.is_empty());
    assert!(sb.global_dir().join("m.md").is_file());
    assert_eq!(sb.run(&["get", "m", "--local"]).ok().stdout, "L");
    let o = sb.run(&["move", "m", "--to", "local", "--json"]).code(1);
    assert_eq!(o.err_json()["error"]["kind"], "already_exists");
}

#[test]
fn move_missing_id_is_exit_3() {
    let sb = Sandbox::with_local();
    sb.run(&["move", "none", "--to", "local"]).code(3);
    sb.run(&["move", "none", "--to", "global"]).code(3);
}

#[test]
fn move_to_same_scope_where_it_already_is_fails_without_change() {
    let sb = Sandbox::with_local();
    sb.add("m", Some("--local"));
    let o = sb.run(&["move", "m", "--to", "local"]);
    assert_ne!(o.code, 0, "{o:?}");
    assert!(sb.local_dir().join("m.md").is_file());
}

// ---------- scope 우선순위 종합 ----------

#[test]
fn scope_rules_end_to_end() {
    let sb = Sandbox::new();
    // local 이 없을 때: 기본은 global, --local 은 에러
    sb.run(&["add", "a", "--body", "1"]).ok();
    assert!(sb.global_dir().join("a.md").is_file());
    sb.run(&["add", "b", "--body", "1", "--local"]).code(1);
    // init 후: 기본은 local
    sb.run(&["init"]).ok();
    sb.run(&["add", "c", "--body", "1"]).ok();
    assert!(sb.local_dir().join("c.md").is_file());
    // 다른 디렉터리(프로젝트 밖)에서는 local 이 보이지 않는다.
    let outside = sb.home.join("elsewhere");
    std::fs::create_dir(&outside).unwrap();
    let o = sb.run_in(&outside, &["list"]).ok();
    assert_eq!(o.stdout_lines().len(), 1, "{o:?}");
    assert!(o.stdout.starts_with("[G] a"));
    sb.run_in(&outside, &["add", "d", "--body", "1"]).ok();
    assert!(sb.global_dir().join("d.md").is_file());
    // 프로젝트 안에서는 병합
    assert_eq!(sb.run(&["list"]).ok().stdout_lines().len(), 3);
}

#[test]
fn home_dot_ph_is_not_treated_as_local() {
    let sb = Sandbox::new();
    std::fs::create_dir_all(sb.home.join(".ph").join("prompts")).unwrap();
    let o = sb.run(&["add", "x", "--body", "b", "--local"]).code(1);
    assert!(o.stderr.contains("ph init"), "{o:?}");
    sb.run(&["add", "x", "--body", "b"]).ok();
    assert!(sb.global_dir().join("x.md").is_file());
}
