# ph 아키텍처

> 작성: software-architect / 기준: [SPEC.md](../SPEC.md) 3, 4, 5, 6, 8절 / 상태: M0 확정 + M1(CLI) 설계 추가, 사용자 결정 반영 (SPEC 변경 승인분)
> SPEC 과 충돌하는 내용은 없다. SPEC 트리에 없는 파일 추가는 "SPEC 대비 추가 사항"에 따로 적었다.
> 구현(M0)과 이 문서가 다른 곳은 **구현에 맞춰 이 문서를 고쳤다** (3.5절 등).

## 1. 원칙

1. **단일 crate.** 모듈로 계층을 나눈다. workspace 분리는 컴파일 시간이나 재사용 요구가 실제로 생길 때 한다.
2. **동기 IO.** async 런타임을 넣지 않는다. 저장소는 로컬 파일이고 TUI 는 crossterm 이벤트 루프면 충분하다.
3. **core 는 순수하다.** 파일, 터미널, 환경변수, 현재 시각을 직접 만지지 않는다. 필요한 것은 trait 로 주입받는다 (`Storage`, `Clock`).
4. **규칙은 core 에 한 번만 쓴다.** id 검증, scope 병합과 우선순위, 쓰기 대상 결정, frontmatter 파싱, 변수 치환은 모두 `core` 에 있다. cli 와 tui 는 `PromptService` 를 호출해 결과를 표시만 한다.
5. **OS 종속은 `platform/` 에만 둔다.** SPEC 6절 이식성 규칙을 리뷰 기준으로 삼는다.

## 2. 모듈 구조와 의존 방향

```
src/
├─ main.rs            # 인자 파싱, 조립(composition root), 종료 코드 변환
├─ bootstrap.rs       # Runtime(=service + 경로) 조립, Clock 어댑터. main 만 호출하고 완성된 PromptService 를 cli/tui 에 넘긴다
├─ core/
│  ├─ mod.rs
│  ├─ model.rs        # Prompt, PromptId, Scope, NewPrompt, PromptPatch
│  ├─ id.rs           # id 생성/검증 (SPEC 2절 id 규칙, NFC, 예약어, 100바이트)
│  ├─ format.rs       # 파일 텍스트 <-> Prompt (TOML frontmatter, CRLF/BOM 허용, LF 출력). 순수 함수
│  ├─ template.rs     # {{var}} 파서, 치환 (M3)
│  ├─ storage.rs      # Storage trait (구현체 없음)
│  ├─ clock.rs        # Clock trait
│  ├─ service.rs      # PromptService
│  └─ error.rs        # PhError
├─ storage/
│  ├─ fs.rs           # FsStorage (디렉터리 하나 = scope 하나), local 탐색
│  └─ memory.rs       # MemoryStorage (테스트, 공개 모듈)
├─ platform/
│  ├─ env.rs          # cwd(), get(key): std::env 래퍼. main/bootstrap 은 std::env 를 직접 쓰지 않는다
│  ├─ paths.rs        # global 데이터 디렉터리, 홈 디렉터리 (XDG_*, HOME 은 여기서만 읽음)
│  ├─ fs.rs           # atomic_write
│  ├─ editor.rs       # 외부 에디터 실행 ($VISUAL → $EDITOR → vi)
│  ├─ time.rs         # now_local() 함수 (Clock 어댑터는 bootstrap 에 있다)
│  ├─ terminal.rs     # ColorMode, detect_color_mode(env) — COLORTERM/NO_COLOR 는 여기서만 (M2)
│  └─ clipboard.rs    # Clipboard trait + arboard 구현 (M2)
├─ cli/               # clap 정의, 핸들러, 출력(text/json), 종료 코드 매핑 (세부는 10절)
├─ tui/               # AppState, event, update, view, theme, editor(인라인)
└─ skill/             # SKILL.md 템플릿 생성/설치
tests/                # CLI 통합 테스트
```

### 의존 그래프

```
            main ──► bootstrap
             │          │
     ┌───────┴──┐       │ (조립: 구현체를 service 에 주입)
     ▼          ▼       ▼
    cli        tui ──► core ◄── storage
     │  │       │       ▲          │
     │  └► skill┴───────┘          │
     └────────┬─────────────────────┘
              ▼
           platform      (core 를 참조하지 않는다)
```

허용되는 `use` 관계 (이 표에 없으면 금지):

| 모듈 | 참조 가능 |
|---|---|
| `core` | `std`, `serde`, `toml`, `unicode-*`, `chrono`(타입만), `thiserror` |
| `platform` | `std`, OS 관련 crate (`directories`, `arboard`). `core` 금지 |
| `storage` | `core`, `platform` |
| `skill` | `core` (본문 생성만. 파일 쓰기는 `platform::fs`) |
| `cli` | `core`, `platform`, `skill`, `clap`, `serde_json`, `storage::fs::init_local` (이 함수 하나만) |
| `tui` | `core`, `platform`, `ratatui`, `crossterm`, `ratatui-textarea` |
| `bootstrap` | `core`, `storage`, `platform` (`Clock` 어댑터 포함) |
| `main` | `cli`, `tui`, `bootstrap` |

- `cli` ↔ `tui` 는 서로 `use` 하지 않는다.
- `core` 에서 `std::fs`, `std::env`, `std::process`, `SystemTime`, `Local::now()` 를 쓰지 않는다. 리뷰와 QA 의 grep 점검 대상이다.
- `core` 가 `chrono` 를 쓰는 것은 `DateTime<FixedOffset>` 타입과 RFC3339 파싱/포맷뿐이다. 현재 시각은 `Clock` 으로만 얻는다.

### SPEC 대비 추가 사항

SPEC 6절 트리에 다음을 더한다. 의존 방향 규칙은 바꾸지 않는다.

- `core/id.rs`, `core/storage.rs`, `core/clock.rs`, `core/format.rs`: `model.rs` 가 커지는 것을 막고 trait 위치를 정하기 위한 분리다.
- `core/format.rs`: SPEC 5.1절의 외부 에디터 흐름은 "frontmatter 포함 전체 텍스트"를 다시 검증해야 한다. tui 와 cli 가 각자 파싱하면 규칙이 갈라지므로 순수 함수로 core 에 둔다. `storage::fs` 도 이것을 쓴다.
- `bootstrap.rs`: 환경변수, cwd, local 탐색으로 `PromptService` 를 조립하는 코드. 이를 cli 와 tui 에 각각 두면 scope 설정이 갈라진다.
- `platform/time.rs`, `platform/clipboard.rs`: 시스템 시각과 클립보드 접근.

## 3. 공개 인터페이스 (시그니처 골격)

구현은 `coder` 가 한다. 아래는 계약이다. 이름을 바꾸려면 이 문서를 먼저 고친다.

### 3.1 model

```rust
pub enum Scope { Local, Global }            // serde: "local" | "global"

/// 검증된 id. 생성자는 SPEC 2절 규칙을 모두 강제한다 (NFC, 금지 문자, 예약어, 100바이트).
pub struct PromptId(String);
impl PromptId {
    pub fn parse(s: &str) -> Result<Self, PhError>;          // 기존 id 검증
    pub fn from_title(title: &str) -> Result<Self, PhError>; // 공백 → '-', 금지 문자는 에러
    pub fn with_suffix(&self, n: u32) -> Result<Self, PhError>; // 중복 회피: "이름-2"
    pub fn as_str(&self) -> &str;
    pub fn fold_key(&self) -> String;                        // 대소문자 무시 비교용 키
}

pub struct Prompt {
    pub id: PromptId,
    pub scope: Scope,                       // 파일에는 없음. Storage 가 채운다
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    pub description: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

pub struct NewPrompt { pub title: String, pub body: String, pub tags: Vec<String>, pub description: Option<String> }
pub struct PromptPatch { pub title: Option<String>, pub body: Option<String>, pub tags: Option<Vec<String>>, pub description: Option<Option<String>> }
```

`--json` 출력 스키마는 `Prompt` 를 그대로 직렬화하지 않고 `cli::output` 의 전용 DTO 로 만든다 (스키마 버전을 core 모델에서 분리하기 위함).

`PromptId::from_title` 은 유니코드 공백을 `-` 로 바꾸고 탭과 개행 같은 제어 문자는 공백으로 보지 않고 에러로 남긴다 (SPEC 2절).

### 3.2 Storage (core 가 정의, storage 가 구현)

