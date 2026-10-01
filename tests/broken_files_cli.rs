//! 깨진 파일 규칙 R1~R6 회귀 테스트: 실제 프로세스 통합 + CLI 핸들러 단위 (SPEC 3.3절).

mod common;

use std::path::{Path, PathBuf};

use common::cli::*;

const ID: &str = "x";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum St {
    M,
    O,
    B,
}
use St::{B, M, O};

fn broken_local() -> String {
    "+++\ntitle = \n+++\nLOCAL-BROKEN".to_string()
}
fn broken_global() -> String {
    "GLOBAL-BROKEN no frontmatter".to_string()
}

/// (local, global) 상태를 만든다. 정상 본문은 `LOCAL` / `GLOBAL`.
fn setup(l: St, g: St) -> Sandbox {
    let sb = Sandbox::with_local();
    match l {
        M => {}
        O => {
            sb.run(&["add", ID, "--body", "LOCAL", "--local"]).ok();
        }
        B => sb.write_file(&sb.local_dir(), "x.md", &broken_local()),
    }
    match g {
        M => {}
        O => {
            sb.run(&["add", ID, "--body", "GLOBAL", "--global"]).ok();
        }
        B => sb.write_file(&sb.global_dir(), "x.md", &broken_global()),
    }
    sb
}

fn bytes(p: &Path) -> Vec<u8> {
    std::fs::read(p).unwrap()
}
fn gfile(sb: &Sandbox) -> PathBuf {
    sb.global_dir().join("x.md")
}
fn lfile(sb: &Sandbox) -> PathBuf {
    sb.local_dir().join("x.md")
}

fn assert_invalid_format_failure(o: &Out, badge: &str) {
    assert_eq!(o.code, 1, "{o:?}");
    assert!(o.stdout.is_empty(), "에러 시 stdout 은 비어야 한다: {o:?}");
    assert!(o.stderr.starts_with("오류:"), "{o:?}");
    assert!(o.stderr.contains("파일 형식 오류"), "{o:?}");
    assert!(o.stderr.contains(badge), "{o:?}");
    assert!(o.stderr.contains("직접 고치거나 지우세요"), "{o:?}");
    assert!(
        !o.stderr.contains("해당하는 prompt 가 없습니다"),
        "NotFound 문구가 나오면 안 된다: {o:?}"
    );
}

fn assert_json_error(o: &Out, kind: &str, code: i32) {
    assert_eq!(o.code, code, "{o:?}");
    assert!(o.stdout.is_empty(), "{o:?}");
    assert_eq!(o.stderr_lines().len(), 1, "{o:?}");
    let v = o.err_json();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["error"]["kind"], kind, "{o:?}");
}

// ---------- 1. R1 ----------

#[test]
fn r1_only_broken_get_rm_move_fail_with_invalid_format_exit_1() {
    for (l, g, badge) in [(B, M, "[L]"), (M, B, "[G]")] {
        let sb = setup(l, g);
        let (lb, gb) = (
            lfile(&sb).exists().then(|| bytes(&lfile(&sb))),
            gfile(&sb).exists().then(|| bytes(&gfile(&sb))),
        );
        assert_invalid_format_failure(&sb.run(&["get", ID]), badge);
        assert_invalid_format_failure(&sb.run(&["rm", ID, "--yes"]), badge);
        assert_json_error(&sb.run(&["get", ID, "--json"]), "invalid_format", 1);
        assert_json_error(&sb.run(&["rm", ID, "--yes", "--json"]), "invalid_format", 1);
        // 파일은 그대로
        assert_eq!(lfile(&sb).exists().then(|| bytes(&lfile(&sb))), lb);
        assert_eq!(gfile(&sb).exists().then(|| bytes(&gfile(&sb))), gb);
    }
}

