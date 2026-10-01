//! 깨진 파일 규칙 R1~R6 회귀 테스트: service 단위 (SPEC 3.3절, ARCHITECTURE 3.4절/10.8절).
//! `FsStorage` 를 임시 디렉터리에 두고 (local, global) 상태 9가지 조합을 모두 검증한다.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use common::*;
use ph::core::error::PhError;
use ph::core::model::{NewPrompt, PromptPatch, Scope};
use ph::core::service::{PromptService, ScopeFilter, WriteTarget};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum St {
    /// 파일 없음
    M,
    /// 정상
    O,
    /// 깨짐
    B,
}
use St::{B, M, O};

const ALL: [St; 3] = [M, O, B];
const ID: &str = "x";
const RAW_OK: &str = "+++\ntitle = \"x\"\ncreated_at = \"2026-09-30T12:00:00+09:00\"\nupdated_at = \"2026-09-30T12:00:00+09:00\"\n+++\nNEW";

struct Fx {
    _d: tempfile::TempDir,
    g: PathBuf,
    l: PathBuf,
    svc: PromptService,
}

fn broken_text(scope: Scope) -> String {
    format!("+++\ntitle = \n+++\n깨짐-{}", scope.as_str())
}

fn setup(l: St, g: St) -> Fx {
    let (d, gd, ld, svc) = fs_service(true);
    for (st, scope, dir) in [(l, Scope::Local, &ld), (g, Scope::Global, &gd)] {
        match st {
            M => {}
            O => {
                svc.add(
                    NewPrompt {
                        title: ID.into(),
                        body: scope.as_str().to_uppercase(),
                        ..Default::default()
                    },
                    WriteTarget::Explicit(scope),
                )
                .unwrap();
            }
            B => {
                std::fs::create_dir_all(dir).unwrap();
                std::fs::write(dir.join("x.md"), broken_text(scope)).unwrap();
            }
        }
    }
    Fx {
        _d: d,
        g: gd,
        l: ld,
        svc,
    }
}

fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut m = BTreeMap::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd {
            let e = e.unwrap();
            m.insert(
                e.file_name().to_string_lossy().into_owned(),
                std::fs::read(e.path()).unwrap(),
            );
        }
    }
    m
}

fn snap_both(f: &Fx) -> (BTreeMap<String, Vec<u8>>, BTreeMap<String, Vec<u8>>) {
    (snapshot(&f.l), snapshot(&f.g))
}

fn assert_invalid(e: &PhError, badge: &str, ctx: &str) {
    match e {
        PhError::InvalidFormat { .. } => {
            let m = e.to_string();
            assert!(m.contains(badge), "{ctx}: 메시지에 {badge} 가 없다: {m}");
            assert!(
                m.contains("직접 고치거나 지우세요"),
                "{ctx}: 안내가 없다: {m}"
            );
        }
        other => panic!("{ctx}: InvalidFormat 이어야 한다: {other:?}"),
    }
}