하나의 `Storage` 는 **하나의 scope** 를 담당한다. 병합은 service 가 한다.

```rust
pub trait Storage {
    fn scope(&self) -> Scope;
    /// 표시용 위치 (fs: 디렉터리 경로, memory: "memory"). 상태바와 stderr 안내에 쓴다.
    fn location(&self) -> String;
    /// 읽을 수 없는 항목은 실패시키지 않고 skipped 로 돌려준다 (깨진 파일 하나가 전체 list 를 막지 않게).
    fn list(&self) -> Result<Listing, PhError>;
    fn get(&self, id: &PromptId) -> Result<Option<Prompt>, PhError>;   // 없으면 Ok(None)
    fn put(&self, prompt: &Prompt) -> Result<(), PhError>;            // 생성 또는 덮어쓰기. atomic
    fn delete(&self, id: &PromptId) -> Result<(), PhError>;           // 없으면 NotFound
}

pub struct Listing { pub prompts: Vec<Prompt>, pub skipped: Vec<SkippedEntry> }
pub struct SkippedEntry { pub name: String, pub reason: String }
```

- `Storage` 는 `&self` 메서드만 가진다 (`MemoryStorage` 는 내부에서 `RefCell`/`Mutex`). TUI 와 CLI 가 같은 service 를 빌려 쓰기 쉽게 하기 위해서다.
- 대소문자 무시 중복 검사와 id 해석은 `Storage` 가 아니라 service 가 `list()` 결과로 한다. macOS 와 Linux 의 파일시스템 차이에 결과가 좌우되지 않게 하기 위해서다.
- `FsStorage::put` 은 `platform::fs::atomic_write` 로만 쓴다. 직렬화는 `core::format`.
- `list` 는 파일명이 id 규칙을 어기는 파일(공백이 든 손편집 `my prompt.md` 등)과 파싱에 실패한 파일을 `skipped` 로 돌려준다. 이름을 자동으로 고치지 않는다 (SPEC 2절).
- `get` 은 파일이 없으면 `Ok(None)`, **파일은 있는데 깨졌으면 `Err(InvalidFormat)`** 다.

### 3.3 Clock

```rust
pub trait Clock { fn now(&self) -> DateTime<FixedOffset>; }   // 구현: bootstrap 의 SystemClock 어댑터 (platform::time::now_local 호출), 테스트: FixedClock
```

### 3.4 PromptService

```rust
pub struct PromptService { /* global: Box<dyn Storage>, local: Option<Box<dyn Storage>>, clock: Box<dyn Clock> */ }

pub enum ScopeFilter { All, Only(Scope) }          // --local / --global, TUI 의 s 키
pub enum WriteTarget { Auto, Explicit(Scope) }

pub struct Entry { pub prompt: Prompt, pub shadowed: bool }   // shadowed: local 에 가려진 global 항목
pub struct Written { pub prompt: Prompt, pub scope: Scope, pub auto_selected: bool, pub notes: Vec<WriteNote> } // cli 가 stderr 안내에 사용
pub enum WriteNote { IdKeyIgnored }   // save_raw: 편집한 frontmatter 에 `id` 키가 있었으나 무시했다 (SPEC 2절). 표시는 호출자 몫

impl PromptService {
    pub fn new(global: Box<dyn Storage>, local: Option<Box<dyn Storage>>, clock: Box<dyn Clock>) -> Self;

    // 읽기: 병합 + local 우선 (SPEC 3.2)
    pub fn list(&self, filter: ScopeFilter, tag: Option<&str>) -> Result<ListResult, PhError>; // Entry 목록 + skipped 경고
    pub fn search(&self, query: &str, filter: ScopeFilter, tag: Option<&str>) -> Result<Vec<Entry>, PhError>;
    pub fn get(&self, id: &str, filter: ScopeFilter) -> Result<Resolved, PhError>; // Resolved { prompt, ambiguous: bool }

    // 쓰기: 대상 결정은 여기서 (SPEC 3.2)
    pub fn add(&self, new: NewPrompt, target: WriteTarget) -> Result<Written, PhError>;
    pub fn update(&self, id: &str, patch: PromptPatch, filter: ScopeFilter) -> Result<Written, PhError>;
    pub fn remove(&self, id: &str, filter: ScopeFilter) -> Result<Scope, PhError>;
    pub fn move_to(&self, id: &str, to: Scope) -> Result<Written, PhError>;

    // 전체 텍스트 편집 (외부 에디터용). 검증은 core::format 과 PromptId 규칙을 그대로 쓴다.
    pub fn export_raw(&self, id: &str, filter: ScopeFilter) -> Result<String, PhError>;
    pub fn save_raw(&self, id: &str, raw: &str, filter: ScopeFilter) -> Result<Written, PhError>; // 실패 시 원본 불변

    // 편의
    pub fn has_local(&self) -> bool;
    pub fn local_location(&self) -> Option<String>;   // 상태바 (없으면 None → "local 없음")
    pub fn render(&self, id: &str, vars: &Vars, allow_missing: bool, filter: ScopeFilter) -> Result<String, PhError>; // M3
}
```

규칙 구현 위치:

- `--local`(`ScopeFilter::Only(Local)`) 인데 local 이 없으면 **읽기(`list`/`search`/`get`)와 쓰기 모두** `PhError::LocalNotInitialized` (`ph init` 안내 포함). 빈 결과나 NotFound 로 넘기지 않고 global 로도 가지 않는다.
- **알려진 동작 (깨진 파일과 다른 scope 의 정상 항목)**: `find_in` 은 한 scope 의 깨진 파일이 낸 `InvalidFormat` 을 다른 scope 에 같은 id 의 정상 항목이 있어도 **전파한다** (가려진 것처럼 조용히 넘어가지 않는다). 우회는 정상 항목이 있는 쪽을 `--global`/`--local` 로 명시하는 것이다. 의도된 동작으로 유지하고 회귀 테스트로 잠근다.
- **깨진 파일의 `get`**: id 로 찾은 파일이 있는데 파싱에 실패하면 `NotFound` 가 아니라 `InvalidFormat`(id, 줄 번호, 사유)을 돌려준다. 구현: `list()` 결과에 없으면 `Storage::get(&id)` 를 한 번 더 호출해 그 에러를 전파한다. `update`/`remove`/`export_raw`/`save_raw` 도 대상 해석에 `get` 을 쓰므로 같다 (깨진 파일은 손으로 고치거나 지워야 한다는 안내가 메시지에 있어야 한다).
- **TUI 는 항상 scope 를 명시한다 (기본값 채택, 사용자 미확인)**: 편집/삭제/태그/`E` 는 선택 항목의 scope 를 `ScopeFilter::Only(entry.prompt.scope)` 로 넘긴다. 현재 시그니처(`update`, `remove`, `export_raw`, `save_raw` 가 `filter` 를 받는다)로 충분하므로 API 변경은 없다. 그러면 shadowed global 항목을 골라도 그 global 이 대상이다. CLI 는 `--local/--global` 이 없을 때만 `All`(local 우선)을 쓴다. `move_to` 는 원래 scope 를 명시한다.
- `get` 이 양쪽에 있으면 local 을 반환하고 `ambiguous = true`. cli 가 이를 stderr 경고로 바꾼다. 이 동작은 고정이며 테스트로 잠근다.
- `update`/`save_raw`/`add` 는 `updated_at` 을 `Clock` 으로 갱신한다. `created_at` 은 유지한다.
- id 중복은 `fold_key` 로 검사한다. `add` 에서 충돌하면 `with_suffix` 로 자동 회피한다. `update`/`save_raw` 로 title 이 바뀌어도 **id 는 바꾸지 않는다**, 외부 에디터 frontmatter 의 `id` 키는 무시하고 `WriteNote::IdKeyIgnored` 로 알린다 (SPEC 2절. 파일명은 안정적이어야 하고 agent 가 id 로 참조한다). 이름 변경 동작은 v0.1 에 없다.
- `export`/`import` 는 M3 에서 service 에 `export_all`/`import_all` 로 추가한다. JSON 형식은 그때 확정한다.

### 3.5 platform

`platform` 은 `core` 를 모르므로 **`std::io::Result` 를 반환**한다. `PhError` 로 감싸는(context 추가) 일은 호출하는 쪽(`storage`, `cli`, `tui`, `bootstrap`)이 `PhError::io(context, e)` 로 한다. (M0 구현에 맞춰 수정했다.)

