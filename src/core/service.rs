//! `PromptService`: scope 병합, local 우선, 쓰기 대상 결정 (SPEC 3.2절).
//! cli 와 tui 는 이 규칙을 다시 구현하지 않는다.

use super::clock::Clock;
use super::error::PhError;
use super::format;
use super::model::{NewPrompt, Prompt, PromptId, PromptPatch, Scope};
use super::storage::{SkippedEntry, Storage};

/// 읽기 대상 scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeFilter {
    /// local 과 global 병합
    All,
    /// 한 scope 만
    Only(Scope),
}

/// 쓰기 대상.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteTarget {
    /// local 이 있으면 local, 없으면 global
    Auto,
    /// 명시된 scope
    Explicit(Scope),
}

/// 목록 항목.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// prompt
    pub prompt: Prompt,
    /// local 에 같은 id 가 있어 가려진 global 항목이면 `true`
    pub shadowed: bool,
}

/// 건너뛴 항목과 그 scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedSkip {
    /// 항목이 있던 scope
    pub scope: Scope,
    /// 건너뛴 항목
    pub entry: SkippedEntry,
}

/// `list` 결과.
#[derive(Debug, Default)]
pub struct ListResult {
    /// 병합된 항목 (id 대소문자 무시 순, 같으면 local 먼저)
    pub entries: Vec<Entry>,
    /// 읽지 못한 파일 경고
    pub skipped: Vec<ScopedSkip>,
}

/// `get` 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// 선택된 prompt (양쪽에 있으면 local)
    pub prompt: Prompt,
    /// 양쪽 scope 에 같은 id 의 정상 항목이 있으면 `true`
    pub ambiguous: bool,
    /// 우선순위상 앞 scope 의 파일이 깨져 뒤 scope 항목으로 대체되었으면 채운다 (`get` 만)
    pub fallback: Option<BrokenFallback>,
}

/// 깨진 파일 때문에 다른 scope 항목으로 대체되었다는 정보.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokenFallback {
    /// 깨진 파일이 있는 scope
    pub broken_scope: Scope,
    /// `InvalidFormat` 의 설명 (호출자가 stderr 경고에 쓴다)
    pub reason: String,
}

/// scope 하나에서 id 를 찾은 결과.
enum Lookup {
    Found(Prompt),
    /// 파일은 있으나 읽을 수 없다 (`InvalidFormat`)
    Broken(PhError),
    Missing,
}

/// 쓰기 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    /// 저장된 prompt
    pub prompt: Prompt,
    /// 저장된 scope
    pub scope: Scope,
    /// `Auto` 로 scope 가 선택되었으면 `true` (호출자가 stderr 로 알린다)
    pub auto_selected: bool,
    /// 호출자가 사용자에게 알릴 부가 정보
    pub notes: Vec<WriteNote>,
}

/// 쓰기 결과에 붙는 부가 정보.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteNote {
    /// 편집한 frontmatter 에 `id` 키가 있었으나 무시했다 (id 는 생성 후 고정).
    IdKeyIgnored,
}

/// prompt CRUD 진입점.
pub struct PromptService {
    global: Box<dyn Storage>,
    local: Option<Box<dyn Storage>>,
    clock: Box<dyn Clock>,
}

impl PromptService {
    /// global 은 필수, local 은 선택이다.
    pub fn new(
        global: Box<dyn Storage>,
        local: Option<Box<dyn Storage>>,
        clock: Box<dyn Clock>,
    ) -> Self {
        Self {
            global,
            local,
            clock,
        }
    }

    /// local 저장소가 있는지.
    pub fn has_local(&self) -> bool {
        self.local.is_some()
    }

    /// local 저장소 위치 (없으면 `None`).
    pub fn local_location(&self) -> Option<String> {
        self.local.as_ref().map(|s| s.location())
    }

    /// global 저장소 위치.
    pub fn global_location(&self) -> String {
        self.global.location()
    }

    fn storage(&self, scope: Scope) -> Result<&dyn Storage, PhError> {
        match scope {
            Scope::Global => Ok(self.global.as_ref()),
            Scope::Local => self.local.as_deref().ok_or(PhError::LocalNotInitialized),
        }
    }