#[test]
fn r1_move_with_only_broken_source_is_invalid_format() {
    let sb = setup(B, M);
    assert_invalid_format_failure(&sb.run(&["move", ID, "--to", "global"]), "[L]");
    assert_eq!(bytes(&lfile(&sb)), broken_local().as_bytes());
    assert!(!gfile(&sb).exists());
    let sb = setup(M, B);
    assert_invalid_format_failure(&sb.run(&["move", ID, "--to", "local"]), "[G]");
    assert!(!lfile(&sb).exists());
}

// ---------- 2. R2 ----------

#[test]
fn r2_good_local_broken_global_get_is_local_exit_0_without_any_warning() {
    let sb = setup(O, B);
    let o = sb.run(&["get", ID]).ok();
    assert_eq!(o.stdout, "LOCAL");
    assert!(o.stderr.is_empty(), "R2 는 경고도 내지 않는다: {o:?}");
    let o = sb.run(&["get", ID, "--json"]).ok();
    assert!(o.stderr.is_empty(), "{o:?}");
    let v = o.json();
    assert_eq!(
        (v["prompt"]["scope"].as_str(), v["ambiguous"].as_bool()),
        (Some("local"), Some(false))
    );
}

#[test]
fn r2_write_ops_use_local_and_leave_broken_global_untouched() {
    let sb = setup(O, B);
    let o = sb.run(&["rm", ID, "--yes"]).ok();
    assert!(o.stderr.contains("삭제됨: [L] x"), "{o:?}");
    assert!(!o.stderr.contains("경고"), "{o:?}");
    assert_eq!(bytes(&gfile(&sb)), broken_global().as_bytes());
}

// ---------- 3. R3 get ----------

#[test]
fn r3_get_returns_global_body_with_warning_on_stderr_only_exit_0() {
    let sb = setup(B, O);
    let o = sb.run(&["get", ID]).ok();
    assert_eq!(o.stdout, "GLOBAL", "stdout 에는 본문만: {o:?}");
    let w = o.stderr_lines();
    assert_eq!(w.len(), 1, "경고는 한 줄: {o:?}");
    assert!(w[0].starts_with("경고"), "{o:?}");
    assert!(
        w[0].contains("local") && w[0].contains("global 을 사용합니다") && w[0].contains("깨져"),
        "{o:?}"
    );
    assert!(w[0].contains(ID));
}

#[test]
fn r3_get_json_stdout_schema_is_unchanged_and_warning_only_on_stderr() {
    let sb = setup(B, O);
    let o = sb.run(&["get", ID, "--json"]).ok();
    assert_eq!(o.stdout_lines().len(), 1);
    let v = o.json();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["ambiguous"], false);
    assert_eq!(v["prompt"]["scope"], "global");
    assert_eq!(v["prompt"]["body"], "GLOBAL");
    let mut keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(
        keys,
        ["ambiguous", "prompt", "schema_version"],
        "스키마에 필드를 추가하면 안 된다"
    );
    assert!(
        !o.stdout.contains("깨져") && !o.stdout.contains("경고"),
        "경고가 stdout 에 섞였다"
    );
    assert!(
        o.stderr.contains("경고") && o.stderr.contains("깨져"),
        "{o:?}"
    );
    // --global 로 직접 읽은 결과와 stdout JSON 이 완전히 같다.
    let direct = sb.run(&["get", ID, "--global", "--json"]).ok();
    assert!(direct.stderr.is_empty(), "{direct:?}");
    assert_eq!(v, direct.json());
}

// ---------- 4. R3 write ----------

#[test]
fn r3_rm_without_scope_is_invalid_format_and_global_file_is_unchanged() {
    let sb = setup(B, O);
    let g_before = bytes(&gfile(&sb));
    let l_before = bytes(&lfile(&sb));
    let o = sb.run(&["rm", ID, "--yes"]);
    assert_invalid_format_failure(&o, "[L]");
    assert_eq!(bytes(&gfile(&sb)), g_before);
    assert_eq!(bytes(&lfile(&sb)), l_before);
    assert_json_error(&sb.run(&["rm", ID, "--yes", "--json"]), "invalid_format", 1);
    assert_eq!(bytes(&gfile(&sb)), g_before);
}