```rust
// paths.rs
pub struct Dirs { pub global_prompts: PathBuf, pub home: Option<PathBuf> }
pub fn resolve_dirs(ph_home_override: Option<&Path>) -> io::Result<Dirs>;
//   PH_HOME/--home 이 있으면 그 아래 prompts/, 없으면 OS 데이터 디렉터리. 테스트는 override 로 주입한다.

// fs.rs
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()>;   // 같은 디렉터리에 임시 파일 → rename. 부모 디렉터리를 만든다. LF 통일은 format 이 담당

// editor.rs
pub struct EditorCommand { pub program: OsString, pub args: Vec<OsString> }
pub enum EditorExit { Success, Failed(Option<i32>) }                       // None = 시그널 종료
pub fn resolve_editor(env: &dyn Fn(&str) -> Option<OsString>) -> EditorCommand;   // $VISUAL → $EDITOR → vi. env 주입으로 테스트
pub fn run_editor(cmd: &EditorCommand, file: &Path) -> io::Result<EditorExit>;   // spawn 실패는 Err. 셸 문자열 결합 금지
pub struct TempEditFile { .. }   // 10.6절. create(id, contents) / path() / read(), Drop 시 삭제

// time.rs
pub fn now_local() -> DateTime<FixedOffset>;
//   Clock 어댑터(`struct SystemClock; impl core::Clock`)는 platform 이 core 를 참조하면 안 되므로 bootstrap.rs 에 둔다.

// terminal.rs (M2)
pub enum ColorMode { Mono, Ansi16, TrueColor }
pub fn detect_color_mode(env: &dyn Fn(&str) -> Option<OsString>) -> ColorMode;   // NO_COLOR → COLORTERM → Ansi16 (DESIGN 4.2절)

// clipboard.rs (M2)
pub trait Clipboard { fn set_text(&mut self, text: &str) -> io::Result<()>; }
```

- `$EDITOR="code --wait"` 처럼 인자가 섞인 값은 `shlex` 류 분리 대신 **공백 분리 후 program/args** 로 처리한다 (v0.1). 따옴표가 필요한 경우는 지원하지 않는다고 문서화한다. 셸을 거치지 않는다는 SPEC 규칙은 지킨다.
- TUI 중단과 복구 (raw mode 와 alternate screen 해제/복구)는 `tui` 가 한다. `platform::editor` 는 프로세스 실행만 안다. crossterm 은 크로스플랫폼이라 `tui` 에 두어도 이식성 규칙에 위배되지 않는다.
- local 탐색 알고리즘(상위 디렉터리 탐색, 홈과 루트에서 정지, 홈의 `.ph/` 제외)은 `storage::fs::find_local(cwd, home)` 에 둔다. 인자로 경로를 주입받으므로 실제 cwd 와 무관하게 테스트한다. `cwd` 는 `main` 이 `platform::env::cwd()` 로 읽어 `bootstrap` 에 넘긴다.

## 4. 에러 전략

`core::error::PhError` 하나를 `thiserror` 로 정의한다. `core` 와 `storage` 는 `PhError` 를 반환하고, `platform` 은 `io::Result` 를 반환한다 (호출자가 `PhError::io(context, e)` 로 감싼다). `main` 도 `PhError` 를 직접 처리하므로 `anyhow` 를 도입하지 않는다 (ADR-11).

```rust
#[derive(Debug, thiserror::Error)]
pub enum PhError {
    NotFound { id: String },                          // → 3
    InvalidId { input: String, reason: String },      // → 1
    InvalidFormat { id: Option<String>, line: Option<usize>, reason: String }, // frontmatter 오류. 위치 포함
    AlreadyExists { id: String },                     // → 1
    LocalNotInitialized,                              // → 1 ("ph init 을 먼저 실행하세요")
    MissingVariable { name: String },                 // → 1
    Usage(String),                                    // core 가 검출한 사용법 오류 (예: 필수 조합 누락) → 2
    NonInteractive(String),                           // --yes 필요 → 2
    Io { context: String, #[source] source: std::io::Error }, // → 1
    Editor(String),                                   // 에디터 실행 실패/비정상 종료 → 1
    Clipboard(String),                                // → 1 (TUI 는 상태바에 표시만)
}
```

- 종료 코드 매핑은 **`cli::exit_code(&PhError) -> u8`** 한 곳에 둔다 (`core` 는 종료 코드를 모른다). clap 이 내는 파싱 오류는 기본 종료 코드가 2 라서 SPEC 과 일치한다.
- 메시지는 "무엇이 잘못됐고 다음에 무엇을 하면 되는지"를 담는다 (SPEC 8절). 예: ``local 저장소가 없습니다. `ph init` 으로 만든 뒤 다시 시도하세요``.
- `Io` 는 항상 경로나 동작 `context` 를 붙인다. 맨 `io::Error` 를 그대로 올리지 않는다.
- panic 은 종료 코드 매핑 대상이 아니다. 테스트 밖 `unwrap`/`expect` 금지로 막는다.

## 5. 데이터 흐름

```
CLI:  argv ─clap→ Command ─handler→ PromptService ─→ Storage(fs) ─→ platform::fs
                                        │                              (atomic_write)
                                        └→ Entry/Written ─→ cli::output(text|json) ─→ stdout
                                                          └→ 경고/안내 ─→ stderr

TUI:  crossterm Event ─→ Action ─→ update(&mut AppState, Action, &PromptService) ─→ AppState
                                                                                     │
                                                          view(&AppState, &mut Frame) ◄┘ (렌더링만)
      외부 에디터(E): update 가 Effect::EditExternal(id) 를 반환 → 이벤트 루프가 터미널 중단
                     → service.export_raw → 임시 파일 → platform::editor → service.save_raw
                     → 실패 시 원본 유지 + 오류 위치 표시 + 재편집 확인 → 터미널 복구
```

### TUI 구조 규칙

- `AppState` 는 데이터만 갖고 IO 를 하지 않는다. `update` 는 `(AppState, Action) -> Vec<Effect>` 형태로 만들고, 터미널이나 프로세스를 건드리는 일은 `Effect` 로 이벤트 루프에 넘긴다. 이렇게 하면 `update` 를 터미널 없이 테스트할 수 있다.
- `PromptService` 호출은 `update` 안에서 해도 된다 (메모리 저장소로 테스트). 다만 에디터 실행, 클립보드, 터미널 제어는 `Effect` 로 분리한다.
- `view` 는 `&AppState` 만 읽는다. `TestBackend` 로 검증한다.
- panic hook 은 `tui::terminal` 에서 설치한다.

## 6. 인라인 에디터 crate 선정 (M2 architect 항목)

### 조사 결과 (crates.io, 2026-09-30 확인)

| crate | 최신 | 갱신 | 라이선스 | ratatui | 비고 |
|---|---|---|---|---|---|
| `tui-textarea` | 0.7.0 | 2024-10 | MIT | **0.29** | 사실상 정체. ratatui 0.30 과 함께 쓸 수 없다 (한 빌드에 ratatui 두 버전이 들어가 `Widget` 타입이 어긋남) |
| **`ratatui-textarea`** | 0.9.2 | 2026-06 | MIT | **0.30 (`ratatui-core` 0.1)** | ratatui 조직의 공식 fork. tui-textarea 의 기능을 이어받아 계속 관리된다. MSRV 1.86 |
| `edtui` | 0.11.x | 2026-08 | MIT | 0.30 계열 | vim 모달 편집 중심. 우리 UX (일반 텍스트 입력, `Tab` 필드 이동) 와 맞지 않고 모드 개념이 TUI 의 Normal/Edit 모드와 겹친다 |
| 직접 구현 | - | - | - | - | wide 문자 폭, undo/redo, 선택을 다시 만들어야 한다. 비용 대비 이득이 없다 |

### 결정: `ratatui-textarea` 채택 (SPEC 이 후보로 적은 `tui-textarea` 는 **채택하지 않는다**)

- 최신 ratatui 0.30 과 호환되는 유지되는 crate 이다. `tui-textarea` 는 ratatui 0.29 에 묶여 있다.
- 요구 기능 충족: 여러 줄 편집, 커서 이동, 선택, undo/redo, 단일 줄 입력 (`single_line` 예제, title/tags/description 필드용), soft wrap, 마우스 스크롤.
- 텍스트 폭은 `unicode-width` 와 `unicode-segmentation` (grapheme 단위) 으로 계산하므로 한글 등 wide 문자에 맞다.
- 의존성은 `ratatui-core`, `ratatui-widgets`, `unicode-*` 로 작고, 라이선스는 MIT 다. 검색(regex) 등 불필요한 기능은 feature 를 끈다.
- SPEC 5.1절과 6절 crate 표는 이 결정에 맞춰 정정됐다 (`tui-textarea` → `ratatui-textarea`).

