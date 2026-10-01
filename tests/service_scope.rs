//! `PromptService` scope 병합, 우선순위, 쓰기 대상 규칙 (SPEC 3.2절). memory 와 fs 양쪽에서 검증한다.

mod common;

use common::*;
use ph::core::error::PhError;
use ph::core::model::{NewPrompt, PromptPatch, Scope};
use ph::core::service::{ScopeFilter, WriteTarget};

const G: WriteTarget = WriteTarget::Explicit(Scope::Global);
const L: WriteTarget = WriteTarget::Explicit(Scope::Local);

/// memory 와 fs 서비스를 모두 돌린다.
fn services(
    with_local: bool,
) -> Vec<(
    &'static str,
    ph::core::service::PromptService,
    Option<tempfile::TempDir>,
)> {
    let (d, _, _, fs) = fs_service(with_local);
    vec![
        ("memory", memory_service(with_local), None),
        ("fs", fs, Some(d)),
    ]
}

// ---------- 쓰기 대상 ----------

#[test]
fn auto_writes_to_local_when_present_and_reports_auto_selected() {
    for (n, s, _d) in services(true) {
        let w = s.add(new_prompt("a"), WriteTarget::Auto).unwrap();
        assert_eq!(
            (w.scope, w.auto_selected, w.prompt.scope),
            (Scope::Local, true, Scope::Local),
            "{n}"
        );
        assert!(s.get("a", ScopeFilter::Only(Scope::Global)).is_err(), "{n}");
    }
}

#[test]
fn auto_writes_to_global_when_no_local() {
    for (n, s, _d) in services(false) {
        let w = s.add(new_prompt("a"), WriteTarget::Auto).unwrap();
        assert_eq!((w.scope, w.auto_selected), (Scope::Global, true), "{n}");
    }
}

#[test]
fn explicit_target_is_never_auto_selected_and_is_respected() {
    for (n, s, _d) in services(true) {
        let w = s.add(new_prompt("g"), G).unwrap();
        assert_eq!((w.scope, w.auto_selected), (Scope::Global, false), "{n}");
        let w = s.add(new_prompt("l"), L).unwrap();
        assert_eq!((w.scope, w.auto_selected), (Scope::Local, false), "{n}");
    }
}

#[test]
fn explicit_local_without_local_errors_and_never_falls_back_to_global() {
    for (n, s, d) in services(false) {
        assert!(
            matches!(s.add(new_prompt("a"), L), Err(PhError::LocalNotInitialized)),
            "{n}"
        );
        assert!(
            s.list(ScopeFilter::All, None).unwrap().entries.is_empty(),
            "{n}"
        );
        assert!(
            matches!(
                s.list(ScopeFilter::Only(Scope::Local), None),
                Err(PhError::LocalNotInitialized)
            ),
            "{n}"
        );
        assert!(
            matches!(
                s.get("a", ScopeFilter::Only(Scope::Local)),
                Err(PhError::LocalNotInitialized)
            ),
            "{n}"
        );
        assert!(
            matches!(
                s.move_to("a", Scope::Local),
                Err(PhError::LocalNotInitialized)
            ),
            "{n}"
        );
        assert!(
            matches!(
                s.remove("a", ScopeFilter::Only(Scope::Local)),
                Err(PhError::LocalNotInitialized)
            ),
            "{n}"
        );
        if let Some(d) = d {
            // global 에도 아무것도 쓰이지 않았다.
            assert!(!d.path().join("global").exists(), "{n}");
        }
    }
}

#[test]
fn has_local_and_locations() {
    let (_d, g, l, s) = fs_service(true);
    assert!(s.has_local());
    assert_eq!(s.global_location(), g.display().to_string());
    assert_eq!(s.local_location(), Some(l.display().to_string()));
    let s = memory_service(false);
    assert!(!s.has_local());
    assert_eq!(s.local_location(), None);
}

// ---------- 병합 ----------