/// 기대값: 읽기(`get`) 결과.
#[derive(Debug, PartialEq)]
enum Got {
    NotFound,
    /// 깨진 scope 의 배지
    Invalid(&'static str),
    Found {
        scope: Scope,
        ambiguous: bool,
        fallback: Option<Scope>,
    },
}

fn exp_get(l: St, g: St) -> Got {
    use Scope::{Global as G, Local as L};
    match (l, g) {
        (M, M) => Got::NotFound,
        (M, O) => Got::Found {
            scope: G,
            ambiguous: false,
            fallback: None,
        },
        (M, B) => Got::Invalid("[G]"),
        (O, M) => Got::Found {
            scope: L,
            ambiguous: false,
            fallback: None,
        },
        (O, O) => Got::Found {
            scope: L,
            ambiguous: true,
            fallback: None,
        },
        (O, B) => Got::Found {
            scope: L,
            ambiguous: false,
            fallback: None,
        }, // R2
        (B, M) => Got::Invalid("[L]"),
        (B, O) => Got::Found {
            scope: G,
            ambiguous: false,
            fallback: Some(L),
        }, // R3
        (B, B) => Got::Invalid("[L]"), // R6
    }
}

/// 기대값: 쓰기 대상(`target`). R3 만 `get` 과 다르다.
fn exp_target(l: St, g: St) -> Got {
    match (l, g) {
        (B, O) => Got::Invalid("[L]"),
        (l, g) => exp_get(l, g),
    }
}

fn check(r: Result<ph::core::service::Resolved, PhError>, want: Got, ctx: &str, is_target: bool) {
    match (r, want) {
        (Err(PhError::NotFound { .. }), Got::NotFound) => {}
        (Err(e), Got::Invalid(b)) => assert_invalid(&e, b, ctx),
        (
            Ok(r),
            Got::Found {
                scope,
                ambiguous,
                fallback,
            },
        ) => {
            assert_eq!(r.prompt.scope, scope, "{ctx}");
            assert_eq!(r.prompt.body, scope.as_str().to_uppercase(), "{ctx}");
            assert_eq!(r.ambiguous, ambiguous, "{ctx}");
            let got_fb = r.fallback.as_ref().map(|f| f.broken_scope);
            if is_target {
                assert!(
                    r.fallback.is_none(),
                    "{ctx}: target 은 fallback 이 없어야 한다"
                );
            } else {
                assert_eq!(got_fb, fallback, "{ctx}");
                if let Some(f) = r.fallback {
                    assert!(
                        f.reason.contains("파일 형식 오류") || !f.reason.is_empty(),
                        "{ctx}"
                    );
                }
            }
        }
        (got, want) => panic!("{ctx}: 기대 {want:?}, 실제 {got:?}"),
    }
}

// ---------- R1~R6: get / target 전체 조합 ----------

#[test]
fn get_matches_rules_for_all_nine_state_combinations() {
    for l in ALL {
        for g in ALL {
            let f = setup(l, g);
            let before = snap_both(&f);
            check(
                f.svc.get(ID, ScopeFilter::All),
                exp_get(l, g),
                &format!("get L={l:?} G={g:?}"),
                false,
            );
            assert_eq!(
                snap_both(&f),
                before,
                "읽기가 파일을 바꿨다 L={l:?} G={g:?}"
            );
        }
    }
}

#[test]
fn target_matches_rules_for_all_nine_state_combinations() {
    for l in ALL {
        for g in ALL {
            let f = setup(l, g);
            check(
                f.svc.target(ID, ScopeFilter::All),
                exp_target(l, g),
                &format!("target L={l:?} G={g:?}"),
                true,
            );
        }
    }
}

#[test]
fn r1_only_broken_never_reports_not_found() {
    for (l, g) in [(B, M), (M, B)] {
        let f = setup(l, g);
        let badge = if l == B { "[L]" } else { "[G]" };
        assert_invalid(&f.svc.get(ID, ScopeFilter::All).unwrap_err(), badge, "get");
        assert_invalid(
            &f.svc.target(ID, ScopeFilter::All).unwrap_err(),
            badge,
            "target",
        );
        assert_invalid(
            &f.svc.export_raw(ID, ScopeFilter::All).unwrap_err(),
            badge,
            "export_raw",
        );
        assert_invalid(
            &f.svc.remove(ID, ScopeFilter::All).unwrap_err(),
            badge,
            "remove",
        );
        let upd = f.svc.update(
            ID,
            PromptPatch {
                body: Some("n".into()),
                ..Default::default()
            },
            ScopeFilter::All,
        );
        assert_invalid(&upd.unwrap_err(), badge, "update");
        assert_invalid(
            &f.svc.save_raw(ID, RAW_OK, ScopeFilter::All).unwrap_err(),
            badge,
            "save_raw",
        );
    }
}

#[test]
fn r2_good_local_with_broken_global_is_silent_local() {
    let f = setup(O, B);
    let r = f.svc.get(ID, ScopeFilter::All).unwrap();
    assert_eq!(r.prompt.scope, Scope::Local);
    assert!(r.fallback.is_none() && !r.ambiguous);
}

#[test]
fn r3_get_falls_back_to_global_and_reports_which_scope_was_broken() {
    let f = setup(B, O);
    let r = f.svc.get(ID, ScopeFilter::All).unwrap();
    assert_eq!(r.prompt.scope, Scope::Global);
    let fb = r.fallback.expect("fallback");
    assert_eq!(fb.broken_scope, Scope::Local);
    assert!(!fb.reason.is_empty());
    assert!(!r.ambiguous);
}

#[test]
fn r5_both_good_is_ambiguous_local() {
    let f = setup(O, O);
    let r = f.svc.get(ID, ScopeFilter::All).unwrap();
    assert_eq!(
        (r.prompt.scope, r.ambiguous, r.fallback.is_none()),
        (Scope::Local, true, true)
    );
}

#[test]
fn r6_both_broken_reports_local_file_first() {
    let f = setup(B, B);
    let e = f.svc.get(ID, ScopeFilter::All).unwrap_err();
    assert_invalid(&e, "[L]", "get");
    assert!(!e.to_string().contains("[G]"));
}

// ---------- 7. 한정 필터 ----------

#[test]
fn scope_filter_never_falls_back_and_reports_that_scopes_broken_file() {
    // (L, G, filter) -> 기대
    for l in ALL {
        for g in ALL {
            let f = setup(l, g);
            for (scope, st) in [(Scope::Local, l), (Scope::Global, g)] {
                let ctx = format!("Only({scope:?}) L={l:?} G={g:?}");
                for target in [false, true] {
                    let filter = ScopeFilter::Only(scope);
                    let r = if target {
                        f.svc.target(ID, filter)
                    } else {
                        f.svc.get(ID, filter)
                    };
                    match st {
                        M => assert!(matches!(r, Err(PhError::NotFound { .. })), "{ctx}: {r:?}"),
                        B => assert_invalid(
                            &r.unwrap_err(),
                            if scope == Scope::Local { "[L]" } else { "[G]" },
                            &ctx,
                        ),
                        O => {
                            let r = r.unwrap();
                            assert_eq!(r.prompt.scope, scope, "{ctx}");
                            assert!(r.fallback.is_none() && !r.ambiguous, "{ctx}");
                        }
                    }
                }
            }
        }
    }
}

// ---------- 4. R3 쓰기, 전 조합 ----------

#[derive(Clone, Copy, Debug)]
enum Op {
    Update,
    Remove,
    ExportRaw,
    SaveRaw,
}

fn apply(f: &Fx, op: Op, filter: ScopeFilter) -> Result<(), PhError> {
    match op {
        Op::Update => f
            .svc
            .update(
                ID,
                PromptPatch {
                    body: Some("CHANGED".into()),
                    ..Default::default()
                },
                filter,
            )
            .map(|_| ()),
        Op::Remove => f.svc.remove(ID, filter).map(|_| ()),
        Op::ExportRaw => f.svc.export_raw(ID, filter).map(|_| ()),
        Op::SaveRaw => f.svc.save_raw(ID, RAW_OK, filter).map(|_| ()),
    }
}

const OPS: [Op; 4] = [Op::Update, Op::Remove, Op::ExportRaw, Op::SaveRaw];

#[test]
fn write_ops_without_scope_follow_target_rule_and_never_touch_the_other_scope() {
    for l in ALL {
        for g in ALL {
            for op in OPS {
                let f = setup(l, g);
                let before = snap_both(&f);
                let ctx = format!("{op:?} L={l:?} G={g:?}");
                let r = apply(&f, op, ScopeFilter::All);
                match exp_target(l, g) {
                    Got::NotFound => {
                        assert!(matches!(r, Err(PhError::NotFound { .. })), "{ctx}: {r:?}");
                        assert_eq!(snap_both(&f), before, "{ctx}");
                    }
                    Got::Invalid(b) => {
                        assert_invalid(&r.unwrap_err(), b, &ctx);
                        assert_eq!(snap_both(&f), before, "{ctx}: 실패했는데 파일이 바뀌었다");
                    }
                    Got::Found { scope, .. } => {
                        r.unwrap_or_else(|e| panic!("{ctx}: {e}"));
                        let after = snap_both(&f);
                        // 반대편 scope 는 한 바이트도 바뀌지 않는다.
                        if scope == Scope::Local {
                            assert_eq!(after.1, before.1, "{ctx}: global 이 바뀌었다");
                        } else {
                            assert_eq!(after.0, before.0, "{ctx}: local 이 바뀌었다");
                        }
                        // 대상 scope 는 연산에 맞게 바뀐다.
                        let (b, a) = if scope == Scope::Local {
                            (&before.0, &after.0)
                        } else {
                            (&before.1, &after.1)
                        };
                        match op {
                            Op::ExportRaw => assert_eq!(a, b, "{ctx}"),
                            Op::Remove => assert!(!a.contains_key("x.md"), "{ctx}"),
                            Op::Update | Op::SaveRaw => {
                                assert_ne!(a.get("x.md"), b.get("x.md"), "{ctx}")
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn r3_write_without_scope_leaves_good_global_byte_identical() {
    for op in [Op::Update, Op::Remove, Op::SaveRaw, Op::ExportRaw] {
        let f = setup(B, O);
        let g_before = std::fs::read(f.g.join("x.md")).unwrap();
        let l_before = std::fs::read(f.l.join("x.md")).unwrap();
        let e = apply(&f, op, ScopeFilter::All).unwrap_err();
        assert_invalid(&e, "[L]", &format!("{op:?}"));
        assert_eq!(std::fs::read(f.g.join("x.md")).unwrap(), g_before, "{op:?}");
        assert_eq!(std::fs::read(f.l.join("x.md")).unwrap(), l_before, "{op:?}");
    }
}

#[test]
fn r3_explicit_global_succeeds_and_leaves_broken_local_untouched() {
    for op in OPS {
        let f = setup(B, O);
        let l_before = std::fs::read(f.l.join("x.md")).unwrap();
        apply(&f, op, ScopeFilter::Only(Scope::Global)).unwrap_or_else(|e| panic!("{op:?}: {e}"));
        assert_eq!(std::fs::read(f.l.join("x.md")).unwrap(), l_before, "{op:?}");
        match op {
            Op::Remove => assert!(!f.g.join("x.md").exists()),
            Op::Update => assert_eq!(
                f.svc
                    .get(ID, ScopeFilter::Only(Scope::Global))
                    .unwrap()
                    .prompt
                    .body,
                "CHANGED"
            ),
            Op::SaveRaw => assert_eq!(
                f.svc
                    .get(ID, ScopeFilter::Only(Scope::Global))
                    .unwrap()
                    .prompt
                    .body,
                "NEW"
            ),
            Op::ExportRaw => {}
        }
    }
}

#[test]
fn explicit_scope_writes_on_broken_target_scope_fail_and_keep_file() {
    for scope in [Scope::Local, Scope::Global] {
        for op in OPS {
            let (l, g) = if scope == Scope::Local {
                (B, O)
            } else {
                (O, B)
            };
            let f = setup(l, g);
            let before = snap_both(&f);
            let e = apply(&f, op, ScopeFilter::Only(scope)).unwrap_err();
            assert_invalid(
                &e,
                if scope == Scope::Local { "[L]" } else { "[G]" },
                &format!("{op:?}"),
            );
            assert_eq!(snap_both(&f), before, "{op:?} {scope:?}");
        }
    }
}

#[test]
fn r2_write_without_scope_hits_good_local_and_ignores_broken_global() {
    let f = setup(O, B);
    let g_before = std::fs::read(f.g.join("x.md")).unwrap();
    assert_eq!(f.svc.remove(ID, ScopeFilter::All).unwrap(), Scope::Local);
    assert!(!f.l.join("x.md").exists());
    assert_eq!(std::fs::read(f.g.join("x.md")).unwrap(), g_before);
}

// ---------- 8. list / search ----------

#[test]
fn list_and_search_skip_broken_files_for_every_combination_and_keep_good_ones() {
    for l in ALL {
        for g in ALL {
            let f = setup(l, g);
            let ctx = format!("L={l:?} G={g:?}");
            let r = f.svc.list(ScopeFilter::All, None).unwrap();
            let want_entries = [l, g].iter().filter(|s| **s == O).count();
            assert_eq!(r.entries.len(), want_entries, "{ctx}");
            let mut sk: Vec<_> = r
                .skipped
                .iter()
                .map(|s| (s.scope, s.entry.name.clone()))
                .collect();
            sk.sort_by_key(|(s, _)| s.as_str());
            let mut want_sk = Vec::new();
            if g == B {
                want_sk.push((Scope::Global, "x.md".to_string()));
            }
            if l == B {
                want_sk.push((Scope::Local, "x.md".to_string()));
            }
            want_sk.sort_by_key(|(s, _)| s.as_str());
            assert_eq!(sk, want_sk, "{ctx}");
            assert!(
                r.skipped.iter().all(|s| !s.entry.reason.is_empty()),
                "{ctx}"
            );
            // search 는 정상 항목만 돌려주고 실패하지 않는다.
            let hits = f.svc.search("LOCAL", ScopeFilter::All, None).unwrap();
            assert_eq!(hits.len(), usize::from(l == O), "{ctx}");
            let hits = f.svc.search("GLOBAL", ScopeFilter::All, None).unwrap();
            assert_eq!(hits.len(), usize::from(g == O), "{ctx}");
        }
    }
}

#[test]
fn list_with_broken_local_does_not_shadow_good_global_of_same_id() {
    let f = setup(B, O);
    let r = f.svc.list(ScopeFilter::All, None).unwrap();
    assert_eq!(r.entries.len(), 1);
    assert_eq!(r.entries[0].prompt.scope, Scope::Global);
    assert!(
        !r.entries[0].shadowed,
        "깨진 local 은 global 을 가리지 않는다 (get 의 fallback 과 일치)"
    );
}

#[test]
fn list_with_scope_filter_reports_only_that_scopes_skipped() {
    let f = setup(B, B);
    let l = f.svc.list(ScopeFilter::Only(Scope::Local), None).unwrap();
    assert_eq!(l.skipped.len(), 1);
    assert_eq!(l.skipped[0].scope, Scope::Local);
    let g = f.svc.list(ScopeFilter::Only(Scope::Global), None).unwrap();
    assert_eq!(g.skipped.len(), 1);
    assert_eq!(g.skipped[0].scope, Scope::Global);
}

#[test]
fn list_survives_many_kinds_of_broken_files() {
    let f = setup(M, O);
    for (n, c) in [
        ("a.md", ""),
        ("b.md", "+++"),
        ("c.md", "+++\n+++\n"),
        ("d.md", "+++\ntitle = 1\n+++\n"),
        ("e.md", "no front"),
    ] {
        std::fs::write(f.g.join(n), c).unwrap();
    }
    std::fs::write(f.g.join("f.md"), [0xff, 0xfe]).unwrap();
    let r = f.svc.list(ScopeFilter::All, None).unwrap();
    assert_eq!(r.entries.len(), 1);
    assert_eq!(r.skipped.len(), 6);
}

// ---------- 9. move ----------

fn move_expectation(src: St, dst: St) -> &'static str {
    match (src, dst) {
        (M, _) => "notfound",
        (B, _) => "invalid_src",
        (O, O) => "exists",
        (O, B) => "invalid_dst",
        (O, M) => "ok",
    }
}

#[test]
fn move_to_handles_every_source_and_destination_state_without_changing_files_on_failure() {
    for to in [Scope::Local, Scope::Global] {
        for src in ALL {
            for dst in ALL {
                let (l, g) = if to == Scope::Local {
                    (dst, src)
                } else {
                    (src, dst)
                };
                let f = setup(l, g);
                let before = snap_both(&f);
                let ctx = format!("to={to:?} src={src:?} dst={dst:?}");
                let r = f.svc.move_to(ID, to);
                let src_badge = if to == Scope::Local { "[G]" } else { "[L]" };
                let dst_badge = if to == Scope::Local { "[L]" } else { "[G]" };
                match move_expectation(src, dst) {
                    "notfound" => {
                        assert!(matches!(r, Err(PhError::NotFound { .. })), "{ctx}: {r:?}")
                    }
                    "invalid_src" => assert_invalid(&r.unwrap_err(), src_badge, &ctx),
                    "invalid_dst" => assert_invalid(&r.unwrap_err(), dst_badge, &ctx),
                    "exists" => assert!(
                        matches!(r, Err(PhError::AlreadyExists { .. })),
                        "{ctx}: {r:?}"
                    ),
                    _ => {
                        r.unwrap_or_else(|e| panic!("{ctx}: {e}"));
                        let after = snap_both(&f);
                        let (src_dir, dst_dir) = if to == Scope::Local {
                            (&after.1, &after.0)
                        } else {
                            (&after.0, &after.1)
                        };
                        assert!(
                            !src_dir.contains_key("x.md") && dst_dir.contains_key("x.md"),
                            "{ctx}"
                        );
                        continue;
                    }
                }
                assert_eq!(snap_both(&f), before, "{ctx}: 실패한 move 가 파일을 바꿨다");
            }
        }
    }
}

#[test]
fn move_never_overwrites_broken_destination_file() {
    let f = setup(B, O);
    let broken = std::fs::read(f.l.join("x.md")).unwrap();
    assert!(f.svc.move_to(ID, Scope::Local).is_err());
    assert_eq!(std::fs::read(f.l.join("x.md")).unwrap(), broken);
    assert!(f.g.join("x.md").is_file());
}

// ---------- 10. add ----------

#[test]
fn add_does_not_overwrite_broken_file_and_uses_suffix() {
    for scope in [Scope::Local, Scope::Global] {
        let (l, g) = if scope == Scope::Local {
            (B, M)
        } else {
            (M, B)
        };
        let f = setup(l, g);
        let dir = if scope == Scope::Local { &f.l } else { &f.g };
        let broken = std::fs::read(dir.join("x.md")).unwrap();
        let w = f
            .svc
            .add(new_prompt("x"), WriteTarget::Explicit(scope))
            .unwrap();
        assert_eq!(w.prompt.id.as_str(), "x-2", "{scope:?}");
        assert_eq!(
            std::fs::read(dir.join("x.md")).unwrap(),
            broken,
            "{scope:?}: 깨진 파일 내용이 바뀌었다"
        );
        assert!(dir.join("x-2.md").is_file());
        // 세 번째도 -3
        let w = f
            .svc
            .add(new_prompt("x"), WriteTarget::Explicit(scope))
            .unwrap();
        assert_eq!(w.prompt.id.as_str(), "x-3");
    }
}

#[test]
fn add_treats_broken_file_name_case_insensitively() {
    let f = setup(M, B);
    let w = f
        .svc
        .add(new_prompt("X"), WriteTarget::Explicit(Scope::Global))
        .unwrap();
    assert_eq!(w.prompt.id.as_str(), "X-2");
    assert_eq!(
        std::fs::read(f.g.join("x.md")).unwrap(),
        broken_text(Scope::Global).as_bytes()
    );
}

#[test]
fn add_treats_hangul_broken_file_name_as_taken() {
    let (_d, g, _l, svc) = fs_service(false);
    std::fs::create_dir_all(&g).unwrap();
    std::fs::write(g.join("코드-리뷰.md"), "garbage").unwrap();
    let w = svc.add(new_prompt("코드 리뷰"), WriteTarget::Auto).unwrap();
    assert_eq!(w.prompt.id.as_str(), "코드-리뷰-2");
    assert_eq!(std::fs::read(g.join("코드-리뷰.md")).unwrap(), b"garbage");
}

#[test]
fn add_to_one_scope_ignores_broken_file_of_same_name_in_other_scope() {
    let f = setup(B, M);
    let w = f
        .svc
        .add(new_prompt("x"), WriteTarget::Explicit(Scope::Global))
        .unwrap();
    assert_eq!(w.prompt.id.as_str(), "x");
}

// ---------- 오류 메시지 ----------

#[test]
fn invalid_format_message_has_scope_badge_location_and_guidance() {
    let f = setup(B, M);
    let m = f.svc.get(ID, ScopeFilter::All).unwrap_err().to_string();
    assert!(m.contains("[L]"), "{m}");
    assert!(
        m.contains(&f.l.display().to_string()),
        "위치가 있어야 한다: {m}"
    );
    assert!(m.contains("직접 고치거나 지우세요"), "{m}");
    assert!(m.contains("번째 줄") || m.contains("파일 형식 오류"), "{m}");
}

#[test]
fn unrelated_ids_are_not_affected_by_broken_neighbors() {
    let f = setup(B, B);
    f.svc
        .add(new_prompt("other"), WriteTarget::Explicit(Scope::Local))
        .unwrap();
    assert!(f.svc.get("other", ScopeFilter::All).is_ok());
    assert!(matches!(
        f.svc.get("nothing", ScopeFilter::All),
        Err(PhError::NotFound { .. })
    ));
}