#[test]
fn r3_rm_with_global_flag_succeeds_and_leaves_broken_local() {
    let sb = setup(B, O);
    let l_before = bytes(&lfile(&sb));
    let o = sb.run(&["rm", ID, "--yes", "--global"]).ok();
    assert!(o.stderr.contains("삭제됨: [G] x"), "{o:?}");
    assert!(!gfile(&sb).exists());
    assert_eq!(bytes(&lfile(&sb)), l_before);
}

#[test]
fn r3_non_interactive_rm_without_yes_prefers_which_error() {
    // 대상 해석이 먼저라 깨진 local 이 먼저 보고된다 (비대화형 검사보다 앞).
    let sb = setup(B, O);
    let o = sb.run(&["rm", ID]);
    assert_eq!(o.code, 1, "{o:?}");
    assert!(o.stderr.contains("파일 형식 오류"), "{o:?}");
    assert!(gfile(&sb).is_file());
}

#[test]
fn r3_move_with_broken_destination_or_source_is_invalid_format_and_changes_nothing() {
    // 원본 global 정상, 대상 local 깨짐
    let sb = setup(B, O);
    let (lb, gb) = (bytes(&lfile(&sb)), bytes(&gfile(&sb)));
    assert_invalid_format_failure(&sb.run(&["move", ID, "--to", "local"]), "[L]");
    assert_eq!(
        (bytes(&lfile(&sb)), bytes(&gfile(&sb))),
        (lb.clone(), gb.clone())
    );
    // 원본 local 깨짐, 대상 global 정상
    assert_invalid_format_failure(&sb.run(&["move", ID, "--to", "global"]), "[L]");
    assert_eq!((bytes(&lfile(&sb)), bytes(&gfile(&sb))), (lb, gb));
    // 원본 local 정상, 대상 global 깨짐
    let sb = setup(O, B);
    let (lb, gb) = (bytes(&lfile(&sb)), bytes(&gfile(&sb)));
    assert_invalid_format_failure(&sb.run(&["move", ID, "--to", "global"]), "[G]");
    assert_eq!((bytes(&lfile(&sb)), bytes(&gfile(&sb))), (lb, gb));
    let o = sb.run(&["move", ID, "--to", "global", "--json"]);
    assert_json_error(&o, "invalid_format", 1);
}

#[test]
fn move_destination_with_good_same_id_is_already_exists_not_invalid_format() {
    let sb = setup(O, O);
    let (lb, gb) = (bytes(&lfile(&sb)), bytes(&gfile(&sb)));
    let o = sb.run(&["move", ID, "--to", "global", "--json"]);
    assert_json_error(&o, "already_exists", 1);
    assert_eq!((bytes(&lfile(&sb)), bytes(&gfile(&sb))), (lb, gb));
}

// ---------- 5. R5 / 6. R6 ----------

#[test]
fn r5_both_good_still_returns_local_with_ambiguous_warning() {
    let sb = setup(O, O);
    let o = sb.run(&["get", ID]).ok();
    assert_eq!(o.stdout, "LOCAL");
    assert!(o.stderr.contains("양쪽"), "{o:?}");
    assert!(!o.stderr.contains("깨져"), "{o:?}");
    let v = sb.run(&["get", ID, "--json"]).ok().json();
    assert_eq!(v["ambiguous"], true);
}

#[test]
fn r6_both_broken_is_invalid_format_for_get_and_rm_and_names_local() {
    let sb = setup(B, B);
    let (lb, gb) = (bytes(&lfile(&sb)), bytes(&gfile(&sb)));
    let o = sb.run(&["get", ID]);
    assert_invalid_format_failure(&o, "[L]");
    assert!(!o.stderr.contains("[G]"), "{o:?}");
    assert_invalid_format_failure(&sb.run(&["rm", ID, "--yes"]), "[L]");
    assert_json_error(&sb.run(&["get", ID, "--json"]), "invalid_format", 1);
    assert_eq!((bytes(&lfile(&sb)), bytes(&gfile(&sb))), (lb, gb));
}