#[test]
fn list_merges_both_scopes_sorted_by_id_ignoring_case() {
    for (n, s, _d) in services(true) {
        s.add(new_prompt("beta"), G).unwrap();
        s.add(new_prompt("Alpha"), L).unwrap();
        s.add(new_prompt("gamma"), G).unwrap();
        s.add(new_prompt("Delta"), L).unwrap();
        let ids: Vec<_> = s
            .list(ScopeFilter::All, None)
            .unwrap()
            .entries
            .iter()
            .map(|e| e.prompt.id.as_str().to_string())
            .collect();
        assert_eq!(ids, ["Alpha", "beta", "Delta", "gamma"], "{n}");
    }
}

#[test]
fn same_id_local_wins_global_is_shadowed_and_local_listed_first() {
    for (n, s, _d) in services(true) {
        s.add(new_prompt("공통"), G).unwrap();
        s.add(new_prompt("공통"), L).unwrap();
        let l = s.list(ScopeFilter::All, None).unwrap();
        assert_eq!(l.entries.len(), 2, "{n}");
        assert_eq!(l.entries[0].prompt.scope, Scope::Local, "{n}");
        assert!(!l.entries[0].shadowed, "{n}");
        assert_eq!(l.entries[1].prompt.scope, Scope::Global, "{n}");
        assert!(l.entries[1].shadowed, "{n}");
    }
}

#[test]
fn shadowing_is_case_insensitive() {
    for (n, s, _d) in services(true) {
        s.add(new_prompt("Code"), L).unwrap();
        s.add(new_prompt("code"), G).unwrap();
        let l = s.list(ScopeFilter::All, None).unwrap();
        let g = l
            .entries
            .iter()
            .find(|e| e.prompt.scope == Scope::Global)
            .unwrap();
        assert!(g.shadowed, "{n}");
    }
}

#[test]
fn shadowed_flag_is_independent_of_tag_filter() {
    for (n, s, _d) in services(true) {
        s.add(new_prompt("x"), L).unwrap(); // local 은 태그 없음
        s.add(
            NewPrompt {
                tags: vec!["review".into()],
                ..new_prompt("x")
            },
            G,
        )
        .unwrap();
        let l = s.list(ScopeFilter::All, Some("review")).unwrap();
        assert_eq!(l.entries.len(), 1, "{n}");
        assert!(
            l.entries[0].shadowed,
            "{n}: local 이 필터로 빠져도 global 은 가려진 상태다"
        );
    }
}

#[test]
fn scope_filter_only_shows_that_scope_and_nothing_is_shadowed() {
    for (n, s, _d) in services(true) {
        s.add(new_prompt("x"), L).unwrap();
        s.add(new_prompt("x"), G).unwrap();
        s.add(new_prompt("y"), G).unwrap();
        let g = s.list(ScopeFilter::Only(Scope::Global), None).unwrap();
        assert_eq!(g.entries.len(), 2, "{n}");
        assert!(
            g.entries
                .iter()
                .all(|e| !e.shadowed && e.prompt.scope == Scope::Global),
            "{n}"
        );
        let l = s.list(ScopeFilter::Only(Scope::Local), None).unwrap();
        assert_eq!(l.entries.len(), 1, "{n}");
    }
}

#[test]
fn get_prefers_local_and_flags_ambiguous_only_when_in_both() {
    for (n, s, _d) in services(true) {
        s.add(new_prompt("both"), G).unwrap();
        s.add(new_prompt("both"), L).unwrap();
        s.add(new_prompt("only-g"), G).unwrap();
        let r = s.get("both", ScopeFilter::All).unwrap();
        assert_eq!((r.prompt.scope, r.ambiguous), (Scope::Local, true), "{n}");
        let r = s.get("BOTH", ScopeFilter::All).unwrap();
        assert_eq!(r.prompt.scope, Scope::Local, "{n}");
        let r = s.get("both", ScopeFilter::Only(Scope::Global)).unwrap();
        assert_eq!((r.prompt.scope, r.ambiguous), (Scope::Global, false), "{n}");
        let r = s.get("only-g", ScopeFilter::All).unwrap();
        assert_eq!((r.prompt.scope, r.ambiguous), (Scope::Global, false), "{n}");
    }
}