    /// 필터가 가리키는 scope 들 (local 먼저). `Only(Local)` 인데 local 이 없으면 에러다.
    fn scopes(&self, filter: ScopeFilter) -> Result<Vec<Scope>, PhError> {
        match filter {
            ScopeFilter::All if self.has_local() => Ok(vec![Scope::Local, Scope::Global]),
            ScopeFilter::All => Ok(vec![Scope::Global]),
            ScopeFilter::Only(s) => {
                self.storage(s)?;
                Ok(vec![s])
            }
        }
    }

    /// 병합 목록. `tag` 는 대소문자 무시 완전 일치다.
    pub fn list(&self, filter: ScopeFilter, tag: Option<&str>) -> Result<ListResult, PhError> {
        self.collect(filter, |p| matches_tag(p, tag))
    }

    /// 검색. id, title, description, tags, body 에서 대소문자 무시 부분 일치를 찾는다.
    pub fn search(
        &self,
        query: &str,
        filter: ScopeFilter,
        tag: Option<&str>,
    ) -> Result<Vec<Entry>, PhError> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Err(PhError::Usage(
                "검색어가 비어 있습니다. 찾을 단어를 입력하세요".to_string(),
            ));
        }
        Ok(self
            .collect(filter, |p| matches_tag(p, tag) && matches_query(p, &q))?
            .entries)
    }

    fn collect(
        &self,
        filter: ScopeFilter,
        keep: impl Fn(&Prompt) -> bool,
    ) -> Result<ListResult, PhError> {
        let mut result = ListResult::default();
        let mut local_keys = std::collections::HashSet::new();
        for scope in self.scopes(filter)? {
            let listing = self.storage(scope)?.list()?;
            for entry in listing.skipped {
                result.skipped.push(ScopedSkip { scope, entry });
            }
            for prompt in listing.prompts {
                let key = prompt.id.fold_key();
                // 가림 여부는 tag/query 필터와 무관하게 판단한다.
                let shadowed = scope == Scope::Global && local_keys.contains(&key);
                if scope == Scope::Local {
                    local_keys.insert(key);
                }
                if keep(&prompt) {
                    result.entries.push(Entry { prompt, shadowed });
                }
            }
        }
        result.entries.sort_by(|a, b| {
            a.prompt
                .id
                .fold_key()
                .cmp(&b.prompt.id.fold_key())
                .then_with(|| scope_rank(a.prompt.scope).cmp(&scope_rank(b.prompt.scope)))
        });
        Ok(result)
    }

    /// 읽기용 조회 (대소문자 무시). 우선순위(local → global)상 첫 정상 항목을 돌려준다.
    /// 앞 scope 의 파일이 깨져 있으면 뒤 scope 의 정상 항목으로 대체하고 `fallback` 에 알린다.
    /// 뒤 scope 의 깨진 파일은 무시한다. 정상 항목이 없고 깨진 것이 있으면 `InvalidFormat` 이다.
    pub fn get(&self, id: &str, filter: ScopeFilter) -> Result<Resolved, PhError> {
        let pid = PromptId::parse(id)?;
        let mut found: Vec<Prompt> = Vec::new();
        let mut first_broken: Option<(Scope, PhError)> = None;
        let mut broken_before_found: Option<(Scope, String)> = None;
        for scope in self.scopes(filter)? {
            match self.lookup(scope, &pid)? {
                Lookup::Found(p) => {
                    if found.is_empty() {
                        if let Some((s, e)) = &first_broken {
                            broken_before_found = Some((*s, e.to_string()));
                        }
                    }
                    found.push(p);
                }
                Lookup::Broken(e) => {
                    if first_broken.is_none() {
                        first_broken = Some((scope, e));
                    }
                }
                Lookup::Missing => {}
            }
        }
        let ambiguous = found.len() > 1;
        match found.into_iter().next() {
            Some(prompt) => Ok(Resolved {
                prompt,
                ambiguous,
                fallback: broken_before_found.map(|(broken_scope, reason)| BrokenFallback {
                    broken_scope,
                    reason,
                }),
            }),
            None => match first_broken {
                Some((_, e)) => Err(e),
                None => Err(PhError::NotFound { id: id.to_string() }),
            },
        }
    }

    /// 쓰기 대상 해석용 조회 (엄격). 우선순위상 가장 앞에 **존재하는** 항목이 대상이고,
    /// 그것이 깨졌으면 뒤 scope 가 정상이어도 `InvalidFormat` 이다. `fallback` 은 항상 `None`.
    pub fn target(&self, id: &str, filter: ScopeFilter) -> Result<Resolved, PhError> {
        let pid = PromptId::parse(id)?;
        let mut chosen: Option<Prompt> = None;
        let mut ambiguous = false;
        for scope in self.scopes(filter)? {
            match self.lookup(scope, &pid)? {
                Lookup::Found(p) => {
                    if chosen.is_some() {
                        ambiguous = true;
                    } else {
                        chosen = Some(p);
                    }
                }
                // 대상이 정해진 뒤의 깨진 파일은 무시한다.
                Lookup::Broken(e) if chosen.is_none() => return Err(e),
                Lookup::Broken(_) | Lookup::Missing => {}
            }
        }
        match chosen {
            Some(prompt) => Ok(Resolved {
                prompt,
                ambiguous,
                fallback: None,
            }),
            None => Err(PhError::NotFound { id: id.to_string() }),
        }
    }

    /// 한 scope 에서 id(대소문자 무시)로 찾는다. 목록에 없으면 `Storage::get` 을 한 번 더 불러
    /// 깨진 파일(`Broken`)과 없는 파일(`Missing`)을 구분한다. IO 오류는 전파한다.
    fn lookup(&self, scope: Scope, id: &PromptId) -> Result<Lookup, PhError> {
        let storage = self.storage(scope)?;
        let key = id.fold_key();
        if let Some(p) = storage
            .list()?
            .prompts
            .into_iter()
            .find(|p| p.id.fold_key() == key)
        {
            return Ok(Lookup::Found(p));
        }
        match storage.get(id) {
            Ok(Some(p)) => Ok(Lookup::Found(p)),
            Ok(None) => Ok(Lookup::Missing),
            Err(PhError::InvalidFormat {
                id: eid,
                line,
                reason,
            }) => Ok(Lookup::Broken(PhError::InvalidFormat {
                id: eid,
                line,
                reason: format!(
                    "{reason}. 파일을 직접 고치거나 지우세요 ({} {})",
                    badge(scope),
                    storage.location()
                ),
            })),
            Err(e) => Err(e),
        }
    }

    /// 새 prompt 를 추가한다. 같은 scope 에 id 가 있으면 `-2`, `-3` … 을 붙여 피한다.
    pub fn add(&self, new: NewPrompt, target: WriteTarget) -> Result<Written, PhError> {
        let (scope, auto_selected) = match target {
            WriteTarget::Auto if self.has_local() => (Scope::Local, true),
            WriteTarget::Auto => (Scope::Global, true),
            WriteTarget::Explicit(s) => (s, false),
        };
        let storage = self.storage(scope)?;
        let title = new.title.trim().to_string();
        let base = PromptId::from_title(&title)?;
        // 깨져서 건너뛴 파일의 이름도 사용 중으로 본다 (덮어쓰기 방지).
        let listing = storage.list()?;
        let mut taken: std::collections::HashSet<String> =
            listing.prompts.iter().map(|p| p.id.fold_key()).collect();
        for s in &listing.skipped {
            let stem = s.name.strip_suffix(".md").unwrap_or(&s.name);
            let nfc: String = unicode_normalization::UnicodeNormalization::nfc(stem).collect();
            taken.insert(nfc.to_lowercase());
        }
        let mut id = base.clone();
        let mut n = 2u32;
        while taken.contains(&id.fold_key()) {
            id = base.with_suffix(n)?;
            n += 1;
        }
        let now = self.clock.now();
        let prompt = Prompt {
            id,
            scope,
            title,
            body: new.body,
            tags: clean_tags(new.tags),
            description: clean_desc(new.description),
            created_at: now,
            updated_at: now,
        };
        storage.put(&prompt)?;
        Ok(Written {
            prompt,
            scope,
            auto_selected,
            notes: Vec::new(),
        })
    }

    /// 부분 수정. 대상은 `target` 규칙(local 우선, 깨진 파일은 대체하지 않음)으로 찾고 `updated_at` 을 갱신한다.
    /// id 는 title 이 바뀌어도 유지한다.
    pub fn update(
        &self,
        id: &str,
        patch: PromptPatch,
        filter: ScopeFilter,
    ) -> Result<Written, PhError> {
        let mut prompt = self.target(id, filter)?.prompt;
        if let Some(t) = patch.title {
            prompt.title = require_title(&t)?;
        }
        if let Some(b) = patch.body {
            prompt.body = b;
        }
        if let Some(t) = patch.tags {
            prompt.tags = clean_tags(t);
        }
        if let Some(d) = patch.description {
            prompt.description = clean_desc(d);
        }
        prompt.updated_at = self.clock.now();
        self.write_back(prompt)
    }

    /// 삭제. 지운 scope 를 돌려준다.
    pub fn remove(&self, id: &str, filter: ScopeFilter) -> Result<Scope, PhError> {
        let p = self.target(id, filter)?.prompt;
        self.storage(p.scope)?.delete(&p.id)?;
        Ok(p.scope)
    }

    /// scope 를 옮긴다. 대상 scope 에 같은 id(대소문자 무시)가 있으면 `AlreadyExists`.
    /// 시각은 바꾸지 않는다.
    pub fn move_to(&self, id: &str, to: Scope) -> Result<Written, PhError> {
        let dest = self.storage(to)?;
        let from = to.other();
        let pid = PromptId::parse(id)?;
        let mut prompt = match self.lookup(from, &pid)? {
            Lookup::Found(p) => p,
            Lookup::Broken(e) => return Err(e),
            Lookup::Missing => return Err(PhError::NotFound { id: id.to_string() }),
        };
        match self.lookup(to, &pid)? {
            Lookup::Found(_) => {
                return Err(PhError::AlreadyExists {
                    id: prompt.id.as_str().to_string(),
                })
            }
            // 깨진 파일을 덮어쓰지 않는다.
            Lookup::Broken(e) => return Err(e),
            Lookup::Missing => {}
        }
        prompt.scope = to;
        dest.put(&prompt)?;
        let source_id = prompt.id.clone();
        self.storage(from)?.delete(&source_id)?;
        Ok(Written {
            prompt,
            scope: to,
            auto_selected: false,
            notes: Vec::new(),
        })
    }

    /// 외부 에디터용 전체 텍스트 (frontmatter 포함).
    pub fn export_raw(&self, id: &str, filter: ScopeFilter) -> Result<String, PhError> {
        format::serialize(&self.target(id, filter)?.prompt)
    }

    /// 편집된 전체 텍스트를 검증해 저장한다. 실패하면 원본은 바뀌지 않는다.
    /// id 와 `created_at` 은 유지하고 `updated_at` 을 갱신한다.
    pub fn save_raw(&self, id: &str, raw: &str, filter: ScopeFilter) -> Result<Written, PhError> {
        let old = self.target(id, filter)?.prompt;
        let (mut prompt, id_key) = format::parse_detailed(raw, &old.id, old.scope)?;
        prompt.title = require_title(&prompt.title)?;
        prompt.tags = clean_tags(prompt.tags);
        prompt.description = clean_desc(prompt.description);
        prompt.created_at = old.created_at;
        prompt.updated_at = self.clock.now();
        let mut written = self.write_back(prompt)?;
        if id_key {
            written.notes.push(WriteNote::IdKeyIgnored);
        }
        Ok(written)
    }

    fn write_back(&self, prompt: Prompt) -> Result<Written, PhError> {
        let scope = prompt.scope;
        self.storage(scope)?.put(&prompt)?;
        Ok(Written {
            prompt,
            scope,
            auto_selected: false,
            notes: Vec::new(),
        })
    }
}