### 사용 방식

- 필드 4개(`title`, `tags`, `description`, `body`)마다 `TextArea` 를 하나씩 둔다. 단일 줄 필드는 Enter 를 무시한다 (또는 `Tab` 과 같게 처리). `Tab`/`Shift+Tab` 은 `AppState` 가 포커스를 옮기고, textarea 에는 넘기지 않는다.
- 우리 키맵(`Ctrl+S` 저장, `Esc` 취소)은 textarea 로 전달하기 **전에** `update` 가 먼저 가로챈다. textarea 의 기본 emacs 키와 충돌하는 키는 `docs/DESIGN.md` 확정 후 조정한다.
- "변경 있음" 판단은 시작 시점의 값과 비교한다 (`lines()` 를 합쳐 비교). textarea 내부 undo 스택에 의존하지 않는다.
- 저장은 반드시 `PromptService::update`/`add` 로 한다. textarea 는 입력 위젯일 뿐 검증 규칙을 갖지 않는다.

### DESIGN 13절 확인 요청에 대한 답

| 항목 | 답 |
|---|---|
| (a) 필드별 undo | 필드마다 `TextArea` 인스턴스를 따로 두므로 undo/redo 이력이 필드 단위다. 포커스를 옮겨도 이력은 유지된다. 요구와 일치한다 |
| (b) 한 줄 필드의 개행 차단 | `update` 가 `Enter` 를 textarea 로 넘기기 전에 가로채 "다음 필드로 이동" 으로 처리한다 (DESIGN 3.4). 붙여넣기 문자열은 넘기기 전에 개행을 공백으로 바꾼다 (DESIGN 8.7) |
| (c) `BorderType::Double` 포커스 | `TextArea::set_block(Block)` 로 우리가 만든 `Block`(테두리 종류, 제목, 스타일)을 그대로 넘길 수 있는 API 다. 충돌하지 않을 것으로 본다. 다만 위젯 API 를 코드로 확인한 것은 아니므로 아래 spike 항목 4 로 둔다 |

### M2 착수 시 coder 가 먼저 확인할 것 (spike)

1. `ratatui 0.30` + `ratatui-textarea 0.9` + `crossterm` 조합에서 `Cargo.lock` 에 ratatui 계열 버전이 중복되지 않는지 (`cargo tree -d`). `ratatui-crossterm` feature 와 우리 `crossterm` 버전 정합성을 포함한다.
2. 한글 입력 (조합 완료된 문자), 한글이 섞인 줄에서 커서 이동과 wrap 이 올바른지. 터미널의 IME 조합 중 표시는 터미널 에뮬레이터 몫이라 범위 밖이다.
3. `set_block` 에 `BorderType::Double` 블록과 제목(`▶ Title`)을 넘겨 그려지는지, 한 줄 필드에서 `Enter` 를 가로챈 뒤 붙여넣기 이벤트(bracketed paste)가 개행을 포함해도 한 줄로 유지되는지, 필드별 undo 가 독립인지.
4. 위 항목이 실패하면 폴백은 `tui-textarea` 가 아니라 (ratatui 0.29 고정이 필요해지므로) **직접 구현** 또는 `edtui` 이다. 이 경우 이 문서를 수정하고 orchestrator 에게 알린다.

## 7. 그 밖의 crate 검토

| 용도 | 결정 | 근거 |
|---|---|---|
| TUI | `ratatui` 0.30, `crossterm` 0.29 | 확정 (SPEC). 둘 다 MIT |
| CLI | `clap` 4 (derive) | 표준. MIT/Apache-2.0 |
| 직렬화 | `serde`, `serde_json`, `toml` 1.x | 모두 활발히 관리됨. MIT/Apache-2.0 |
| 시간 | **`chrono`** (`default-features = false`, `std`, `serde`) | RFC3339 파싱/포맷이 쉽고 `FixedOffset` 으로 오프셋을 보존한다. `time` 은 Unix 에서 로컬 오프셋을 얻는 데 제약이 있다. 시각은 `Clock` 으로만 얻으므로 이후 교체가 쉽다 |
| 경로 | `directories` 6 | MIT/Apache-2.0. **`platform::paths` 안에서만** 쓴다. macOS 는 `Library/Application Support`, Linux 는 XDG 를 따르므로 SPEC 3.1절과 일치한다 |
| 유니코드 | `unicode-normalization` (NFC), `unicode-width` | MIT/Apache-2.0 |
| 클립보드 | `arboard` 3 | MIT/Apache-2.0. `platform::clipboard` 에서만 쓰고 `Clipboard` trait 로 감싼다. 이미지 기능(`image-data`)은 끈다 |
| 에러 | `thiserror` | `anyhow` 는 도입하지 않는다 (ADR-11) |
| 임시 파일 (테스트) | `tempfile` (dev-dependency) | MIT/Apache-2.0. atomic write 구현에도 쓸지는 coder 가 정한다 (직접 구현이 작으면 dev 로만) |

- 모든 후보가 MIT/Apache-2.0 이라 GPL/AGPL 문제는 없다. 새 crate 를 추가할 때는 `cargo tree` 와 라이선스 확인 후 이 표에 한 줄을 추가한다. CI 에 `cargo deny check licenses` 도입은 M0 이후 검토한다.
- frontmatter 는 별도 crate 없이 `+++` 구분을 직접 자르고 `toml` 로 파싱한다. 형식이 단순하고 CRLF/BOM 처리를 통제해야 해서다.
- 테스트를 위한 mock 프레임워크(`mockall` 등)는 쓰지 않는다. in-memory 구현과 `FixedClock` 으로 충분하다.

## 8. Windows 확장성 점검표 (리뷰 기준)

| 규칙 | 이 설계에서의 보장 |
|---|---|
| OS 종속 코드 격리 | `#[cfg(unix)]`, `std::os::unix`, `libc` 는 `platform/` 안에만. PR 리뷰 때 `rg 'cfg\(unix\)|std::os::|libc::' src --glob '!src/platform/**'` 결과가 비어야 한다 |
| 경로 | `PathBuf` 만 사용. `directories` 는 `platform::paths` 안에서만 |
| atomic write | `platform::fs::atomic_write` 하나. Windows 는 이 함수만 교체 |
| 외부 프로그램 | `platform::editor` 만. program/args 분리 |
| CRLF, BOM | `core::format` 이 읽을 때 허용, 쓸 때 LF |
| 잠금/권한 | 사용하지 않는다. 필요해지면 `platform` 에 trait 추가 |
| 환경변수 | `PH_HOME` 은 clap `env` 로, `EDITOR`/`VISUAL` 은 주입된 `env` 클로저로 읽어 `Dirs`, `EditorCommand` 로 값을 넘긴다. `COLORTERM`/`NO_COLOR` 는 `platform::terminal` 에서만 읽고 `ColorMode` 로 전달한다. `XDG_*`, `HOME` 은 `platform::paths` 안에서만 |
| id | `PromptId` 생성자가 예약어, 대소문자, NFC, 바이트 길이를 강제하므로 Unix 에서 만든 저장소가 Windows 에서도 유효하다 |
| 테스트 | 경로 구분자, 개행에 의존하는 assert 금지. `Path` 비교와 `lines()` 를 쓴다 |
| CI | Linux/macOS 실행. Windows 는 `cargo check --target x86_64-pc-windows-msvc` 컴파일 확인만 (M0 이후 추가) |

## 9. Claude skill 과 CLI 의 일관성

- `skill/` 은 SKILL.md 본문을 **템플릿 하나(`include_str!` 또는 상수)** 로 만들고, 설치 경로는 SPEC 7절을 따른다. 파일 쓰기는 `platform::fs::atomic_write`.
- 어긋남을 막는 장치: `cli` 에 테스트 하나를 둔다. 템플릿의 코드 블록에서 `ph ...` 로 시작하는 줄을 추출해 clap `Command` 로 **파싱이 성공하는지** 확인하고, 각 커맨드가 agent 용 규칙(비대화형, 조회 커맨드는 `--json` 지원)을 만족하는지 검사한다. 커맨드나 플래그를 바꾸면 이 테스트가 깨져 skill 갱신을 강제한다.
- TUI 전용 동작(클립보드 복사, 인라인 편집)은 CLI 서브커맨드에 넣지 않는다.
- 세부 검토는 M4 의 architect 항목에서 이어서 한다.