#[test]
fn get_missing_and_invalid_id() {
    for (n, s, _d) in services(true) {
        assert!(
            matches!(
                s.get("none", ScopeFilter::All),
                Err(PhError::NotFound { .. })
            ),
            "{n}"
        );
        assert!(
            matches!(
                s.get("a/b", ScopeFilter::All),
                Err(PhError::InvalidId { .. })
            ),
            "{n}"
        );
        assert!(
            matches!(s.get("", ScopeFilter::All), Err(PhError::InvalidId { .. })),
            "{n}"
        );
    }
}

#[test]
fn get_matches_nfd_input_against_nfc_stored_id() {
    for (n, s, _d) in services(false) {
        s.add(new_prompt("한글"), WriteTarget::Auto).unwrap();
        let nfd = "\u{1112}\u{1161}\u{11ab}\u{1100}\u{1173}\u{11af}";
        assert!(s.get(nfd, ScopeFilter::All).is_ok(), "{n}");
    }
}

// ---------- add ----------

#[test]
fn add_duplicates_only_conflict_within_same_scope() {
    for (n, s, _d) in services(true) {
        let a = s.add(new_prompt("x"), G).unwrap();
        let b = s.add(new_prompt("x"), L).unwrap();
        assert_eq!(a.prompt.id.as_str(), "x", "{n}");
        assert_eq!(b.prompt.id.as_str(), "x", "{n}");
        let c = s.add(new_prompt("x"), G).unwrap();
        let d = s.add(new_prompt("X"), G).unwrap();
        assert_eq!(c.prompt.id.as_str(), "x-2", "{n}");
        assert_eq!(d.prompt.id.as_str(), "X-3", "{n}");
    }
}

#[test]
fn add_rejects_invalid_title_without_writing() {
    for (n, s, _d) in services(false) {
        for t in ["", "  ", "a/b", "CON", "x."] {
            assert!(
                matches!(
                    s.add(new_prompt(t), WriteTarget::Auto),
                    Err(PhError::InvalidId { .. })
                ),
                "{n} {t:?}"
            );
        }
        assert!(
            s.list(ScopeFilter::All, None).unwrap().entries.is_empty(),
            "{n}"
        );
    }
}

#[test]
fn add_trims_title_cleans_tags_and_sets_timestamps_from_clock() {
    for (n, s, _d) in services(false) {
        let w = s
            .add(
                NewPrompt {
                    title: "  코드 리뷰  ".into(),
                    body: "b".into(),
                    tags: vec![" a ".into(), "a".into(), "".into(), "  ".into(), "b".into()],
                    description: Some("   ".into()),
                },
                WriteTarget::Auto,
            )
            .unwrap();
        assert_eq!(w.prompt.title, "코드 리뷰", "{n}");
        assert_eq!(w.prompt.id.as_str(), "코드-리뷰", "{n}");
        assert_eq!(w.prompt.tags, ["a", "b"], "{n}");
        assert_eq!(w.prompt.description, None, "{n}");
        assert_eq!(
            (w.prompt.created_at, w.prompt.updated_at),
            (t0(), t0()),
            "{n}"
        );
        // 저장된 것과 반환된 것이 같다.
        assert_eq!(
            s.get("코드-리뷰", ScopeFilter::All).unwrap().prompt,
            w.prompt,
            "{n}"
        );
    }
}

// ---------- update / remove / move ----------