// ---------- 7. scope 한정 ----------

#[test]
fn scope_flag_get_reports_that_scopes_state_without_fallback() {
    let sb = setup(B, O);
    assert_invalid_format_failure(&sb.run(&["get", ID, "--local"]), "[L]");
    let o = sb.run(&["get", ID, "--global"]).ok();
    assert_eq!((o.stdout.as_str(), o.stderr.as_str()), ("GLOBAL", ""));

    let sb = setup(O, B);
    assert_invalid_format_failure(&sb.run(&["get", ID, "--global"]), "[G]");
    let o = sb.run(&["get", ID, "--local"]).ok();
    assert_eq!((o.stdout.as_str(), o.stderr.as_str()), ("LOCAL", ""));

    let sb = setup(M, B);
    assert_invalid_format_failure(&sb.run(&["get", ID, "--global"]), "[G]");
    assert_eq!(sb.run(&["get", ID, "--local"]).code, 3);
}

// ---------- 8. list / search ----------

#[test]
fn list_and_search_keep_skipped_warnings_for_all_combinations_exit_0() {
    for l in [M, O, B] {
        for g in [M, O, B] {
            let sb = setup(l, g);
            let ctx = format!("L={l:?} G={g:?}");
            let o = sb.run(&["list"]).ok();
            assert_eq!(
                o.stdout_lines().len(),
                [l, g].iter().filter(|s| **s == O).count(),
                "{ctx}: {o:?}"
            );
            assert_eq!(
                o.stderr.contains("경고: 읽지 못한 파일 [L] x.md"),
                l == B,
                "{ctx}: {o:?}"
            );
            assert_eq!(
                o.stderr.contains("경고: 읽지 못한 파일 [G] x.md"),
                g == B,
                "{ctx}: {o:?}"
            );
            let o = sb.run(&["list", "--json"]).ok();
            let v = o.json();
            assert_eq!(
                v["count"].as_u64().unwrap() as usize,
                [l, g].iter().filter(|s| **s == O).count(),
                "{ctx}"
            );
            let n_warn = [l, g].iter().filter(|s| **s == B).count();
            assert_eq!(v["warnings"].as_array().unwrap().len(), n_warn, "{ctx}");
            for w in v["warnings"].as_array().unwrap() {
                assert_eq!(w["kind"], "skipped");
                assert_eq!(w["name"], "x.md");
            }
            // search 는 정상 항목을 계속 찾고 종료 0
            let o = sb.run(&["search", "LOCAL"]).ok();
            assert_eq!(o.stdout_lines().len(), usize::from(l == O), "{ctx}");
            let o = sb.run(&["search", "GLOBAL", "--json"]).ok();
            assert_eq!(
                o.json()["count"].as_u64().unwrap() as usize,
                usize::from(g == O),
                "{ctx}"
            );
        }
    }
}

#[test]
fn list_with_broken_local_and_good_global_shows_global_not_shadowed() {
    let sb = setup(B, O);
    let o = sb.run(&["list"]).ok();
    assert_eq!(o.stdout_lines().len(), 1);
    assert!(
        o.stdout.starts_with("[G] x") && !o.stdout.contains("(shadowed)"),
        "{o:?}"
    );
    assert!(o.stderr.contains("[L] x.md"), "{o:?}");
}

#[test]
fn list_scope_flags_warn_only_for_selected_scope() {
    let sb = setup(B, B);
    let o = sb.run(&["list", "--local"]).ok();
    assert!(
        o.stderr.contains("[L] x.md") && !o.stderr.contains("[G] x.md"),
        "{o:?}"
    );
    let o = sb.run(&["list", "--global"]).ok();
    assert!(
        o.stderr.contains("[G] x.md") && !o.stderr.contains("[L] x.md"),
        "{o:?}"
    );
}

