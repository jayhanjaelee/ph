//! CLI 통합: `edit` (외부 에디터), 대화형 `rm` 확인. 실제 바이너리를 pty 에서 실행하고,
//! 로직은 가짜 `EditorLauncher` 로 핸들러를 직접 호출해 검증한다.

mod common;

use common::cli::*;

// ---------- 비대화형 ----------

#[test]
fn edit_in_non_tty_is_exit_2_and_never_launches_editor() {
    let sb = Sandbox::new();
    sb.add("x", None);
    let marker = sb.root.path().join("launched");
    // EDITOR 는 존재하지 않는 경로여도 비대화형 검사가 먼저여야 한다.
    let o = sb
        .run_env(&["edit", "x"], &[("EDITOR", marker.to_str().unwrap())])
        .code(2);
    assert!(o.stdout.is_empty());
    assert!(o.stderr.contains("터미널"), "{o:?}");
    assert!(!marker.exists());
}

#[test]
fn edit_non_tty_error_has_priority_over_not_found() {
    let sb = Sandbox::new();
    sb.run(&["edit", "none"]).code(2);
}

// ---------- pty 위에서 실제 바이너리 ----------

#[cfg(unix)]
mod pty_e2e {
    use super::*;
    use common::pty::{run_tty, script};

    /// 본문의 `X` 를 `Y` 로 바꾸는 에디터.
    const SED_FIX: &str = r#"sed 's/원본 본문/수정된 본문/' "$1" > "$1.new" && mv "$1.new" "$1""#;

    fn setup() -> (Sandbox, std::path::PathBuf) {
        let sb = Sandbox::new();
        sb.run(&["add", "원본", "--body", "원본 본문"]).ok();
        let bin = sb.root.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        (sb, bin)
    }

    #[test]
    fn edit_success_saves_body_and_reports_on_stderr() {
        let (sb, bin) = setup();
        let ed = script(&bin, "ed.sh", SED_FIX);
        let Some(o) = run_tty(
            &sb,
            &["edit", "원본"],
            "",
            &[("EDITOR", ed.to_str().unwrap())],
        ) else {
            return;
        };
        assert_eq!(o.code, 0, "{o:?}");
        assert!(o.stdout.is_empty(), "{o:?}");
        assert!(o.stderr.contains("저장됨: [G] 원본"), "{o:?}");
        assert_eq!(sb.run(&["get", "원본"]).ok().stdout, "수정된 본문");
    }

    #[test]
    fn edit_visual_takes_precedence_over_editor() {
        let (sb, bin) = setup();
        let good = script(&bin, "good.sh", SED_FIX);
        let bad = script(&bin, "bad.sh", "exit 9");
        let Some(o) = run_tty(
            &sb,
            &["edit", "원본"],
            "",
            &[
                ("VISUAL", good.to_str().unwrap()),
                ("EDITOR", bad.to_str().unwrap()),
            ],
        ) else {
            return;
        };
        assert_eq!(o.code, 0, "{o:?}");
    }