## 10. CLI 설계 (M1)

기준: SPEC 4절, `agent/TASK.md` M1. 이 절은 `coder` 가 바로 구현할 수 있는 수준의 계약이다. 서비스 규칙(병합, 우선순위, 쓰기 대상)은 3.4절의 `PromptService` 에 이미 있으므로 **CLI 핸들러는 인자를 서비스 호출로 옮기고 결과를 출력하는 일만** 한다.

### 10.1 파일 배치

```
src/
├─ main.rs               # 얇게: 인자 파싱 → 환경 수집 → bootstrap → cli::run → 종료 코드
├─ bootstrap.rs          # build_runtime(): 경로, Storage, Clock 조립 (10.3절)
├─ lib.rs                # pub mod bootstrap; (main.rs 가 lib 를 쓴다)
├─ storage/fs.rs         # + init_local(cwd) (10.4절 init)
├─ platform/
│  ├─ editor.rs          # + TempEditFile (10.6절)
│  └─ terminal.rs        # ColorMode, detect_color_mode(env) (M2 에서 사용. M1 에서는 만들지 않아도 된다)
└─ cli/
   ├─ mod.rs             # pub fn run(command, io, service_factory) -> Result<(), PhError>, re-export
   ├─ args.rs            # clap derive 정의 (10.2절). 로직 없음
   ├─ context.rs         # CliIo, EditorLauncher (10.3절)
   ├─ output.rs          # DTO(Serialize), text 렌더러, JSON 쓰기 (10.5절)
   ├─ exit.rs            # exit_code(), error_kind(), report_error() (10.7절)
   └─ commands/
      ├─ mod.rs
      ├─ init.rs  add.rs  get.rs  list.rs  search.rs  rm.rs  edit.rs  move_.rs
```

- 핸들러 파일 하나는 함수 하나 `pub fn run(args: XxxArgs, svc: &PromptService, io: &mut CliIo) -> Result<(), PhError>` 를 공개한다 (`init` 만 `svc` 가 없다).
- 의존: `cli` → `core`, `platform`, `clap`, `serde_json`. `bootstrap`, `storage` 를 import 하지 않는다. `init` 이 필요한 `storage::fs::init_local` 은 예외로 허용한다 (2절 표의 `cli` 행에 `storage::fs::init_local` 한 함수만 추가).
- M1 에서 추가할 의존성: `clap = { version = "4", features = ["derive", "env"] }`, `serde_json = "1"`. `anyhow` 는 도입하지 않는다 (ADR-11).

### 10.2 clap 정의 (`cli/args.rs`)

```rust
#[derive(Debug, Parser)]
#[command(name = "ph", version, about = "Prompt Hub: AI Agent 용 prompt 저장소")]
pub struct Cli {
    /// global 저장소의 기준 디렉터리 (그 아래 prompts/). local 경로에는 영향이 없다.
    #[arg(long, global = true, env = "PH_HOME", value_name = "PATH")]
    pub home: Option<PathBuf>,
    /// 없으면 TUI 를 실행한다.
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// TUI 실행 (인자 없이 `ph` 와 같다)
    Tui,
    /// 현재 디렉터리에 local 저장소(.ph/)를 만든다
    Init(InitArgs),
    Add(AddArgs),
    Get(GetArgs),
    List(ListArgs),
    Search(SearchArgs),
    Edit(EditArgs),
    Rm(RmArgs),
    Move(MoveArgs),
    // M3: Tag, Export, Import   M4: Skill(SkillCommand)
}

/// --local / --global. init, move 를 제외한 저장소 대상 커맨드가 flatten 한다.
#[derive(Debug, Args, Clone, Copy, Default)]
pub struct ScopeArgs {
    #[arg(long, conflicts_with = "global")] pub local: bool,
    #[arg(long)] pub global: bool,
}
impl ScopeArgs {
    /// 읽기와 "읽기 규칙으로 대상을 찾는" 쓰기(edit, rm): 플래그 없으면 All.
    pub fn filter(&self) -> ScopeFilter;
    /// add: 플래그 없으면 Auto.
    pub fn target(&self) -> WriteTarget;
}

#[derive(Debug, Args)]
pub struct InitArgs { #[arg(long)] pub json: bool }

#[derive(Debug, Args)]
#[command(group(ArgGroup::new("source").args(["body", "file", "stdin"]).required(true).multiple(false)))]
pub struct AddArgs {
    pub title: String,
    #[arg(long)] pub body: Option<String>,
    #[arg(long, value_name = "PATH")] pub file: Option<PathBuf>,
    #[arg(long)] pub stdin: bool,
    #[arg(long = "tag", value_name = "TAG")] pub tags: Vec<String>,
    #[arg(long = "desc", value_name = "TEXT")] pub description: Option<String>,
    #[command(flatten)] pub scope: ScopeArgs,
    #[arg(long)] pub json: bool,
}

#[derive(Debug, Args)]
pub struct GetArgs    { pub id: String, #[command(flatten)] pub scope: ScopeArgs, #[arg(long)] pub json: bool }
#[derive(Debug, Args)]
pub struct ListArgs   { #[arg(long)] pub tag: Option<String>, #[command(flatten)] pub scope: ScopeArgs, #[arg(long)] pub json: bool }
#[derive(Debug, Args)]
pub struct SearchArgs { pub query: String, #[arg(long)] pub tag: Option<String>, #[command(flatten)] pub scope: ScopeArgs, #[arg(long)] pub json: bool }
#[derive(Debug, Args)]
pub struct EditArgs   { pub id: String, #[command(flatten)] pub scope: ScopeArgs }
#[derive(Debug, Args)]
pub struct RmArgs     { pub id: String, #[arg(long, short = 'y')] pub yes: bool, #[command(flatten)] pub scope: ScopeArgs, #[arg(long)] pub json: bool }
#[derive(Debug, Args)]
pub struct MoveArgs   { pub id: String, #[arg(long, value_enum)] pub to: ScopeArg, #[arg(long)] pub json: bool }

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ScopeArg { Local, Global }     // → core::Scope
```

- 사용법 오류(필수 인자 누락, `--local` 과 `--global` 동시, `--body` 와 `--stdin` 동시)는 clap 이 종료 코드 2 로 처리한다. `Cli::try_parse()` 를 쓰고 `e.exit()` 를 호출한다 (`--help`, `--version` 은 0).
- `--home` 은 `env = "PH_HOME"` 으로 환경변수를 clap 이 읽는다 (`--home` 이 우선). 그래서 `main` 이 `PH_HOME` 을 따로 읽지 않는다.
- `get` 의 `--var`, `--allow-missing` 은 M3 에서 추가한다. M1 의 `get` 은 본문을 그대로 출력한다.
- `--json` 은 데이터를 출력하는 커맨드(`init`, `add`, `get`, `list`, `search`, `rm`, `move`)에만 둔다. `edit` 은 대화형이라 두지 않는다.
- `ph edit` 과 TUI 전용 동작을 섞지 않는다 (SPEC 7절).

### 10.3 조립과 실행 흐름

```rust
// bootstrap.rs — main 만 호출한다. cli/tui 는 완성된 PromptService 를 받는다.
pub struct BootstrapInput<'a> { pub home_override: Option<&'a Path>, pub cwd: &'a Path }
pub struct Runtime {
    pub service: PromptService,
    pub dirs: platform::paths::Dirs,
    pub local_root: Option<PathBuf>,        // 찾은 .ph 디렉터리 (없으면 None)
}
pub fn build_runtime(input: &BootstrapInput) -> Result<Runtime, PhError>;

struct SystemClock;                         // impl core::Clock { fn now → platform::time::now_local() }
```

`build_runtime` 순서:

1. `platform::paths::resolve_dirs(home_override)` → `Dirs` (실패하면 `PhError::io("global 저장소 경로 결정", e)`).
2. global: `FsStorage::new(Scope::Global, dirs.global_prompts.clone())`.
3. local: `storage::fs::find_local(cwd, dirs.home.as_deref())` 가 `Some(ph_dir)` 이면 `FsStorage::new(Scope::Local, ph_dir.join(PROMPTS_DIR))`, 아니면 `None`.
4. `PromptService::new(global, local, Box::new(SystemClock))`.