#[test]
fn update_and_remove_follow_read_rule_local_first_and_global_needs_explicit_filter() {
    for (n, s, _d) in services(true) {
        s.add(new_prompt("x"), G).unwrap();
        s.add(new_prompt("x"), L).unwrap();
        let u = s
            .update(
                "x",
                PromptPatch {
                    body: Some("수정".into()),
                    ..Default::default()
                },
                ScopeFilter::All,
            )
            .unwrap();
        assert_eq!(u.scope, Scope::Local, "{n}");
        assert_eq!(
            s.get("x", ScopeFilter::Only(Scope::Global))
                .unwrap()
                .prompt
                .body,
            "x 본문",
            "{n}"
        );
        assert_eq!(
            s.remove("x", ScopeFilter::All).unwrap(),
            Scope::Local,
            "{n}"
        );
        assert_eq!(
            s.remove("x", ScopeFilter::All).unwrap(),
            Scope::Global,
            "{n}"
        );
        assert!(
            matches!(
                s.remove("x", ScopeFilter::All),
                Err(PhError::NotFound { .. })
            ),
            "{n}"
        );
    }
}

#[test]
fn update_description_can_be_cleared_and_title_change_keeps_id() {
    for (n, s, _d) in services(false) {
        s.add(
            NewPrompt {
                description: Some("d".into()),
                ..new_prompt("t")
            },
            WriteTarget::Auto,
        )
        .unwrap();
        let u = s
            .update(
                "t",
                PromptPatch {
                    description: Some(None),
                    title: Some("완전 다른 제목".into()),
                    ..Default::default()
                },
                ScopeFilter::All,
            )
            .unwrap();
        assert_eq!(u.prompt.description, None, "{n}");
        assert_eq!(u.prompt.id.as_str(), "t", "{n}");
        assert!(s.get("t", ScopeFilter::All).is_ok(), "{n}");
    }
}

#[test]
fn update_rejects_blank_title_and_missing_id_leaving_data_unchanged() {
    for (n, s, _d) in services(false) {
        let w = s.add(new_prompt("t"), WriteTarget::Auto).unwrap();
        let r = s.update(
            "t",
            PromptPatch {
                title: Some("  ".into()),
                ..Default::default()
            },
            ScopeFilter::All,
        );
        assert!(matches!(r, Err(PhError::Usage(_))), "{n}");
        assert_eq!(
            s.get("t", ScopeFilter::All).unwrap().prompt,
            w.prompt,
            "{n}"
        );
        let r = s.update("zz", PromptPatch::default(), ScopeFilter::All);
        assert!(matches!(r, Err(PhError::NotFound { .. })), "{n}");
    }
}

#[test]
fn move_preserves_content_and_timestamps_and_removes_source() {
    for (n, s, _d) in services(true) {
        let w = s.add(new_prompt("m"), G).unwrap();
        let m = s.move_to("m", Scope::Local).unwrap();
        assert_eq!(m.scope, Scope::Local, "{n}");
        assert_eq!(m.prompt.body, w.prompt.body, "{n}");
        assert_eq!(
            (m.prompt.created_at, m.prompt.updated_at),
            (t0(), t0()),
            "{n}"
        );
        assert!(
            matches!(
                s.get("m", ScopeFilter::Only(Scope::Global)),
                Err(PhError::NotFound { .. })
            ),
            "{n}"
        );
        assert_eq!(
            s.get("m", ScopeFilter::Only(Scope::Local))
                .unwrap()
                .prompt
                .scope,
            Scope::Local,
            "{n}"
        );
        // 되돌리기
        assert_eq!(
            s.move_to("m", Scope::Global).unwrap().scope,
            Scope::Global,
            "{n}"
        );
    }
}

