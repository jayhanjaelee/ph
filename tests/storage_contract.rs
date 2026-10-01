//! `Storage` 계약 테스트: fs 와 memory 구현이 같은 동작을 해야 한다. 그리고 fs 고유 동작
//! (깨진 파일 skipped, atomic write, local 탐색).

mod common;

use std::path::Path;

use common::*;
use ph::core::error::PhError;
use ph::core::model::{PromptId, Scope};
use ph::core::storage::Storage;
use ph::storage::{find_local, FsStorage, MemoryStorage};

fn pid(s: &str) -> PromptId {
    PromptId::parse(s).unwrap()
}

/// 두 구현에 같은 시나리오를 돌린다. (이름, 저장소, 임시 디렉터리 수명 유지용)
fn both(scope: Scope) -> Vec<(&'static str, Box<dyn Storage>, Option<tempfile::TempDir>)> {
    let d = tempfile::tempdir().unwrap();
    let fs = FsStorage::new(scope, d.path().join("prompts"));
    vec![
        ("fs", Box::new(fs), Some(d)),
        ("memory", Box::new(MemoryStorage::new(scope)), None),
    ]
}

#[test]
fn empty_storage_lists_nothing_and_get_returns_none() {
    for (name, s, _d) in both(Scope::Global) {
        let l = s.list().unwrap();
        assert!(l.prompts.is_empty() && l.skipped.is_empty(), "{name}");
        assert!(s.get(&pid("none")).unwrap().is_none(), "{name}");
    }
}

#[test]
fn put_then_get_returns_identical_prompt() {
    for (name, s, _d) in both(Scope::Global) {
        let mut p = prompt("코드-리뷰", Scope::Global);
        p.body = "다음 {{focus}}\n\n끝".into();
        s.put(&p).unwrap();
        assert_eq!(s.get(&p.id).unwrap().unwrap(), p, "{name}");
    }
}

#[test]
fn put_overwrites_existing_id() {
    for (name, s, _d) in both(Scope::Global) {
        let mut p = prompt("x", Scope::Global);
        s.put(&p).unwrap();
        p.body = "새 본문".into();
        s.put(&p).unwrap();
        assert_eq!(s.get(&p.id).unwrap().unwrap().body, "새 본문", "{name}");
        assert_eq!(s.list().unwrap().prompts.len(), 1, "{name}");
    }
}

#[test]
fn stored_prompt_takes_scope_of_storage() {
    for (name, s, _d) in both(Scope::Local) {
        // 다른 scope 로 표시된 prompt 를 넣어도 저장소의 scope 로 읽힌다.
        s.put(&prompt("x", Scope::Global)).unwrap();
        assert_eq!(
            s.get(&pid("x")).unwrap().unwrap().scope,
            Scope::Local,
            "{name}"
        );
        assert_eq!(s.list().unwrap().prompts[0].scope, Scope::Local, "{name}");
        assert_eq!(s.scope(), Scope::Local, "{name}");
    }
}

#[test]
fn list_is_sorted_by_id_in_both_implementations() {
    for (name, s, _d) in both(Scope::Global) {
        for id in ["b", "가", "a", "B", "나"] {
            s.put(&prompt(id, Scope::Global)).unwrap();
        }
        let ids: Vec<String> = s
            .list()
            .unwrap()
            .prompts
            .iter()
            .map(|p| p.id.as_str().to_string())
            .collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted, "{name}");
        assert_eq!(ids.len(), 5, "{name}");
    }
}

#[test]
fn delete_removes_and_missing_delete_is_not_found() {
    for (name, s, _d) in both(Scope::Global) {
        let p = prompt("x", Scope::Global);
        s.put(&p).unwrap();
        s.delete(&p.id).unwrap();
        assert!(s.get(&p.id).unwrap().is_none(), "{name}");
        assert!(
            matches!(s.delete(&p.id), Err(PhError::NotFound { ref id }) if id == "x"),
            "{name}"
        );
        assert!(
            matches!(s.delete(&pid("never")), Err(PhError::NotFound { .. })),
            "{name}"
        );
    }
}