#[test]
fn list_json_stdout_has_no_warning_text_outside_warnings_array() {
    let sb = setup(B, O);
    let o = sb.run(&["list", "--json"]).ok();
    assert_eq!(o.stdout_lines().len(), 1);
    let v = o.json();
    let keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
    for k in ["schema_version", "count", "prompts", "warnings"] {
        assert!(keys.contains(&k.to_string()), "{k}");
    }
    assert_eq!(keys.len(), 4);
}

// ---------- 10. add ----------

#[test]
fn add_never_overwrites_broken_file_and_uses_suffix_and_stdout_is_new_id() {
    let sb = setup(B, B);
    let (lb, gb) = (bytes(&lfile(&sb)), bytes(&gfile(&sb)));
    let o = sb.run(&["add", "x", "--body", "NEW", "--local"]).ok();
    assert_eq!(o.stdout, "x-2\n");
    let o = sb.run(&["add", "X", "--body", "NEW", "--global"]).ok();
    assert_eq!(o.stdout, "X-2\n");
    assert_eq!((bytes(&lfile(&sb)), bytes(&gfile(&sb))), (lb, gb));
    assert!(sb.local_dir().join("x-2.md").is_file());
    assert!(sb.global_dir().join("X-2.md").is_file());
}

#[test]
fn add_default_target_with_broken_local_does_not_overwrite() {
    let sb = setup(B, M);
    let lb = bytes(&lfile(&sb));
    let o = sb.run(&["add", "x", "--body", "NEW"]).ok();
    assert_eq!(o.stdout, "x-2\n");
    assert_eq!(bytes(&lfile(&sb)), lb);
}

// ---------- edit (pty) ----------

#[cfg(unix)]
mod edit_pty {
    use super::*;
    use common::pty::{run_tty, script};

    const FIX: &str = r#"sed 's/GLOBAL/EDITED/' "$1" > "$1.new" && mv "$1.new" "$1""#;

    fn editor(sb: &Sandbox, body: &str, marker: bool) -> (PathBuf, PathBuf) {
        let bin = sb.root.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let mark = sb.root.path().join("launched");
        let b = if marker {
            format!("touch {}\n{body}", mark.display())
        } else {
            body.to_string()
        };
        (script(&bin, "ed.sh", &b), mark)
    }

    #[test]
    fn edit_r1_only_broken_is_invalid_format_without_launching_editor() {
        for (l, g, badge) in [(B, M, "[L]"), (M, B, "[G]")] {
            let sb = setup(l, g);
            let (ed, mark) = editor(&sb, FIX, true);
            let Some(o) = run_tty(&sb, &["edit", ID], "", &[("EDITOR", ed.to_str().unwrap())])
            else {
                return;
            };
            assert_invalid_format_failure(&o, badge);
            assert!(!mark.exists(), "에디터가 실행되었다");
        }
    }

    #[test]
    fn edit_r3_without_scope_is_invalid_format_editor_not_launched_global_unchanged() {
        let sb = setup(B, O);
        let (ed, mark) = editor(&sb, FIX, true);
        let (gb, lb) = (bytes(&gfile(&sb)), bytes(&lfile(&sb)));
        let Some(o) = run_tty(&sb, &["edit", ID], "", &[("EDITOR", ed.to_str().unwrap())]) else {
            return;
        };
        assert_invalid_format_failure(&o, "[L]");
        assert!(!mark.exists());
        assert_eq!((bytes(&gfile(&sb)), bytes(&lfile(&sb))), (gb, lb));
    }