디렉터리 자체는 만들지 않는다 (`FsStorage` 가 첫 `put` 에서 만든다). 읽기만 하는 명령은 디스크를 바꾸지 않는다.

```rust
// cli/context.rs
pub struct CliIo<'a> {
    pub stdin: &'a mut dyn Read,
    pub stdout: &'a mut dyn Write,
    pub stderr: &'a mut dyn Write,
    /// stdin 이 터미널인가. false 면 확인 프롬프트를 띄우지 않는다 (SPEC 4절).
    pub interactive: bool,
    pub cwd: &'a Path,
    pub home_override: Option<&'a Path>,             // init 이 홈 디렉터리를 판별할 때 쓴다
    pub env: &'a dyn Fn(&str) -> Option<OsString>,   // 에디터 결정용. 실제로는 platform::env::get
    pub editor: &'a dyn EditorLauncher,
}

pub trait EditorLauncher {                           // 테스트에서 가짜로 바꾼다
    fn run(&self, cmd: &EditorCommand, file: &Path) -> io::Result<EditorExit>;
}
pub struct SystemEditor;                             // platform::editor::run_editor 위임

// cli/mod.rs
pub fn run(
    command: Command,
    io: &mut CliIo,
    service: &dyn Fn() -> Result<PromptService, PhError>,   // 지연 생성: init 은 global 경로 없이도 동작해야 한다
) -> Result<(), PhError>;
```

`main.rs` 골격:

```rust
fn main() -> ExitCode {
    let cli = match Cli::try_parse() { Ok(c) => c, Err(e) => e.exit() };      // 사용법 오류 = 2
    let cwd = match platform::env::cwd() { Ok(p) => p, Err(e) => return fail(PhError::io("현재 디렉터리 확인", e), false) };
    let interactive = std::io::stdin().is_terminal();
    // stdin/stdout/stderr 잠금, CliIo 구성 ...
    let command = cli.command.unwrap_or(Command::Tui);
    let result = match command {
        Command::Tui => tui_entry(...),     // M1: "TUI 는 아직 구현되지 않았습니다. `ph --help` 로 CLI 사용법을 확인하세요" 를 stderr 로 내고 종료 코드 1
        cmd => cli::run(cmd, &mut io, &|| bootstrap::build_runtime(&input).map(|r| r.service)),
    };
    match result { Ok(()) => ExitCode::SUCCESS, Err(e) => exit::report_error(&e, json_requested, &mut stderr) }
}
```

- `--json` 이 요청됐는지는 `Command` 에서 꺼내 `report_error` 에 넘긴다 (`Command::wants_json() -> bool`).
- `std::io::IsTerminal` 은 std 의 크로스 플랫폼 API 이므로 `main` 에서 써도 이식성 규칙 위반이 아니다.
- **stdout 이 닫힌 파이프** (`ph list | head`): 쓰기 오류 `BrokenPipe` 는 조용히 종료 코드 0 으로 처리한다 (`exit.rs` 가 `PhError::Io` 의 `source.kind()` 로 판별).

### 10.4 커맨드별 동작

`svc` 는 `PromptService`, "해석" 은 `args.scope.filter()`. stdout 은 데이터만, stderr 는 안내, 경고, 에러다.

| 커맨드 | 서비스 호출 | stdout (text) | stderr | 실패 |
|---|---|---|---|---|
| `init` | 서비스 없음. `storage::fs::init_local(io.cwd, home)` | `.ph` 디렉터리 경로 한 줄 | 새로 만들었으면 `local 저장소를 만들었습니다: <경로>`. 이미 있으면 `이미 초기화되어 있습니다: <경로>` (종료 0). 상위 디렉터리에 다른 local 이 있으면 `참고: 상위 <경로> 에도 local 저장소가 있습니다` | cwd 가 홈이면 `Usage` (2) |
| `add` | 본문 결정 → `svc.add(NewPrompt, args.scope.target())` | 저장된 id 한 줄 | `Written.auto_selected` 일 때만 `저장됨: [L] <id> (<위치>)` | 잘못된 title → `InvalidId`(1), `--local` 인데 local 없음 → `LocalNotInitialized`(1) |
| `get` | `svc.get(id, filter)` | **본문만**, 저장된 그대로 (끝에 개행을 덧붙이지 않는다) | `ambiguous` 면 경고 (아래) | 없음 → `NotFound`(3), 깨진 파일 → `InvalidFormat`(1) |
| `list` | `svc.list(filter, tag)` | 한 줄에 하나 (10.5절) | `skipped` 마다 경고 한 줄 | `--local` 인데 local 없음 → 1 |
| `search` | `svc.search(query, filter, tag)` | `list` 와 같은 형식 | `list` 와 같음 (search 는 skipped 를 돌려주지 않으므로 M1 에서는 경고 없음. 필요하면 서비스에서 `ListResult` 를 돌려주게 바꾼다) | 빈 검색어 → `Usage`(2) |
| `edit` | 10.6절 | 없음 | `저장됨: [L] <id>` 또는 `변경 없음` | 10.6절 |
| `rm` | `svc.get` → 확인 → `svc.remove(id, Only(scope))` | (JSON 일 때만) | `삭제됨: [L] <id>`. 양쪽에 같은 id 가 있었으면 `참고: global 에 같은 id 가 남아 있습니다` | 없는 id → `NotFound`(3, `--yes` 여부와 무관). 존재하는 id 이고 비대화형에서 `--yes` 없음 → `NonInteractive`(2) |
| `move` | `svc.move_to(id, to)` | 이동한 prompt 의 id | `이동됨: [G] <id> → [L]` | 대상에 같은 id → `AlreadyExists`(1) |

세부 규칙:

- **본문 결정 (`add`)**: `--body <text>` 는 그대로, `--file <path>` 는 UTF-8 로 읽는다 (BOM 과 CRLF 는 읽기에서 허용하고 저장은 서비스와 `format` 이 LF 로 통일한다), `--stdin` 은 EOF 까지 읽는다. 셋 중 하나는 clap 이 필수로 강제한다. 빈 본문도 허용한다. 파일을 읽지 못하면 `PhError::io("<경로> 읽기", e)`.
- **ambiguous 경고 (`get`, `rm`, `edit`)**: `경고: id '<id>' 가 local 과 global 양쪽에 있습니다. local 을 사용합니다. global 을 쓰려면 --global 을 지정하세요`. 결과는 바뀌지 않는다 (SPEC 3.2절 Agent 안전 규칙).
- **skipped 경고**: `경고: 읽지 못한 파일 [G] <이름>: <사유>`. 종료 코드는 0 이다.
- **`rm` 확인**: `--yes` 가 있으면 확인 없이 삭제한다. 없고 `io.interactive` 이면 stderr 에 `[L] <title> (<id>) 를 삭제할까요? [y/N] ` 를 쓰고 stdin 에서 한 줄을 읽는다. `y`, `yes`(대소문자 무시)만 진행하고, 그 외는 `취소했습니다` 를 stderr 로 내고 종료 코드 0 이다. 비대화형이고 `--yes` 가 없으면 `NonInteractive("삭제하려면 --yes 를 지정하세요")` 로 **프롬프트 없이** 실패한다. 단 대상 조회(`svc.get`)가 확인보다 먼저이므로 **없는 id 는 `--yes` 없이도 `NotFound`(3)** 이고, `NonInteractive`(2) 는 존재하는 id 에만 나온다.
- **`rm`/`edit` 의 대상 고정**: `svc.get` 으로 대상을 찾은 뒤 그 항목의 scope 로 `Only(scope)` 를 만들어 이후 호출에 쓴다. 확인 프롬프트에 보인 항목과 실제로 지우는 항목이 같아야 한다.
- **`init` 의 `.gitkeep`**: `init_local` 은 `.ph/prompts/` 를 만들고 그 안에 빈 `.gitkeep` 을 둔다 (git 은 빈 디렉터리를 추적하지 않는다). `FsStorage::list` 는 `.md` 만 읽으므로 영향이 없다. 홈 디렉터리 판별은 `platform::paths::resolve_dirs(io.home_override)` 결과의 `Dirs.home` 과 `io.cwd` 를 비교한다 (경로 비교는 `Path` 로).
- **`move`**: `--to local` 인데 local 이 없으면 `LocalNotInitialized`. 시각은 바꾸지 않는다 (서비스 동작).

### 10.5 출력 형식