#[test]
fn unicode_title_and_long_body_roundtrip() {
    for (name, s, _d) in both(Scope::Global) {
        let mut p = prompt("한글-제목", Scope::Global);
        p.title = "한글 제목 🚀".into();
        p.body = "긴 본문 ".repeat(100_000);
        s.put(&p).unwrap();
        assert_eq!(s.get(&p.id).unwrap().unwrap(), p, "{name}");
    }
}

#[test]
fn empty_body_and_no_optional_fields_roundtrip() {
    for (name, s, _d) in both(Scope::Global) {
        let mut p = prompt("min", Scope::Global);
        p.body = String::new();
        p.tags = vec![];
        p.description = None;
        s.put(&p).unwrap();
        assert_eq!(s.get(&p.id).unwrap().unwrap(), p, "{name}");
    }
}

#[test]
fn location_is_reported() {
    let d = tempfile::tempdir().unwrap();
    let dir = d.path().join("p");
    assert_eq!(
        FsStorage::new(Scope::Global, dir.clone()).location(),
        dir.display().to_string()
    );
    assert_eq!(MemoryStorage::new(Scope::Global).location(), "memory");
}

// ---------- fs 고유 ----------

fn fs_in(d: &tempfile::TempDir) -> FsStorage {
    FsStorage::new(Scope::Global, d.path().to_path_buf())
}

#[test]
fn fs_list_on_missing_directory_is_empty_and_put_creates_it() {
    let d = tempfile::tempdir().unwrap();
    let dir = d.path().join("a").join("b").join("prompts");
    let s = FsStorage::new(Scope::Global, dir.clone());
    assert!(s.list().unwrap().prompts.is_empty());
    s.put(&prompt("x", Scope::Global)).unwrap();
    assert!(dir.join("x.md").is_file());
}

#[test]
fn fs_file_is_named_after_id_with_md_extension_and_written_with_lf() {
    let d = tempfile::tempdir().unwrap();
    let s = fs_in(&d);
    let mut p = prompt("코드-리뷰", Scope::Global);
    p.body = "a\r\nb\r\n".into();
    s.put(&p).unwrap();
    let bytes = std::fs::read(d.path().join("코드-리뷰.md")).unwrap();
    assert!(bytes.starts_with(b"+++\n"));
    assert!(!bytes.contains(&b'\r'));
    assert_ne!(&bytes[..3], [0xEF, 0xBB, 0xBF]);
}

#[test]
fn fs_broken_files_are_skipped_not_fatal() {
    let d = tempfile::tempdir().unwrap();
    let s = fs_in(&d);
    s.put(&prompt("ok", Scope::Global)).unwrap();
    let p = d.path();
    std::fs::write(p.join("no-front.md"), "그냥 텍스트").unwrap();
    std::fs::write(p.join("unclosed.md"), "+++\ntitle = \"a\"\n").unwrap();
    std::fs::write(p.join("bad-toml.md"), "+++\ntitle = \n+++\nbody").unwrap();
    std::fs::write(p.join("empty.md"), "").unwrap();
    std::fs::write(p.join("not-utf8.md"), [0xff, 0xfe, 0xfd, b'+', b'+', b'+']).unwrap();
    // 유효하지 않은 id 이름
    std::fs::write(p.join("CON.md"), "+++\n+++\n").unwrap();
    std::fs::write(p.join("has space.md"), "x").unwrap();
    // 무시 대상
    std::fs::write(p.join("notes.txt"), "x").unwrap();
    std::fs::write(p.join("README"), "x").unwrap();
    std::fs::create_dir(p.join("dir.md")).unwrap();

    let l = s.list().unwrap();
    assert_eq!(l.prompts.len(), 1);
    assert_eq!(l.prompts[0].id.as_str(), "ok");
    let mut names: Vec<_> = l.skipped.iter().map(|s| s.name.clone()).collect();
    names.sort();
    assert_eq!(
        names,
        [
            "CON.md",
            "bad-toml.md",
            "empty.md",
            "has space.md",
            "no-front.md",
            "not-utf8.md",
            "unclosed.md"
        ]
    );
    assert!(l.skipped.iter().all(|s| !s.reason.is_empty()));
}