#[test]
fn move_conflict_or_missing_source_changes_nothing() {
    for (n, s, _d) in services(true) {
        s.add(new_prompt("c"), G).unwrap();
        s.add(new_prompt("C"), L).unwrap();
        assert!(
            matches!(
                s.move_to("c", Scope::Local),
                Err(PhError::AlreadyExists { .. })
            ),
            "{n}"
        );
        assert_eq!(
            s.list(ScopeFilter::All, None).unwrap().entries.len(),
            2,
            "{n}"
        );
        assert!(
            matches!(
                s.move_to("nothing", Scope::Local),
                Err(PhError::NotFound { .. })
            ),
            "{n}"
        );
        // 이미 그 scope 에만 있는 것을 같은 scope 로 옮기면 출발 scope(반대편)에 없어 NotFound
        s.add(new_prompt("only-l"), L).unwrap();
        assert!(
            matches!(
                s.move_to("only-l", Scope::Local),
                Err(PhError::NotFound { .. })
            ),
            "{n}"
        );
    }
}

// ---------- 깨진 파일 / raw 편집 ----------

#[test]
fn broken_file_is_reported_as_skipped_with_scope_and_others_still_listed() {
    let (_d, g, l, s) = fs_service(true);
    s.add(new_prompt("ok-g"), G).unwrap();
    s.add(new_prompt("ok-l"), L).unwrap();
    std::fs::write(g.join("bad-g.md"), "broken").unwrap();
    std::fs::write(l.join("bad-l.md"), "+++\nx").unwrap();
    let r = s.list(ScopeFilter::All, None).unwrap();
    assert_eq!(r.entries.len(), 2);
    let mut sk: Vec<_> = r
        .skipped
        .iter()
        .map(|k| (k.scope, k.entry.name.as_str()))
        .collect();
    sk.sort_by_key(|(sc, _)| sc.as_str());
    assert_eq!(
        sk,
        [(Scope::Global, "bad-g.md"), (Scope::Local, "bad-l.md")]
    );
    // 검색에서도 실패하지 않는다.
    assert_eq!(s.search("본문", ScopeFilter::All, None).unwrap().len(), 2);
}

#[test]
fn save_raw_with_broken_frontmatter_keeps_original_on_fs() {
    let (_d, g, _l, s) = fs_service(false);
    s.add(new_prompt("문서"), WriteTarget::Auto).unwrap();
    let before = std::fs::read(g.join("문서.md")).unwrap();
    for bad in ["", "garbage", "+++\ntitle = \n+++\nx", "+++\ncreated_at=\"2026-09-30T12:00:00+09:00\"\nupdated_at=\"2026-09-30T12:00:00+09:00\"\n+++\n본문"] {
        assert!(s.save_raw("문서", bad, ScopeFilter::All).is_err(), "{bad:?}");
        assert_eq!(std::fs::read(g.join("문서.md")).unwrap(), before, "{bad:?}");
    }
    // 빈 title 은 거부
    let blank = "+++\ntitle = \"  \"\ncreated_at = \"2026-09-30T12:00:00+09:00\"\nupdated_at = \"2026-09-30T12:00:00+09:00\"\n+++\nx";
    assert!(matches!(
        s.save_raw("문서", blank, ScopeFilter::All),
        Err(PhError::Usage(_))
    ));
    assert_eq!(std::fs::read(g.join("문서.md")).unwrap(), before);
}

#[test]
fn save_raw_preserves_id_scope_and_created_at_but_refreshes_updated_at() {
    let s = memory_service(true);
    s.add(new_prompt("문서"), L).unwrap();
    let raw = s.export_raw("문서", ScopeFilter::All).unwrap();
    // created_at 을 조작한 편집본
    let edited = raw
        .replace("2026-09-30T12:00:00+09:00", "1999-01-01T00:00:00+09:00")
        .replace("문서 본문", "고침");
    let w = s.save_raw("문서", &edited, ScopeFilter::All).unwrap();
    assert_eq!(w.prompt.created_at, t0());
    assert_eq!(w.prompt.body, "고침");
    assert_eq!((w.scope, w.prompt.id.as_str()), (Scope::Local, "문서"));
}