**text** (사람용. 형식은 안정적 계약이 아니다. 스크립트와 agent 는 `--json` 을 쓴다):

```
[L] 코드-리뷰-요청  코드 리뷰 요청  #review #rust
[G] Code-Review     Code Review     (shadowed)
```

`<배지> <id>  <title>` 뒤에 태그가 있으면 `  #tag ...`, 가려졌으면 `  (shadowed)`. 열 정렬은 하지 않는다 (한글 폭 문제를 피한다). 결과가 없으면 stdout 은 비고 종료 코드는 0 이다 (`search` 도 마찬가지. NotFound 로 취급하지 않는다).

**JSON** (스키마 버전 1). 모든 성공 출력은 **한 줄 JSON 객체 하나 + `\n`** 이고 최상위에 `"schema_version": 1` 을 둔다. 필드 이름과 순서는 아래 DTO 와 같다. `scope` 는 `"local"`/`"global"`, 시각은 RFC3339 문자열, 없는 값은 `null`, 태그는 항상 배열이다.

```rust
// cli/output.rs — 내부 모델과 분리된 외부 계약. 변경하면 SCHEMA_VERSION 을 올린다.
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Serialize)] pub struct PromptSummaryJson {
    pub id: String, pub scope: &'static str, pub title: String,
    pub description: Option<String>, pub tags: Vec<String>,
    pub created_at: String, pub updated_at: String,
    pub shadowed: bool,                  // list/search 에서만 의미가 있다. 그 외에는 false
}
#[derive(Serialize)] pub struct PromptJson {           // Summary + body
    #[serde(flatten)] pub summary: PromptSummaryJson, pub body: String,
}
#[derive(Serialize)] pub struct WarningJson { pub kind: &'static str, pub scope: &'static str, pub name: String, pub reason: String }
```

| 커맨드 | JSON 최상위 |
|---|---|
| `list`, `search` | `{"schema_version":1,"count":N,"prompts":[PromptSummary...],"warnings":[Warning...]}` |
| `get` | `{"schema_version":1,"prompt":Prompt,"ambiguous":bool}` (M1 의 `body` 는 원문. M3 에서 `--var` 를 쓰면 치환된 본문을 `body` 에 담고 `rendered: true` 를 더한다) |
| `add` | `{"schema_version":1,"prompt":Prompt,"auto_selected":bool}` |
| `rm` | `{"schema_version":1,"removed":{"id":"...","scope":"local"}}` |
| `move` | `{"schema_version":1,"prompt":Prompt,"from":"local","to":"global"}` |
| `init` | `{"schema_version":1,"path":"...","created":bool}` |

- `--json` 이어도 stderr 의 경고와 안내는 그대로 나간다 (stdout 만 JSON 이다). `warnings` 배열은 같은 내용을 agent 가 읽기 쉽게 담은 것이다 (`kind` 는 `"skipped"`).
- `get --json` 에서 `ambiguous` 는 stderr 경고와 별개로 항상 채운다.
- 에러 JSON 은 10.7절.

### 10.6 `ph edit` 흐름

```
edit <id> [--local|--global]
 1. io.interactive 가 false 면 NonInteractive("ph edit 은 터미널에서만 쓸 수 있습니다. 스크립트는 ph add/rm 을 쓰세요") → 종료 2
 2. resolved = svc.get(id, filter)               # 대상 확정. ambiguous 면 경고
    scope = resolved.prompt.scope;  only = Only(scope)
 3. raw = svc.export_raw(id, only)               # frontmatter 포함 전체
 4. tmp = TempEditFile::create(id, &raw)         # platform::editor. Drop 시 삭제
 5. cmd = resolve_editor(io.env)
 6. loop:
      exit = io.editor.run(&cmd, tmp.path())
        Err(io)            → Editor("에디터를 실행하지 못했습니다 ('<program>'): <원인>. $VISUAL 또는 $EDITOR 를 설정하세요")  → 종료 1
        Ok(Failed(code))   → Editor("에디터가 비정상 종료했습니다 (코드 N). 변경은 반영되지 않았습니다")                 → 종료 1
        Ok(Success)        → edited = tmp.read()
      if edited == raw     → stderr "변경 없음", 종료 0 (저장하지 않는다. updated_at 유지)
      match svc.save_raw(id, &edited, only):
        Ok(w)   → stderr "저장됨: [L] <id>" (w.notes 에 IdKeyIgnored 가 있으면 "참고: id 는 변경되지 않습니다"), 종료 0
        Err(e) if e 가 InvalidFormat | InvalidId | Usage (검증 실패)
                → stderr 에 e 를 (줄 번호 포함) 출력, "원본 prompt 는 변경되지 않았습니다."
                  prompt "다시 편집할까요? [Y/n]"  → `n`/`no`(대소문자 무시)만 거절: 종료 1 (e 를 그대로 반환). 그 외 입력(빈 줄, y, 기타)은 모두 재편집: 같은 임시 파일 내용 그대로 loop 재진입
        Err(e)  → 그대로 반환 (Io 등)
```

- **임시 파일 계약** (`platform::editor`, TUI 의 `E` 도 이것을 쓴다):

  ```rust
  pub struct TempEditFile { /* 전용 임시 디렉터리 + 파일 */ }
  impl TempEditFile {
      /// `<임시 디렉터리>/ph-<pid>-<n>/ph-<id>.md` 에 contents 를 쓴다. 파일 이름에 id 를 쓰는 이유는 에디터 제목줄에 보이게 하기 위함이다.
      pub fn create(id: &str, contents: &str) -> io::Result<Self>;
      pub fn path(&self) -> &Path;
      pub fn read(&self) -> io::Result<String>;     // UTF-8 아니면 InvalidData
  }   // Drop: 디렉터리 삭제 (실패는 무시)
  ```

  `std::env::temp_dir()` 는 크로스 플랫폼 std API 다. `tempfile` 은 dev-dependency 로만 둔다. 임시 파일 권한 설정 (chmod) 은 하지 않는다 (규칙 6).
- 에디터 종료 후 `read()` 가 UTF-8 오류이면 검증 실패와 같게 취급해 재편집을 묻는다 (`InvalidFormat`).
- 비대화형 거부는 `edit` 만이다. 에디터는 TTY 를 요구할 수 있고 agent 가 실수로 에디터를 띄워 멈추는 사고를 막기 위해서다. 종료 코드 2 는 "사용법 오류" 로 본다.
- "다시 편집" 프롬프트의 입력은 `io.stdin` 이다 (`interactive` 가 참일 때만 도달한다).
- 재편집 반복 횟수 제한은 없다.
- `save_raw` 가 `id` 를 바꾸지 않으므로 루프 안에서 `id` 와 `only` 는 고정이다.

### 10.7 에러 출력과 종료 코드

```rust
// cli/exit.rs
pub fn exit_code(e: &PhError) -> u8;
pub fn error_kind(e: &PhError) -> &'static str;      // JSON 에러의 kind
pub fn report_error(e: &PhError, json: bool, stderr: &mut dyn Write) -> ExitCode;
```

| PhError | 종료 코드 | `kind` |
|---|---|---|
| (성공, 취소 포함) | 0 | - |
| `NotFound` | **3** | `not_found` |
| `Usage`, `NonInteractive`, clap 사용법 오류 | **2** | `usage`, `non_interactive` |
| `InvalidId` | 1 | `invalid_id` |
| `InvalidFormat` | 1 | `invalid_format` |
| `AlreadyExists` | 1 | `already_exists` |
| `LocalNotInitialized` | 1 | `local_not_initialized` |
| `MissingVariable` | 1 | `missing_variable` |
| `Editor` | 1 | `editor` |
| `Clipboard` | 1 | `clipboard` |
| `Io` | 1 (단, `BrokenPipe` 는 0, 출력 없음) | `io` |

- text 모드 에러: `오류: <PhError 메시지>` 한 줄을 stderr 로. 메시지는 이미 "다음 행동"을 담고 있으므로 덧붙이지 않는다.
- `--json` 모드 에러: stdout 은 **비우고**, stderr 에 `{"schema_version":1,"error":{"kind":"not_found","message":"..."}}` 한 줄을 쓴다. clap 의 사용법 오류는 clap 의 텍스트 그대로 stderr 로 나간다 (JSON 아님).
- `InvalidId` 는 입력값이 규칙에 어긋난 경우라 "사용법 오류(2)" 로 볼 여지가 있으나, SPEC 4절의 정의(2 = 사용법 오류)를 clap 이 검출하는 것으로 좁히고 core 가 검출하는 값 오류는 1 로 둔다. 예외는 `Usage`/`NonInteractive` 로 core 와 cli 가 명시한 것뿐이다.
- 종료 코드 매핑은 `exit_code` 한 곳에만 있고 단위 테스트가 모든 `PhError` 변형을 순회해 표와 일치함을 확인한다 (변형 추가 시 컴파일 오류가 나도록 `match` 에 `_` 를 쓰지 않는다).