#[test]
fn fs_get_on_broken_file_returns_invalid_format() {
    let d = tempfile::tempdir().unwrap();
    let s = fs_in(&d);
    std::fs::write(d.path().join("bad.md"), "garbage").unwrap();
    assert!(matches!(
        s.get(&pid("bad")),
        Err(PhError::InvalidFormat { line: Some(1), .. })
    ));
}

#[test]
fn fs_reads_hand_edited_bom_crlf_file() {
    let d = tempfile::tempdir().unwrap();
    let s = fs_in(&d);
    let raw = "\u{feff}+++\r\ntitle = \"수동\"\r\ncreated_at = \"2026-09-30T12:00:00+09:00\"\r\nupdated_at = \"2026-09-30T12:00:00+09:00\"\r\n+++\r\n본문\r\n";
    std::fs::write(d.path().join("수동.md"), raw).unwrap();
    let p = s.get(&pid("수동")).unwrap().unwrap();
    assert_eq!(p.title, "수동");
    assert_eq!(p.body, "본문\n");
    assert_eq!(s.list().unwrap().prompts.len(), 1);
}

#[test]
fn fs_put_leaves_no_temp_files() {
    let d = tempfile::tempdir().unwrap();
    let s = fs_in(&d);
    for i in 0..20 {
        let mut p = prompt("same", Scope::Global);
        p.body = format!("v{i}");
        s.put(&p).unwrap();
    }
    let names: Vec<_> = std::fs::read_dir(d.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["same.md"]);
}

#[test]
fn fs_put_failure_keeps_original_and_cleans_temp() {
    // 대상 경로 `x.md` 가 디렉터리이면 rename 이 실패한다.
    let d = tempfile::tempdir().unwrap();
    let s = fs_in(&d);
    std::fs::create_dir(d.path().join("x.md")).unwrap();
    let r = s.put(&prompt("x", Scope::Global));
    assert!(matches!(r, Err(PhError::Io { .. })), "{r:?}");
    let leftovers: Vec<_> = std::fs::read_dir(d.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(leftovers, ["x.md"], "임시 파일이 남았다");
}

#[cfg(unix)]
#[test]
fn fs_put_into_read_only_directory_fails_with_io_error_and_no_partial_file() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempfile::tempdir().unwrap();
    let dir = d.path().join("ro");
    std::fs::create_dir(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    // root 는 권한 검사를 우회하므로 실제로 막히는지 먼저 확인한다.
    let blocked = std::fs::write(dir.join("probe"), "x").is_err();
    let s = FsStorage::new(Scope::Global, dir.clone());
    let r = s.put(&prompt("x", Scope::Global));
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    if blocked {
        match r {
            Err(PhError::Io { context, .. }) => assert!(context.contains("x.md"), "{context}"),
            other => panic!("expected Io error, got {other:?}"),
        }
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
    }
}

// ---------- platform::fs::atomic_write ----------

#[test]
fn atomic_write_creates_parents_overwrites_and_handles_empty_and_large() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("a").join("b").join("f.md");
    ph::platform::fs::atomic_write(&p, b"").unwrap();
    assert_eq!(std::fs::read(&p).unwrap(), b"");
    let big = vec![b'x'; 5 * 1024 * 1024];
    ph::platform::fs::atomic_write(&p, &big).unwrap();
    assert_eq!(std::fs::read(&p).unwrap(), big);
    assert_eq!(std::fs::read_dir(p.parent().unwrap()).unwrap().count(), 1);
}