#[test]
fn export_raw_is_lf_and_reparseable_via_save_raw() {
    for (n, s, _d) in services(false) {
        s.add(new_prompt("r"), WriteTarget::Auto).unwrap();
        let raw = s.export_raw("r", ScopeFilter::All).unwrap();
        assert!(!raw.contains('\r'), "{n}");
        // 에디터가 CRLF+BOM 으로 저장해도 받아들인다.
        let crlf = format!("\u{feff}{}", raw.replace('\n', "\r\n"));
        assert!(s.save_raw("r", &crlf, ScopeFilter::All).is_ok(), "{n}");
    }
}

// ---------- 발견한 버그 재현 ----------

/// BUG-1: title/description 에 개행이 들어 있고 그 줄이 `+++` 이면 저장은 성공하지만 다시 읽을 수 없다.
#[test]
fn bug_update_with_delimiter_line_in_description_must_not_corrupt_file() {
    let (_d, _g, _l, s) = fs_service(false);
    s.add(new_prompt("t"), WriteTarget::Auto).unwrap();
    let r = s.update(
        "t",
        PromptPatch {
            description: Some(Some("앞\n+++\n뒤".into())),
            ..Default::default()
        },
        ScopeFilter::All,
    );
    // 저장이 거부되든 성공하든, 이후에도 조회 가능해야 한다.
    if r.is_ok() {
        assert!(
            s.get("t", ScopeFilter::All).is_ok(),
            "저장 성공 후 get 이 실패한다 (파일 손상)"
        );
    }
}

/// BUG-2 (Linux): NFD 파일명(macOS 에서 만든 저장소를 git 으로 받은 경우)은 list 에 보이지만
/// NFC id 로 만든 경로와 달라서 삭제/수정이 실패하거나 중복 파일이 생긴다.
#[test]
fn bug_nfd_named_file_can_be_removed_after_listing() {
    let (_d, g, _l, s) = fs_service(false);
    std::fs::create_dir_all(&g).unwrap();
    let nfd = "\u{1112}\u{1161}\u{11ab}\u{1100}\u{1173}\u{11af}";
    let raw = "+++\ntitle = \"한글\"\ncreated_at = \"2026-09-30T12:00:00+09:00\"\nupdated_at = \"2026-09-30T12:00:00+09:00\"\n+++\nx";
    std::fs::write(g.join(format!("{nfd}.md")), raw).unwrap();
    // 파일시스템이 정규화를 하지 않는 경우(Linux)에만 의미가 있다.
    if std::fs::read_dir(&g).unwrap().count() != 1 || g.join("한글.md").exists() {
        return;
    }
    assert_eq!(s.list(ScopeFilter::All, None).unwrap().entries.len(), 1);
    assert!(
        s.remove("한글", ScopeFilter::All).is_ok(),
        "list 에는 보이지만 remove 가 NotFound"
    );
    assert_eq!(std::fs::read_dir(&g).unwrap().count(), 0);
}

/// BUG-2 변형: NFD 파일을 update 하면 NFC 파일이 새로 생겨 같은 id 가 두 개가 된다.
#[test]
fn bug_nfd_named_file_update_must_not_duplicate() {
    let (_d, g, _l, s) = fs_service(false);
    std::fs::create_dir_all(&g).unwrap();
    let nfd = "\u{1112}\u{1161}\u{11ab}\u{1100}\u{1173}\u{11af}";
    let raw = "+++\ntitle = \"한글\"\ncreated_at = \"2026-09-30T12:00:00+09:00\"\nupdated_at = \"2026-09-30T12:00:00+09:00\"\n+++\nx";
    std::fs::write(g.join(format!("{nfd}.md")), raw).unwrap();
    if g.join("한글.md").exists() {
        return;
    }
    s.update(
        "한글",
        PromptPatch {
            body: Some("y".into()),
            ..Default::default()
        },
        ScopeFilter::All,
    )
    .unwrap();
    assert_eq!(
        std::fs::read_dir(&g).unwrap().count(),
        1,
        "NFD 와 NFC 파일이 공존한다"
    );
}