    #[test]
    fn edit_r3_with_global_flag_edits_global_and_leaves_broken_local() {
        let sb = setup(B, O);
        let (ed, _) = editor(&sb, FIX, false);
        let lb = bytes(&lfile(&sb));
        let Some(o) = run_tty(
            &sb,
            &["edit", ID, "--global"],
            "",
            &[("EDITOR", ed.to_str().unwrap())],
        ) else {
            return;
        };
        assert_eq!(o.code, 0, "{o:?}");
        assert!(o.stderr.contains("저장됨: [G] x"), "{o:?}");
        assert_eq!(sb.run(&["get", ID, "--global"]).ok().stdout, "EDITED");
        assert_eq!(bytes(&lfile(&sb)), lb);
    }

    #[test]
    fn edit_r2_edits_good_local_and_ignores_broken_global() {
        let sb = setup(O, B);
        let (ed, _) = editor(
            &sb,
            r#"sed 's/LOCAL/L-EDITED/' "$1" > "$1.new" && mv "$1.new" "$1""#,
            false,
        );
        let gb = bytes(&gfile(&sb));
        let Some(o) = run_tty(&sb, &["edit", ID], "", &[("EDITOR", ed.to_str().unwrap())]) else {
            return;
        };
        assert_eq!(o.code, 0, "{o:?}");
        assert!(!o.stderr.contains("경고"), "{o:?}");
        assert_eq!(sb.run(&["get", ID, "--local"]).ok().stdout, "L-EDITED");
        assert_eq!(bytes(&gfile(&sb)), gb);
    }

    #[test]
    fn edit_r6_both_broken_is_invalid_format() {
        let sb = setup(B, B);
        let (ed, mark) = editor(&sb, FIX, true);
        let Some(o) = run_tty(&sb, &["edit", ID], "", &[("EDITOR", ed.to_str().unwrap())]) else {
            return;
        };
        assert_invalid_format_failure(&o, "[L]");
        assert!(!mark.exists());
    }

    #[test]
    fn rm_interactive_r3_fails_before_prompting() {
        let sb = setup(B, O);
        let gb = bytes(&gfile(&sb));
        let Some(o) = run_tty(&sb, &["rm", ID], "y\n", &[]) else {
            return;
        };
        assert_invalid_format_failure(&o, "[L]");
        assert!(
            !o.stderr.contains("[y/N]"),
            "프롬프트를 띄우기 전에 실패해야 한다: {o:?}"
        );
        assert_eq!(bytes(&gfile(&sb)), gb);
    }
}

// ---------- CLI 핸들러 단위 (in-process, fs 저장소) ----------

mod handler {
    use std::cell::Cell;
    use std::io;
    use std::path::Path;

    use clap::Parser;
    use ph::cli::{run, Cli, CliIo, EditorLauncher};
    use ph::core::clock::FixedClock;
    use ph::core::error::PhError;
    use ph::core::model::Scope;
    use ph::core::service::PromptService;
    use ph::core::storage::Storage;
    use ph::platform::editor::{EditorCommand, EditorExit};
    use ph::storage::FsStorage;

    use super::{St, B, M, O};
    use crate::common::t0;

    struct CountingEditor(Cell<usize>);
    impl EditorLauncher for CountingEditor {
        fn run(&self, _c: &EditorCommand, file: &Path) -> io::Result<EditorExit> {
            self.0.set(self.0.get() + 1);
            let t = std::fs::read_to_string(file)?;
            std::fs::write(file, t.replace("GLOBAL", "EDITED"))?;
            Ok(EditorExit::Success)
        }
    }

    struct Env {
        _d: tempfile::TempDir,
        g: std::path::PathBuf,
        l: std::path::PathBuf,
    }

    fn svc(g: &Path, l: &Path) -> PromptService {
        PromptService::new(
            Box::new(FsStorage::new(Scope::Global, g.to_path_buf())),
            Some(Box::new(FsStorage::new(Scope::Local, l.to_path_buf())) as Box<dyn Storage>),
            Box::new(FixedClock(t0())),
        )
    }