### 10.8 테스트 지침 (QA 와 coder)

- **핸들러 단위 테스트**: `CliIo` 에 `Vec<u8>`/`Cursor` 를 넣고 `MemoryStorage` + `FixedClock` 으로 만든 `PromptService` 를 넘긴다. `EditorLauncher` 가짜로 성공, 비정상 종료, 내용 수정, 깨진 frontmatter 를 재현한다. 실제 프로세스나 파일시스템이 필요 없다.
- **통합 테스트 (`tests/cli_*.rs`)**: 바이너리를 `std::process::Command` 로 실행한다. 매번 `env_clear()` 후 `PH_HOME=<tempdir>`, `current_dir(<tempdir>)` 를 지정한다 (`EDITOR`, `VISUAL`, `HOME` 이 새어 들어가지 않게). local 은 임시 cwd 에서 `ph init` 으로 만든다. 종료 코드 표(0/1/2/3)를 케이스마다 assert 한다.
- assert 는 경로 구분자와 개행에 의존하지 않는다 (규칙 8): 줄 단위 비교는 `lines()`, 경로는 `Path` 로 비교. `--json` 은 `serde_json::Value` 로 파싱해 필드를 확인한다.
- `edit` 의 통합 테스트는 가짜 에디터 스크립트가 필요하다. 스크립트 생성은 `tests/common` 헬퍼에 격리하고 Unix 전용이면 그 헬퍼 안에서만 `cfg` 를 쓴다 (`src/` 규칙은 아니지만 Windows 컴파일 확인이 깨지지 않게).
- stdout/stderr 구분을 검증한다: `get` 은 stdout 이 본문과 정확히 같고 stderr 에 데이터가 없어야 한다. 에러 시 stdout 이 비어 있어야 한다.
- 다음 동작은 회귀 테스트로 잠근다: `get` 의 ambiguous 시 local 반환과 경고, 깨진 파일 `get` 의 `InvalidFormat`(종료 1), `--local` 인데 local 없음(읽기와 쓰기 모두 종료 1), 비대화형 `rm` 의 `--yes` 요구(종료 2).

### 10.9 M1 범위 밖 (하지 않는다)

`get` 의 `--var`/`--allow-missing`, `tag`, `export`, `import` (M3), `skill install` (M4), TUI (M2). `ph` 인자 없이 실행하면 M1 에서는 안내 메시지와 종료 코드 1 이다.

## 11. 설계 결정 기록 (ADR)

**ADR-1 단일 crate 유지.** 근거: 규모가 작고 workspace 는 조기 복잡성이다. 대신 `use` 규칙(2절 표)을 리뷰와 grep 으로 지킨다. 재검토 조건: 컴파일 시간이 문제이거나 core 를 외부에서 재사용해야 할 때. 그때 `core` → `ph-core` 로 분리하면 되도록 core 는 다른 모듈을 참조하지 않는다.

**ADR-2 동기 IO.** async 를 쓰지 않는다. 저장소가 로컬 파일이고 clap, crossterm 이벤트 모두 동기로 충분하다.

**ADR-3 Storage 는 scope 하나, 병합은 service.** SPEC 3.3절과 같은 구조다. 병합과 우선순위를 storage 에 두면 memory 구현과 fs 구현이 각자 병합 규칙을 구현해야 한다. service 한 곳에 두어 테스트 대상을 하나로 만든다.

**ADR-4 파일 포맷 코덱은 core.** frontmatter 파싱은 순수 문자열 변환이라 IO 가 아니다. 외부 에디터 저장 검증과 fs 저장소가 같은 코드를 쓰게 한다.

**ADR-5 시간은 `Clock` 주입.** `updated_at` 갱신 테스트가 결정적이어야 한다. `Local::now()` 는 `platform::time` 에만 둔다.

**ADR-6 에러는 `PhError` 하나, 종료 코드는 cli 에서 매핑.** core 가 프로세스 종료 규약을 모르게 하고, TUI 는 같은 에러를 상태바 메시지로 쓴다.

**ADR-7 `--json` 은 전용 DTO.** 내부 모델이 바뀌어도 외부 스키마가 조용히 바뀌지 않게 한다. 스키마 변경 시 버전을 올린다 (SPEC 4절).

**ADR-8 id 는 title 이 바뀌어도 유지.** 파일명과 agent 참조를 안정적으로 유지하기 위해서다. 외부 에디터 frontmatter 의 `id` 키는 무시한다 (사용자 결정, SPEC 2절). 이름 변경은 별도 동작이 필요하며 v0.1 에는 없다 (`move` 는 scope 이동 전용).

**ADR-9 인라인 에디터는 `ratatui-textarea`.** 6절 참고. SPEC 의 후보 `tui-textarea` 는 ratatui 0.29 에 묶여 채택하지 않는다.

**ADR-10 `$EDITOR` 는 공백 분리, 셸 미사용.** 셸 주입과 Windows 확장 시의 quoting 차이를 피한다. 따옴표가 필요한 복잡한 값은 v0.1 미지원이다.

**ADR-11 `anyhow` 미도입.** `main` 이 하는 일은 `PhError` 를 종료 코드로 바꾸는 것뿐이고 `PhError` 가 이미 사용자 메시지를 담는다. 오류 종류가 코드에서 갈라져야 하므로(3 = NotFound 등) 타입을 지운 에러가 오히려 방해가 된다. SPEC 6절 crate 표의 `anyhow` 는 "필요해지면" 으로 읽는다.

**ADR-12 CLI 핸들러는 얇게, 서비스는 지연 생성.** `cli::run` 은 `PromptService` 를 만드는 클로저를 받는다. `init` 은 global 경로를 못 구해도 동작해야 하고, 핸들러를 `MemoryStorage` 로 테스트하려면 서비스 주입이 필요하다. `bootstrap` 을 `cli` 가 import 하지 않으므로 의존 방향(main → bootstrap)이 유지된다.

**ADR-13 platform 은 `io::Result`, 매핑은 호출자.** `platform` 이 `core::PhError` 를 반환하면 `platform → core` 의존이 생긴다 (SPEC 6절 금지). Clock 어댑터가 bootstrap 에 있는 이유도 같다.

**ADR-14 환경 판별은 platform 에서만, 값으로 전달.** `PH_HOME` 은 clap 의 `env` 로, `EDITOR`/`VISUAL` 은 주입되는 `env` 클로저로, `COLORTERM`/`NO_COLOR` 는 `platform::terminal` 이 `ColorMode` 로 바꿔 전달한다. `view` 와 `core` 는 환경을 읽지 않는다 (SPEC 6절 규칙 7).

## 12. coder 를 위한 리뷰 체크리스트

- [ ] 2절 표에 없는 `use` 가 없는가 (특히 `core` 에서 `std::fs`, `std::env`, `platform`)
- [ ] `cli` 와 `tui` 가 scope 병합, 우선순위, 쓰기 대상 규칙을 다시 구현하지 않았는가
- [ ] `platform/` 밖에 `cfg(unix)`, `std::os::unix`, `libc`, `XDG_*`, `HOME`, 문자열 경로 결합이 없는가
- [ ] 테스트 밖 `unwrap`/`expect` 가 없는가, 공개 API 에 `///` 가 있는가
- [ ] 새 의존성의 라이선스와 이 문서의 표 갱신
- [ ] 통합 테스트가 `PH_HOME` 과 임시 cwd 로 격리되어 있는가 (`env_clear` 포함)
- [ ] `cli` 핸들러가 stdout 에는 데이터만, stderr 에는 안내/경고/에러만 쓰는가 (10절)
- [ ] `--json` 출력이 10.5절 DTO 와 일치하고 `schema_version` 이 있는가
- [ ] 종료 코드 매핑이 `exit_code` 한 곳에만 있는가 (10.7절)
- [ ] TUI 가 `ScopeFilter::Only(항목의 scope)` 로 서비스를 호출하는가 (3.4절)