#[test]
fn atomic_write_rejects_path_without_file_name() {
    let d = tempfile::tempdir().unwrap();
    assert!(ph::platform::fs::atomic_write(&d.path().join(".."), b"x").is_err());
}

#[test]
fn atomic_write_concurrent_writers_never_expose_partial_content() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("f.md");
    let a = "A".repeat(100_000);
    let b = "B".repeat(100_000);
    std::thread::scope(|sc| {
        for content in [&a, &b] {
            let p = &p;
            sc.spawn(move || {
                for _ in 0..30 {
                    ph::platform::fs::atomic_write(p, content.as_bytes()).unwrap();
                }
            });
        }
        let p = &p;
        let (a, b) = (&a, &b);
        sc.spawn(move || {
            for _ in 0..200 {
                if let Ok(got) = std::fs::read_to_string(p) {
                    assert!(
                        &got == a || &got == b,
                        "부분 내용이 보였다 ({} bytes)",
                        got.len()
                    );
                }
            }
        });
    });
    assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 1);
}

// ---------- local 탐색 ----------

fn mk(p: &Path) {
    std::fs::create_dir_all(p).unwrap();
}

#[test]
fn find_local_finds_in_cwd_and_ancestors() {
    let d = tempfile::tempdir().unwrap();
    let proj = d.path().join("proj");
    let deep = proj.join("a").join("b").join("c");
    mk(&deep);
    assert_eq!(find_local(&deep, None), None);
    mk(&proj.join(".ph"));
    assert_eq!(find_local(&deep, None), Some(proj.join(".ph")));
    assert_eq!(find_local(&proj, None), Some(proj.join(".ph")));
}

#[test]
fn find_local_prefers_nearest_ancestor() {
    let d = tempfile::tempdir().unwrap();
    let outer = d.path().join("outer");
    let inner = outer.join("inner");
    let deep = inner.join("x");
    mk(&deep);
    mk(&outer.join(".ph"));
    mk(&inner.join(".ph"));
    assert_eq!(find_local(&deep, None), Some(inner.join(".ph")));
}

#[test]
fn find_local_ignores_ph_file_that_is_not_a_directory() {
    let d = tempfile::tempdir().unwrap();
    let proj = d.path().join("proj");
    mk(&proj);
    std::fs::write(proj.join(".ph"), "file").unwrap();
    assert_eq!(find_local(&proj, None), None);
}

#[test]
fn find_local_does_not_recognize_home_dot_ph_even_when_cwd_is_home() {
    let d = tempfile::tempdir().unwrap();
    let home = d.path().join("home");
    mk(&home.join(".ph"));
    assert_eq!(find_local(&home, Some(&home)), None);
    let sub = home.join("sub");
    mk(&sub);
    assert_eq!(find_local(&sub, Some(&home)), None);
}

#[test]
fn find_local_stops_at_home_and_does_not_look_above_it() {
    let d = tempfile::tempdir().unwrap();
    let above = d.path().join("above");
    let home = above.join("home");
    let sub = home.join("sub");
    mk(&sub);
    mk(&above.join(".ph")); // 홈 위의 .ph 는 닿지 않는다.
    assert_eq!(find_local(&sub, Some(&home)), None);
}

#[test]
fn find_local_project_under_home_wins_over_home() {
    let d = tempfile::tempdir().unwrap();
    let home = d.path().join("home");
    let proj = home.join("work").join("proj");
    let deep = proj.join("src");
    mk(&deep);
    mk(&home.join(".ph"));
    mk(&proj.join(".ph"));
    assert_eq!(find_local(&deep, Some(&home)), Some(proj.join(".ph")));
}

#[test]
fn find_local_does_not_panic_at_filesystem_root_or_relative_path() {
    let root = if cfg!(windows) {
        Path::new("C:\\")
    } else {
        Path::new("/")
    };
    let _ = find_local(root, None);
    let _ = find_local(Path::new(""), None);
    let _ = find_local(Path::new("relative/nonexistent"), None);
}