    fn setup(l: St, g: St) -> Env {
        let d = tempfile::tempdir().unwrap();
        let (gd, ld) = (d.path().join("g"), d.path().join("l"));
        let s = svc(&gd, &ld);
        for (st, scope, dir, body) in [
            (l, Scope::Local, &ld, "LOCAL"),
            (g, Scope::Global, &gd, "GLOBAL"),
        ] {
            match st {
                M => {}
                O => {
                    s.add(
                        ph::core::model::NewPrompt {
                            title: "x".into(),
                            body: body.into(),
                            ..Default::default()
                        },
                        ph::core::service::WriteTarget::Explicit(scope),
                    )
                    .unwrap();
                }
                B => {
                    std::fs::create_dir_all(dir).unwrap();
                    std::fs::write(dir.join("x.md"), "garbage").unwrap();
                }
            }
        }
        Env {
            _d: d,
            g: gd,
            l: ld,
        }
    }

    struct Res {
        result: Result<(), PhError>,
        stdout: String,
        stderr: String,
        editor_calls: usize,
    }

    fn call(env: &Env, args: &[&str], interactive: bool, stdin: &str) -> Res {
        let cli = Cli::try_parse_from(std::iter::once("ph").chain(args.iter().copied())).unwrap();
        let mut input = io::Cursor::new(stdin.as_bytes().to_vec());
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let ed = CountingEditor(Cell::new(0));
        let getenv = |_: &str| None;
        let cwd = env._d.path().to_path_buf();
        let mut cio = CliIo {
            stdin: &mut input,
            stdout: &mut out,
            stderr: &mut err,
            interactive,
            cwd: &cwd,
            home_override: None,
            env: &getenv,
            editor: &ed,
        };
        let result = run(cli.command.unwrap(), &mut cio, &|| Ok(svc(&env.g, &env.l)));
        Res {
            result,
            stdout: String::from_utf8(out).unwrap(),
            stderr: String::from_utf8(err).unwrap(),
            editor_calls: ed.0.get(),
        }
    }

    fn is_invalid(r: &Result<(), PhError>) -> bool {
        matches!(r, Err(PhError::InvalidFormat { .. }))
    }

    #[test]
    fn get_r3_prints_global_body_and_fallback_warning_only_on_stderr() {
        let env = setup(B, O);
        let r = call(&env, &["get", "x"], false, "");
        r.result.unwrap();
        assert_eq!(r.stdout, "GLOBAL");
        assert!(
            r.stderr.contains("경고")
                && r.stderr.contains("깨져")
                && r.stderr.contains("global 을 사용합니다"),
            "{}",
            r.stderr
        );
        assert!(!r.stderr.contains("양쪽"));
    }

    #[test]
    fn get_r3_json_keeps_schema_and_ambiguous_false() {
        let env = setup(B, O);
        let r = call(&env, &["get", "x", "--json"], false, "");
        r.result.unwrap();
        let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
        assert_eq!(v["ambiguous"], false);
        assert_eq!(v["prompt"]["scope"], "global");
        assert!(r.stderr.contains("깨져"));
        assert!(!r.stdout.contains("깨져"));
    }

    #[test]
    fn get_r2_has_no_warning_and_r1_r6_are_errors_with_empty_stdout() {
        let r = call(&setup(O, B), &["get", "x"], false, "");
        r.result.unwrap();
        assert_eq!((r.stdout.as_str(), r.stderr.as_str()), ("LOCAL", ""));
        for (l, g) in [(B, M), (M, B), (B, B)] {
            let r = call(&setup(l, g), &["get", "x"], false, "");
            assert!(is_invalid(&r.result), "{l:?}/{g:?}: {:?}", r.result);
            assert!(r.stdout.is_empty());
        }
    }