    #[test]
    fn edit_no_change_reports_and_does_not_touch_file() {
        let (sb, bin) = setup();
        let ed = script(&bin, "noop.sh", "exit 0");
        let path = sb.global_dir().join("원본.md");
        let before = std::fs::read(&path).unwrap();
        let Some(o) = run_tty(
            &sb,
            &["edit", "원본"],
            "",
            &[("EDITOR", ed.to_str().unwrap())],
        ) else {
            return;
        };
        assert_eq!(o.code, 0, "{o:?}");
        assert!(o.stderr.contains("변경 없음"), "{o:?}");
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn edit_editor_nonzero_exit_is_exit_1_and_original_untouched() {
        let (sb, bin) = setup();
        let ed = script(&bin, "fail.sh", r#"printf 'zzz' > "$1"; exit 3"#);
        let path = sb.global_dir().join("원본.md");
        let before = std::fs::read(&path).unwrap();
        let Some(o) = run_tty(
            &sb,
            &["edit", "원본"],
            "",
            &[("EDITOR", ed.to_str().unwrap())],
        ) else {
            return;
        };
        assert_eq!(o.code, 1, "{o:?}");
        assert!(o.stdout.is_empty());
        assert!(
            o.stderr.contains("비정상 종료") && o.stderr.contains('3'),
            "{o:?}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn edit_missing_editor_binary_is_exit_1_with_hint() {
        let (sb, _bin) = setup();
        let missing = sb.root.path().join("no-such-editor");
        let Some(o) = run_tty(
            &sb,
            &["edit", "원본"],
            "",
            &[("EDITOR", missing.to_str().unwrap())],
        ) else {
            return;
        };
        assert_eq!(o.code, 1, "{o:?}");
        assert!(o.stderr.contains("에디터를 실행하지 못했습니다"), "{o:?}");
        assert!(o.stderr.contains("EDITOR"), "{o:?}");
    }

    #[test]
    fn edit_broken_frontmatter_then_no_retry_is_exit_1_and_original_untouched() {
        let (sb, bin) = setup();
        let ed = script(&bin, "broken.sh", r#"printf 'garbage' > "$1""#);
        let path = sb.global_dir().join("원본.md");
        let before = std::fs::read(&path).unwrap();
        let Some(o) = run_tty(
            &sb,
            &["edit", "원본"],
            "n\n",
            &[("EDITOR", ed.to_str().unwrap())],
        ) else {
            return;
        };
        assert_eq!(o.code, 1, "{o:?}");
        assert!(
            o.stderr.contains("원본 prompt 는 변경되지 않았습니다"),
            "{o:?}"
        );
        assert!(o.stderr.contains("다시 편집할까요"), "{o:?}");
        assert!(o.stderr.contains("1번째 줄"), "줄 번호 안내: {o:?}");
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn edit_broken_then_retry_default_yes_reopens_same_temp_content_and_saves() {
        let (sb, bin) = setup();
        let counter = sb.root.path().join("count");
        // 1회차: 파일을 깨뜨린다. 2회차: 깨진 내용이 그대로 남아 있는지 확인한 뒤 복구.
        let body = format!(
            r#"if [ -f "{c}" ]; then
  grep -q garbage "$1" || exit 7
  printf '+++\ntitle = "원본"\ncreated_at = "2026-09-30T12:00:00+09:00"\nupdated_at = "2026-09-30T12:00:00+09:00"\n+++\n고친 본문' > "$1"
else
  touch "{c}"; printf 'garbage' > "$1"
fi"#,
            c = counter.display()
        );
        let ed = script(&bin, "twice.sh", &body);
        let Some(o) = run_tty(
            &sb,
            &["edit", "원본"],
            "\n",
            &[("EDITOR", ed.to_str().unwrap())],
        ) else {
            return;
        };
        assert_eq!(o.code, 0, "{o:?}");
        assert!(o.stderr.contains("저장됨"), "{o:?}");
        assert_eq!(sb.run(&["get", "원본"]).ok().stdout, "고친 본문");
    }

    #[test]
    fn edit_id_key_in_frontmatter_is_ignored_with_notice() {
        let (sb, bin) = setup();
        let ed = script(
            &bin,
            "idkey.sh",
            r#"awk 'NR==1{print; print "id = \"hacked\""; next} {print}' "$1" | sed 's/원본 본문/바뀜/' > "$1.new" && mv "$1.new" "$1""#,
        );
        let Some(o) = run_tty(
            &sb,
            &["edit", "원본"],
            "",
            &[("EDITOR", ed.to_str().unwrap())],
        ) else {
            return;
        };
        assert_eq!(o.code, 0, "{o:?}");
        assert!(o.stderr.contains("id 는 변경되지 않습니다"), "{o:?}");
        assert!(sb.global_dir().join("원본.md").is_file());
        assert!(!sb.global_dir().join("hacked.md").exists());
        // awk 가 마지막 줄에 개행을 붙인다.
        assert_eq!(sb.run(&["get", "원본"]).ok().stdout, "바뀜\n");
    }

    #[test]
    fn edit_missing_id_is_exit_3_before_launching_editor() {
        let (sb, bin) = setup();
        let marker = sb.root.path().join("launched");
        let ed = script(&bin, "mark.sh", &format!("touch {}", marker.display()));
        let Some(o) = run_tty(
            &sb,
            &["edit", "none"],
            "",
            &[("EDITOR", ed.to_str().unwrap())],
        ) else {
            return;
        };
        assert_eq!(o.code, 3, "{o:?}");
        assert!(!marker.exists());
    }

    #[test]
    fn edit_editor_with_args_in_env_is_split_on_whitespace() {
        let (sb, bin) = setup();
        // 인자를 받는 에디터: 첫 인자는 플래그, 마지막 인자가 파일
        let ed = script(
            &bin,
            "argful.sh",
            r#"[ "$1" = "--wait" ] || exit 5; f="$2"; sed 's/원본 본문/인자 OK/' "$f" > "$f.new" && mv "$f.new" "$f""#,
        );
        let cmd = format!("{} --wait", ed.display());
        let Some(o) = run_tty(&sb, &["edit", "원본"], "", &[("EDITOR", &cmd)]) else {
            return;
        };
        assert_eq!(o.code, 0, "{o:?}");
        assert_eq!(sb.run(&["get", "원본"]).ok().stdout, "인자 OK");
    }

    #[test]
    fn edit_ambiguous_edits_local_and_warns_and_leaves_global() {
        let sb = Sandbox::with_local();
        sb.run(&["add", "dup", "--body", "원본 본문", "--global"])
            .ok();
        sb.run(&["add", "dup", "--body", "원본 본문", "--local"])
            .ok();
        let bin = sb.root.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let ed = script(&bin, "ed.sh", SED_FIX);
        let Some(o) = run_tty(
            &sb,
            &["edit", "dup"],
            "",
            &[("EDITOR", ed.to_str().unwrap())],
        ) else {
            return;
        };
        assert_eq!(o.code, 0, "{o:?}");
        assert!(
            o.stderr.contains("경고") && o.stderr.contains("저장됨: [L] dup"),
            "{o:?}"
        );
        assert_eq!(
            sb.run(&["get", "dup", "--local"]).ok().stdout,
            "수정된 본문"
        );
        assert_eq!(sb.run(&["get", "dup", "--global"]).ok().stdout, "원본 본문");
    }

    #[test]
    fn edit_leaves_no_temp_files_behind() {
        let (sb, bin) = setup();
        let ed = script(&bin, "ed.sh", SED_FIX);
        let tmp = sb.root.path().join("tmp");
        std::fs::create_dir(&tmp).unwrap();
        let Some(o) = run_tty(
            &sb,
            &["edit", "원본"],
            "",
            &[
                ("EDITOR", ed.to_str().unwrap()),
                ("TMPDIR", tmp.to_str().unwrap()),
            ],
        ) else {
            return;
        };
        assert_eq!(o.code, 0, "{o:?}");
        assert_eq!(
            std::fs::read_dir(&tmp).unwrap().count(),
            0,
            "임시 파일/디렉터리가 남았다"
        );
    }

    // ---------- 대화형 rm ----------

    #[test]
    fn rm_interactive_answer_n_cancels_with_exit_0() {
        let (sb, _) = setup();
        let Some(o) = run_tty(&sb, &["rm", "원본"], "n\n", &[]) else {
            return;
        };
        assert_eq!(o.code, 0, "{o:?}");
        assert!(
            o.stderr.contains("[y/N]") && o.stderr.contains("취소했습니다"),
            "{o:?}"
        );
        assert!(sb.global_dir().join("원본.md").is_file());
    }

    #[test]
    fn rm_interactive_empty_answer_defaults_to_cancel() {
        let (sb, _) = setup();
        let Some(o) = run_tty(&sb, &["rm", "원본"], "\n", &[]) else {
            return;
        };
        assert_eq!(o.code, 0, "{o:?}");
        assert!(sb.global_dir().join("원본.md").is_file());
    }

    #[test]
    fn rm_interactive_answer_y_or_yes_any_case_deletes() {
        for ans in ["y\n", "YES\n", "Yes\n"] {
            let (sb, _) = setup();
            let Some(o) = run_tty(&sb, &["rm", "원본"], ans, &[]) else {
                return;
            };
            assert_eq!(o.code, 0, "{ans:?}: {o:?}");
            assert!(o.stderr.contains("삭제됨: [G] 원본"), "{o:?}");
            assert!(!sb.global_dir().join("원본.md").exists(), "{ans:?}");
        }
    }

    #[test]
    fn rm_interactive_other_answers_cancel() {
        let (sb, _) = setup();
        let Some(o) = run_tty(&sb, &["rm", "원본"], "yep\n", &[]) else {
            return;
        };
        assert_eq!(o.code, 0, "{o:?}");
        assert!(sb.global_dir().join("원본.md").is_file());
    }
}

// ---------- 핸들러 직접 호출 (가짜 EditorLauncher) ----------

mod handler {
    use std::cell::{Cell, RefCell};
    use std::ffi::OsString;
    use std::io;
    use std::path::{Path, PathBuf};

    use clap::Parser;
    use ph::cli::{run, Cli, CliIo, EditorLauncher};
    use ph::core::clock::FixedClock;
    use ph::core::error::PhError;
    use ph::core::model::Scope;
    use ph::core::service::{PromptService, ScopeFilter, WriteTarget};
    use ph::core::storage::Storage;
    use ph::platform::editor::{EditorCommand, EditorExit};
    use ph::storage::FsStorage;

    use crate::common::{new_prompt, t0};

    type Step = Box<dyn Fn(&Path) -> io::Result<EditorExit>>;

    /// 호출마다 미리 정한 동작을 순서대로 실행하는 가짜 에디터.
    struct Fake {
        steps: RefCell<Vec<Step>>,
        calls: Cell<usize>,
        seen_program: RefCell<Option<OsString>>,
    }

    impl Fake {
        fn new(steps: Vec<Step>) -> Self {
            Fake {
                steps: RefCell::new(steps),
                calls: Cell::new(0),
                seen_program: RefCell::new(None),
            }
        }
    }

    impl EditorLauncher for Fake {
        fn run(&self, cmd: &EditorCommand, file: &Path) -> io::Result<EditorExit> {
            self.calls.set(self.calls.get() + 1);
            *self.seen_program.borrow_mut() = Some(cmd.program.clone());
            let mut steps = self.steps.borrow_mut();
            assert!(!steps.is_empty(), "예상보다 에디터를 더 호출했다");
            (steps.remove(0))(file)
        }
    }

    fn edit_with(text: &'static str) -> Step {
        Box::new(move |p| {
            let cur = std::fs::read_to_string(p).unwrap();
            std::fs::write(p, cur.replace("원본 본문", text)).unwrap();
            Ok(EditorExit::Success)
        })
    }
    fn write_raw(text: &'static str) -> Step {
        Box::new(move |p| {
            std::fs::write(p, text).unwrap();
            Ok(EditorExit::Success)
        })
    }

    struct Env {
        _d: tempfile::TempDir,
        g: PathBuf,
        l: PathBuf,
        cwd: PathBuf,
    }

    impl Env {
        fn new() -> Self {
            let d = tempfile::tempdir().unwrap();
            let g = d.path().join("g");
            let l = d.path().join("l");
            let cwd = d.path().to_path_buf();
            Env { _d: d, g, l, cwd }
        }
        /// updated_at 이 t0 와 구분되도록 편집 서비스의 시계는 나중 시각이다.
        fn svc(&self, later: bool) -> PromptService {
            let now = if later {
                chrono::DateTime::parse_from_rfc3339("2030-01-01T00:00:00+09:00").unwrap()
            } else {
                t0()
            };
            PromptService::new(
                Box::new(FsStorage::new(Scope::Global, self.g.clone())),
                Some(Box::new(FsStorage::new(Scope::Local, self.l.clone())) as Box<dyn Storage>),
                Box::new(FixedClock(now)),
            )
        }
        fn seed(&self, title: &str, scope: Scope) {
            self.svc(false)
                .add(
                    ph::core::model::NewPrompt {
                        body: "원본 본문".into(),
                        ..new_prompt(title)
                    },
                    WriteTarget::Explicit(scope),
                )
                .unwrap();
        }
        fn body(&self, id: &str, scope: Scope) -> String {
            self.svc(false)
                .get(id, ScopeFilter::Only(scope))
                .unwrap()
                .prompt
                .body
        }
    }

    struct Res {
        result: Result<(), PhError>,
        stdout: String,
        stderr: String,
    }

    fn run_edit(
        env: &Env,
        args: &[&str],
        fake: &Fake,
        interactive: bool,
        stdin: &str,
        editor_env: Option<&str>,
    ) -> Res {
        let cli = Cli::try_parse_from(std::iter::once("ph").chain(args.iter().copied())).unwrap();
        let mut stdin = io::Cursor::new(stdin.as_bytes().to_vec());
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let ed = editor_env.map(OsString::from);
        let getenv = move |k: &str| if k == "EDITOR" { ed.clone() } else { None };
        let mut cio = CliIo {
            stdin: &mut stdin,
            stdout: &mut out,
            stderr: &mut err,
            interactive,
            cwd: &env.cwd,
            home_override: None,
            env: &getenv,
            editor: fake,
        };
        let result = run(cli.command.unwrap(), &mut cio, &|| Ok(env.svc(true)));
        Res {
            result,
            stdout: String::from_utf8(out).unwrap(),
            stderr: String::from_utf8(err).unwrap(),
        }
    }

    #[test]
    fn success_updates_body_and_updated_at_but_not_created_at() {
        let env = Env::new();
        env.seed("문서", Scope::Global);
        let fake = Fake::new(vec![edit_with("새 본문")]);
        let r = run_edit(&env, &["edit", "문서"], &fake, true, "", Some("myeditor"));
        r.result.unwrap();
        assert!(r.stdout.is_empty());
        assert!(r.stderr.contains("저장됨: [G] 문서"), "{}", r.stderr);
        assert_eq!(env.body("문서", Scope::Global), "새 본문");
        let p = env.svc(false).get("문서", ScopeFilter::All).unwrap().prompt;
        assert_eq!(p.created_at, t0());
        assert!(p.updated_at > t0());
        assert_eq!(
            fake.seen_program.borrow().as_deref(),
            Some(std::ffi::OsStr::new("myeditor"))
        );
    }

    #[test]
    fn unchanged_content_does_not_rewrite_or_bump_updated_at() {
        let env = Env::new();
        env.seed("문서", Scope::Global);
        let fake = Fake::new(vec![Box::new(|_| Ok(EditorExit::Success))]);
        let r = run_edit(&env, &["edit", "문서"], &fake, true, "", None);
        r.result.unwrap();
        assert!(r.stderr.contains("변경 없음"));
        assert_eq!(
            env.svc(false)
                .get("문서", ScopeFilter::All)
                .unwrap()
                .prompt
                .updated_at,
            t0()
        );
    }

    #[test]
    fn defaults_to_vi_when_no_env_editor() {
        let env = Env::new();
        env.seed("문서", Scope::Global);
        let fake = Fake::new(vec![Box::new(|_| Ok(EditorExit::Success))]);
        run_edit(&env, &["edit", "문서"], &fake, true, "", None)
            .result
            .unwrap();
        assert_eq!(
            fake.seen_program.borrow().as_deref(),
            Some(std::ffi::OsStr::new("vi"))
        );
    }

    #[test]
    fn non_interactive_is_non_interactive_error_and_editor_not_called() {
        let env = Env::new();
        env.seed("문서", Scope::Global);
        let fake = Fake::new(vec![]);
        let r = run_edit(&env, &["edit", "문서"], &fake, false, "", None);
        assert!(matches!(r.result, Err(PhError::NonInteractive(_))));
        assert_eq!(fake.calls.get(), 0);
    }

    #[test]
    fn editor_failed_exit_is_editor_error_and_nothing_saved() {
        let env = Env::new();
        env.seed("문서", Scope::Global);
        for exit in [EditorExit::Failed(Some(1)), EditorExit::Failed(None)] {
            let fake = Fake::new(vec![Box::new(move |p| {
                std::fs::write(p, "zzz").unwrap();
                Ok(exit)
            })]);
            let r = run_edit(&env, &["edit", "문서"], &fake, true, "", None);
            assert!(
                matches!(r.result, Err(PhError::Editor(ref m)) if m.contains("비정상 종료")),
                "{:?}",
                r.result
            );
            assert_eq!(env.body("문서", Scope::Global), "원본 본문");
        }
    }

    #[test]
    fn editor_launch_failure_is_editor_error_mentioning_program() {
        let env = Env::new();
        env.seed("문서", Scope::Global);
        let fake = Fake::new(vec![Box::new(|_| {
            Err(io::Error::from(io::ErrorKind::NotFound))
        })]);
        let r = run_edit(
            &env,
            &["edit", "문서"],
            &fake,
            true,
            "",
            Some("nonexistent-ed"),
        );
        match r.result {
            Err(PhError::Editor(m)) => {
                assert!(m.contains("nonexistent-ed") && m.contains("EDITOR"), "{m}")
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn invalid_frontmatter_answer_n_returns_invalid_format_and_keeps_original() {
        let env = Env::new();
        env.seed("문서", Scope::Global);
        let fake = Fake::new(vec![write_raw("garbage")]);
        let r = run_edit(&env, &["edit", "문서"], &fake, true, "n\n", None);
        assert!(
            matches!(r.result, Err(PhError::InvalidFormat { .. })),
            "{:?}",
            r.result
        );
        assert!(r.stderr.contains("원본 prompt 는 변경되지 않았습니다"));
        assert_eq!(fake.calls.get(), 1);
        assert_eq!(env.body("문서", Scope::Global), "원본 본문");
    }

    #[test]
    fn invalid_frontmatter_answer_no_uppercase_also_stops() {
        let env = Env::new();
        env.seed("문서", Scope::Global);
        let fake = Fake::new(vec![write_raw("garbage")]);
        let r = run_edit(&env, &["edit", "문서"], &fake, true, "NO\n", None);
        assert!(r.result.is_err());
        assert_eq!(fake.calls.get(), 1);
    }

    #[test]
    fn retry_keeps_broken_text_in_same_file_then_fix_saves() {
        let env = Env::new();
        env.seed("문서", Scope::Global);
        let fake = Fake::new(vec![
            write_raw("garbage"),
            Box::new(|p| {
                assert_eq!(
                    std::fs::read_to_string(p).unwrap(),
                    "garbage",
                    "재편집은 깨진 내용 그대로 열려야 한다"
                );
                std::fs::write(
                    p,
                    "+++\ntitle = \"문서\"\ncreated_at = \"2026-09-30T12:00:00+09:00\"\nupdated_at = \"2026-09-30T12:00:00+09:00\"\n+++\n고침",
                )
                .unwrap();
                Ok(EditorExit::Success)
            }),
        ]);
        // 빈 줄(기본 Y) 로 재편집
        run_edit(&env, &["edit", "문서"], &fake, true, "\n", None)
            .result
            .unwrap();
        assert_eq!(fake.calls.get(), 2);
        assert_eq!(env.body("문서", Scope::Global), "고침");
    }

    #[test]
    fn retry_prompt_at_eof_defaults_to_retry_and_repeated_failures_keep_asking() {
        let env = Env::new();
        env.seed("문서", Scope::Global);
        // 입력이 y 두 번 뒤 n: 편집기는 3번 호출되고 마지막에 실패 반환
        let fake = Fake::new(vec![write_raw("g1"), write_raw("g2"), write_raw("g3")]);
        let r = run_edit(&env, &["edit", "문서"], &fake, true, "y\n\nn\n", None);
        assert!(r.result.is_err());
        assert_eq!(fake.calls.get(), 3);
        assert_eq!(env.body("문서", Scope::Global), "원본 본문");
    }

    #[test]
    fn empty_title_after_edit_is_validation_failure_with_retry_prompt() {
        let env = Env::new();
        env.seed("문서", Scope::Global);
        let fake = Fake::new(vec![write_raw(
            "+++\ntitle = \"  \"\ncreated_at = \"2026-09-30T12:00:00+09:00\"\nupdated_at = \"2026-09-30T12:00:00+09:00\"\n+++\nx",
        )]);
        let r = run_edit(&env, &["edit", "문서"], &fake, true, "n\n", None);
        assert!(matches!(r.result, Err(PhError::Usage(_))), "{:?}", r.result);
        assert!(r.stderr.contains("다시 편집할까요"));
    }

    #[test]
    fn non_utf8_after_edit_is_validation_failure_not_panic() {
        let env = Env::new();
        env.seed("문서", Scope::Global);
        let fake = Fake::new(vec![Box::new(|p| {
            std::fs::write(p, [0xff, 0xfe, 0x00]).unwrap();
            Ok(EditorExit::Success)
        })]);
        let r = run_edit(&env, &["edit", "문서"], &fake, true, "n\n", None);
        assert!(
            matches!(r.result, Err(PhError::InvalidFormat { .. })),
            "{:?}",
            r.result
        );
        assert!(r.stderr.contains("UTF-8"));
    }

    #[test]
    fn crlf_and_bom_from_editor_are_accepted() {
        let env = Env::new();
        env.seed("문서", Scope::Global);
        let fake = Fake::new(vec![Box::new(|p| {
            let cur = std::fs::read_to_string(p)
                .unwrap()
                .replace("원본 본문", "윈도우 본문");
            std::fs::write(p, format!("\u{feff}{}", cur.replace('\n', "\r\n"))).unwrap();
            Ok(EditorExit::Success)
        })]);
        run_edit(&env, &["edit", "문서"], &fake, true, "", None)
            .result
            .unwrap();
        assert_eq!(env.body("문서", Scope::Global), "윈도우 본문");
    }

    #[test]
    fn id_key_and_title_changes_keep_id_and_print_notice() {
        let env = Env::new();
        env.seed("문서", Scope::Global);
        let fake = Fake::new(vec![Box::new(|p| {
            let cur = std::fs::read_to_string(p).unwrap();
            let cur = cur
                .replacen("+++\n", "+++\nid = \"다른이름\"\n", 1)
                .replace("title = \"문서\"", "title = \"새 제목\"");
            std::fs::write(p, cur).unwrap();
            Ok(EditorExit::Success)
        })]);
        let r = run_edit(&env, &["edit", "문서"], &fake, true, "", None);
        r.result.unwrap();
        assert!(
            r.stderr.contains("참고: id 는 변경되지 않습니다"),
            "{}",
            r.stderr
        );
        let p = env.svc(false).get("문서", ScopeFilter::All).unwrap().prompt;
        assert_eq!((p.id.as_str(), p.title.as_str()), ("문서", "새 제목"));
        assert!(!env.g.join("다른이름.md").exists());
    }

    #[test]
    fn created_at_tampering_is_ignored() {
        let env = Env::new();
        env.seed("문서", Scope::Global);
        let fake = Fake::new(vec![Box::new(|p| {
            let cur = std::fs::read_to_string(p).unwrap();
            std::fs::write(
                p,
                cur.replace("2026-09-30T12:00:00+09:00", "1999-01-01T00:00:00+09:00")
                    .replace("원본 본문", "x"),
            )
            .unwrap();
            Ok(EditorExit::Success)
        })]);
        run_edit(&env, &["edit", "문서"], &fake, true, "", None)
            .result
            .unwrap();
        assert_eq!(
            env.svc(false)
                .get("문서", ScopeFilter::All)
                .unwrap()
                .prompt
                .created_at,
            t0()
        );
    }

    #[test]
    fn ambiguous_edit_targets_local_only_and_warns() {
        let env = Env::new();
        env.seed("dup", Scope::Global);
        env.seed("dup", Scope::Local);
        let fake = Fake::new(vec![edit_with("로컬 수정")]);
        let r = run_edit(&env, &["edit", "dup"], &fake, true, "", None);
        r.result.unwrap();
        assert!(
            r.stderr.contains("경고") && r.stderr.contains("저장됨: [L] dup"),
            "{}",
            r.stderr
        );
        assert_eq!(env.body("dup", Scope::Local), "로컬 수정");
        assert_eq!(env.body("dup", Scope::Global), "원본 본문");
    }

    #[test]
    fn global_flag_edits_global_even_when_local_has_same_id() {
        let env = Env::new();
        env.seed("dup", Scope::Global);
        env.seed("dup", Scope::Local);
        let fake = Fake::new(vec![edit_with("전역 수정")]);
        let r = run_edit(&env, &["edit", "dup", "--global"], &fake, true, "", None);
        r.result.unwrap();
        assert!(!r.stderr.contains("경고"));
        assert_eq!(env.body("dup", Scope::Global), "전역 수정");
        assert_eq!(env.body("dup", Scope::Local), "원본 본문");
    }

    #[test]
    fn editor_receives_frontmatter_file_named_after_id() {
        let env = Env::new();
        env.seed("문서", Scope::Global);
        let fake = Fake::new(vec![Box::new(|p| {
            let text = std::fs::read_to_string(p).unwrap();
            assert!(text.starts_with("+++\n") && text.contains("title = \"문서\""));
            assert!(p.file_name().unwrap().to_string_lossy().contains("문서"));
            Ok(EditorExit::Success)
        })]);
        run_edit(&env, &["edit", "문서"], &fake, true, "", None)
            .result
            .unwrap();
    }

    #[test]
    fn temp_file_is_removed_after_edit_finishes_or_fails() {
        let env = Env::new();
        env.seed("문서", Scope::Global);
        let seen: std::rc::Rc<RefCell<Option<PathBuf>>> = Default::default();
        let s2 = seen.clone();
        let fake = Fake::new(vec![Box::new(move |p| {
            *s2.borrow_mut() = Some(p.to_path_buf());
            Ok(EditorExit::Failed(Some(2)))
        })]);
        let _ = run_edit(&env, &["edit", "문서"], &fake, true, "", None);
        let p = seen.borrow().clone().unwrap();
        assert!(!p.exists(), "임시 파일이 남았다: {}", p.display());
    }

    #[test]
    fn missing_id_is_not_found_before_editor() {
        let env = Env::new();
        let fake = Fake::new(vec![]);
        let r = run_edit(&env, &["edit", "nope"], &fake, true, "", None);
        assert!(matches!(r.result, Err(PhError::NotFound { .. })));
        assert_eq!(fake.calls.get(), 0);
    }
}