fn badge(s: Scope) -> &'static str {
    match s {
        Scope::Local => "[L]",
        Scope::Global => "[G]",
    }
}

fn scope_rank(s: Scope) -> u8 {
    match s {
        Scope::Local => 0,
        Scope::Global => 1,
    }
}

fn require_title(t: &str) -> Result<String, PhError> {
    let t = t.trim();
    if t.contains(['\n', '\r']) {
        return Err(PhError::Usage(
            "title 에는 줄바꿈을 쓸 수 없습니다. 한 줄로 입력하세요".to_string(),
        ));
    }
    if t.is_empty() {
        return Err(PhError::Usage(
            "title 이 비어 있습니다. 표시할 제목을 입력하세요".to_string(),
        ));
    }
    Ok(t.to_string())
}

/// 앞뒤 공백을 지우고 빈 태그와 중복(완전 일치)을 뺀다.
fn clean_tags(tags: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in tags {
        let t = t.trim().to_string();
        if !t.is_empty() && !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

fn clean_desc(d: Option<String>) -> Option<String> {
    d.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn matches_tag(p: &Prompt, tag: Option<&str>) -> bool {
    match tag {
        None => true,
        Some(t) => p
            .tags
            .iter()
            .any(|x| x.to_lowercase() == t.trim().to_lowercase()),
    }
}

fn matches_query(p: &Prompt, q_lower: &str) -> bool {
    let has = |s: &str| s.to_lowercase().contains(q_lower);
    has(p.id.as_str())
        || has(&p.title)
        || has(&p.body)
        || p.description.as_deref().is_some_and(has)
        || p.tags.iter().any(|t| has(t))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::clock::FixedClock;
    use crate::storage::MemoryStorage;
    use chrono::DateTime;

    fn clock(s: &str) -> Box<dyn Clock> {
        Box::new(FixedClock(DateTime::parse_from_rfc3339(s).unwrap()))
    }

    fn svc(with_local: bool) -> PromptService {
        PromptService::new(
            Box::new(MemoryStorage::new(Scope::Global)),
            with_local.then(|| Box::new(MemoryStorage::new(Scope::Local)) as Box<dyn Storage>),
            clock("2026-09-30T12:00:00+09:00"),
        )
    }

    fn new(title: &str) -> NewPrompt {
        NewPrompt {
            title: title.into(),
            body: format!("{title} 본문"),
            ..Default::default()
        }
    }

    #[test]
    fn auto_target_prefers_local_then_global() {
        let w = svc(true).add(new("코드 리뷰"), WriteTarget::Auto).unwrap();
        assert_eq!((w.scope, w.auto_selected), (Scope::Local, true));
        let w = svc(false).add(new("코드 리뷰"), WriteTarget::Auto).unwrap();
        assert_eq!(w.scope, Scope::Global);
    }

    #[test]
    fn explicit_local_without_local_errors() {
        let r = svc(false).add(new("a"), WriteTarget::Explicit(Scope::Local));
        assert!(matches!(r, Err(PhError::LocalNotInitialized)));
        let r = svc(false).list(ScopeFilter::Only(Scope::Local), None);
        assert!(matches!(r, Err(PhError::LocalNotInitialized)));
    }

    #[test]
    fn duplicate_ids_get_suffix_case_insensitively() {
        let s = svc(false);
        s.add(new("Code Review"), WriteTarget::Auto).unwrap();
        let w = s.add(new("code review"), WriteTarget::Auto).unwrap();
        assert_eq!(w.prompt.id.as_str(), "code-review-2");
        let w = s.add(new("CODE REVIEW"), WriteTarget::Auto).unwrap();
        assert_eq!(w.prompt.id.as_str(), "CODE-REVIEW-3");
    }

    #[test]
    fn merge_local_wins_and_marks_shadowed() {
        let s = svc(true);
        s.add(new("공통"), WriteTarget::Explicit(Scope::Global))
            .unwrap();
        s.add(new("공통"), WriteTarget::Explicit(Scope::Local))
            .unwrap();
        s.add(new("전역만"), WriteTarget::Explicit(Scope::Global))
            .unwrap();

        let l = s.list(ScopeFilter::All, None).unwrap();
        assert_eq!(l.entries.len(), 3);
        let shadowed: Vec<_> = l.entries.iter().filter(|e| e.shadowed).collect();
        assert_eq!(shadowed.len(), 1);
        assert_eq!(shadowed[0].prompt.scope, Scope::Global);

        let r = s.get("공통", ScopeFilter::All).unwrap();
        assert_eq!(r.prompt.scope, Scope::Local);
        assert!(r.ambiguous);
        let r = s.get("공통", ScopeFilter::Only(Scope::Global)).unwrap();
        assert_eq!(r.prompt.scope, Scope::Global);
        assert!(!r.ambiguous);
        assert!(!s.get("전역만", ScopeFilter::All).unwrap().ambiguous);
    }

    #[test]
    fn get_is_case_insensitive_and_not_found() {
        let s = svc(false);
        s.add(new("Code Review"), WriteTarget::Auto).unwrap();
        assert!(s.get("code-review", ScopeFilter::All).is_ok());
        assert!(matches!(
            s.get("none", ScopeFilter::All),
            Err(PhError::NotFound { .. })
        ));
    }

    #[test]
    fn update_keeps_id_and_created_at() {
        let s = svc(false);
        let w = s.add(new("원래"), WriteTarget::Auto).unwrap();
        let u = s
            .update(
                "원래",
                PromptPatch {
                    title: Some("바뀐 제목".into()),
                    tags: Some(vec![" a ".into(), "a".into(), "".into()]),
                    ..Default::default()
                },
                ScopeFilter::All,
            )
            .unwrap();
        assert_eq!(u.prompt.id, w.prompt.id);
        assert_eq!(u.prompt.title, "바뀐 제목");
        assert_eq!(u.prompt.tags, vec!["a"]);
        assert_eq!(u.prompt.created_at, w.prompt.created_at);
    }

    #[test]
    fn remove_and_move() {
        let s = svc(true);
        s.add(new("x"), WriteTarget::Explicit(Scope::Global))
            .unwrap();
        let w = s.move_to("x", Scope::Local).unwrap();
        assert_eq!(w.scope, Scope::Local);
        assert!(matches!(
            s.get("x", ScopeFilter::Only(Scope::Global)),
            Err(PhError::NotFound { .. })
        ));
        s.add(new("X"), WriteTarget::Explicit(Scope::Global))
            .unwrap();
        assert!(matches!(
            s.move_to("X", Scope::Local),
            Err(PhError::AlreadyExists { .. })
        ));
        assert_eq!(s.remove("x", ScopeFilter::All).unwrap(), Scope::Local);
    }

    #[test]
    fn search_and_tag_filter() {
        let s = svc(false);
        s.add(
            NewPrompt {
                title: "리뷰".into(),
                body: "Rust 코드".into(),
                tags: vec!["Review".into()],
                description: None,
            },
            WriteTarget::Auto,
        )
        .unwrap();
        s.add(new("기타"), WriteTarget::Auto).unwrap();
        assert_eq!(s.search("rust", ScopeFilter::All, None).unwrap().len(), 1);
        assert_eq!(
            s.list(ScopeFilter::All, Some("review"))
                .unwrap()
                .entries
                .len(),
            1
        );
        assert!(matches!(
            s.search("  ", ScopeFilter::All, None),
            Err(PhError::Usage(_))
        ));
    }

    #[test]
    fn save_raw_validates_and_preserves_original() {
        let s = svc(false);
        s.add(new("문서"), WriteTarget::Auto).unwrap();
        let raw = s.export_raw("문서", ScopeFilter::All).unwrap();
        assert!(s.save_raw("문서", "garbage", ScopeFilter::All).is_err());
        assert_eq!(s.export_raw("문서", ScopeFilter::All).unwrap(), raw);
        let edited = raw.replace("문서 본문", "새 본문");
        let w = s.save_raw("문서", &edited, ScopeFilter::All).unwrap();
        assert_eq!(w.prompt.body, "새 본문");
        assert_eq!(w.prompt.id.as_str(), "문서");
    }

    /// 한쪽 scope 에 깨진 파일을 흉내 내는 저장소: `get` 이 `InvalidFormat`, `list` 는 skipped.
    struct BrokenStorage {
        scope: Scope,
        inner: MemoryStorage,
        broken: &'static str,
    }

    impl Storage for BrokenStorage {
        fn scope(&self) -> Scope {
            self.scope
        }
        fn location(&self) -> String {
            "broken".into()
        }
        fn list(&self) -> Result<crate::core::storage::Listing, PhError> {
            let mut l = self.inner.list()?;
            l.skipped.push(SkippedEntry {
                name: format!("{}.md", self.broken),
                reason: "bad".into(),
            });
            Ok(l)
        }
        fn get(&self, id: &PromptId) -> Result<Option<Prompt>, PhError> {
            if id.as_str() == self.broken {
                return Err(PhError::InvalidFormat {
                    id: Some(self.broken.into()),
                    line: Some(1),
                    reason: "bad".into(),
                });
            }
            self.inner.get(id)
        }
        fn put(&self, p: &Prompt) -> Result<(), PhError> {
            self.inner.put(p)
        }
        fn delete(&self, id: &PromptId) -> Result<(), PhError> {
            self.inner.delete(id)
        }
    }

    fn broken_svc(broken_local: bool, broken_global: bool) -> PromptService {
        let mk = |scope, broken: bool| -> Box<dyn Storage> {
            let inner = MemoryStorage::new(scope);
            if broken {
                Box::new(BrokenStorage {
                    scope,
                    inner,
                    broken: "x",
                })
            } else {
                Box::new(inner)
            }
        };
        PromptService::new(
            mk(Scope::Global, broken_global),
            Some(mk(Scope::Local, broken_local)),
            clock("2026-09-30T12:00:00+09:00"),
        )
    }

    #[test]
    fn broken_rules_r1_r2_r3_r6() {
        // R1/R6: 정상 항목이 없다
        for (l, g) in [(true, false), (false, true), (true, true)] {
            let s = broken_svc(l, g);
            assert!(matches!(
                s.get("x", ScopeFilter::All),
                Err(PhError::InvalidFormat { .. })
            ));
            assert!(matches!(
                s.target("x", ScopeFilter::All),
                Err(PhError::InvalidFormat { .. })
            ));
        }
        // R2: local 정상 + global 깨짐
        let s = broken_svc(false, true);
        s.add(new("x"), WriteTarget::Explicit(Scope::Local))
            .unwrap();
        let r = s.get("x", ScopeFilter::All).unwrap();
        assert_eq!((r.prompt.scope, r.fallback), (Scope::Local, None));
        assert!(s.target("x", ScopeFilter::All).is_ok());
        // R3: local 깨짐 + global 정상
        let s = broken_svc(true, false);
        s.add(new("x"), WriteTarget::Explicit(Scope::Global))
            .unwrap();
        let r = s.get("x", ScopeFilter::All).unwrap();
        assert_eq!(r.prompt.scope, Scope::Global);
        assert_eq!(r.fallback.unwrap().broken_scope, Scope::Local);
        assert!(!r.ambiguous);
        assert!(matches!(
            s.remove("x", ScopeFilter::All),
            Err(PhError::InvalidFormat { .. })
        ));
        assert!(matches!(
            s.get("x", ScopeFilter::Only(Scope::Local)),
            Err(PhError::InvalidFormat { .. })
        ));
        assert_eq!(
            s.remove("x", ScopeFilter::Only(Scope::Global)).unwrap(),
            Scope::Global
        );
    }

    #[test]
    fn move_and_add_respect_broken_files() {
        let s = broken_svc(true, false);
        s.add(new("x"), WriteTarget::Explicit(Scope::Global))
            .unwrap();
        // 대상(local)에 깨진 같은 id
        assert!(matches!(
            s.move_to("x", Scope::Local),
            Err(PhError::InvalidFormat { .. })
        ));
        assert!(s.get("x", ScopeFilter::Only(Scope::Global)).is_ok());
        // 깨진 파일 이름은 add 가 피한다
        let w = s
            .add(new("x"), WriteTarget::Explicit(Scope::Local))
            .unwrap();
        assert_eq!(w.prompt.id.as_str(), "x-2");
    }
}