    #[test]
    fn rm_r3_is_invalid_format_before_any_prompt_and_files_unchanged() {
        let env = setup(B, O);
        let before = std::fs::read(env.g.join("x.md")).unwrap();
        for (interactive, stdin) in [(false, ""), (true, "y\n")] {
            let r = call(&env, &["rm", "x"], interactive, stdin);
            assert!(is_invalid(&r.result), "{:?}", r.result);
            assert!(!r.stderr.contains("[y/N]"), "{}", r.stderr);
        }
        let r = call(&env, &["rm", "x", "--yes"], false, "");
        assert!(is_invalid(&r.result));
        assert_eq!(std::fs::read(env.g.join("x.md")).unwrap(), before);
        assert_eq!(std::fs::read(env.l.join("x.md")).unwrap(), b"garbage");
    }

    #[test]
    fn rm_r3_global_flag_deletes_global_only() {
        let env = setup(B, O);
        let r = call(&env, &["rm", "x", "--yes", "--global"], false, "");
        r.result.unwrap();
        assert!(!env.g.join("x.md").exists());
        assert!(env.l.join("x.md").exists());
    }

    #[test]
    fn rm_r2_deletes_local_only() {
        let env = setup(O, B);
        call(&env, &["rm", "x", "--yes"], false, "").result.unwrap();
        assert!(!env.l.join("x.md").exists());
        assert!(env.g.join("x.md").exists());
    }

    #[test]
    fn edit_r3_is_invalid_format_without_launching_editor_and_global_unchanged() {
        let env = setup(B, O);
        let before = std::fs::read(env.g.join("x.md")).unwrap();
        let r = call(&env, &["edit", "x"], true, "");
        assert!(is_invalid(&r.result), "{:?}", r.result);
        assert_eq!(r.editor_calls, 0);
        assert_eq!(std::fs::read(env.g.join("x.md")).unwrap(), before);
    }

    #[test]
    fn edit_r3_global_flag_edits_global_via_editor() {
        let env = setup(B, O);
        let r = call(&env, &["edit", "x", "--global"], true, "");
        r.result.unwrap();
        assert_eq!(r.editor_calls, 1);
        assert!(r.stderr.contains("저장됨: [G] x"));
        assert!(std::fs::read_to_string(env.g.join("x.md"))
            .unwrap()
            .contains("EDITED"));
        assert_eq!(std::fs::read(env.l.join("x.md")).unwrap(), b"garbage");
    }

    #[test]
    fn edit_r1_and_r6_are_invalid_format_without_editor() {
        for (l, g) in [(B, M), (M, B), (B, B)] {
            let r = call(&setup(l, g), &["edit", "x"], true, "");
            assert!(is_invalid(&r.result), "{l:?}/{g:?}: {:?}", r.result);
            assert_eq!(r.editor_calls, 0);
        }
    }

    #[test]
    fn list_and_search_print_skipped_warnings_and_succeed() {
        let env = setup(B, O);
        let r = call(&env, &["list"], false, "");
        r.result.unwrap();
        assert!(r.stdout.starts_with("[G] x"));
        assert!(
            r.stderr.contains("경고: 읽지 못한 파일 [L] x.md"),
            "{}",
            r.stderr
        );
        let r = call(&env, &["search", "GLOBAL"], false, "");
        r.result.unwrap();
        assert_eq!(r.stdout.lines().count(), 1);
    }

    #[test]
    fn move_with_broken_either_side_is_invalid_format_and_files_unchanged() {
        for (l, g, to) in [
            (B, O, "local"),
            (B, O, "global"),
            (O, B, "global"),
            (O, B, "local"),
        ] {
            let env = setup(l, g);
            let snap = |p: &Path| std::fs::read(p).ok();
            let before = (snap(&env.l.join("x.md")), snap(&env.g.join("x.md")));
            let r = call(&env, &["move", "x", "--to", to], false, "");
            // (O,B)->local: 원본 global 이 깨짐, (B,O)->global: 원본 local 이 깨짐 => 모두 InvalidFormat
            assert!(is_invalid(&r.result), "{l:?}/{g:?} to {to}: {:?}", r.result);
            assert!(r.stdout.is_empty());
            assert_eq!(
                before,
                (snap(&env.l.join("x.md")), snap(&env.g.join("x.md")))
            );
        }
    }
}
