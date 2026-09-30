# ph (Prompt Hub) SPEC

> 버전: 0.1 (초안) / 언어: Rust / 상태: 작업 착수 전
> 작업 현황은 [agent/TASK.md](agent/TASK.md) 에서 관리한다.

## 1. 개요

`ph` 는 AI Agent 를 사용할 때 쓰는 **prompt 저장소(Prompt Hub)** 가 핵심 기능인 CLI 프로그램이다.

- 인자 없이 실행하면 **TUI** 로 동작한다.
- 서브커맨드를 주면 **CLI** 로 동작한다. 스크립트와 agent 가 이 모드를 쓴다.
- agent(Claude Code 등)가 바로 호출할 수 있게 **Claude skill 로 확장 가능한 구조**를 갖춘다.

### 지원 플랫폼
- v0.1 지원: **Linux, macOS**
- Windows 는 지원하지 않지만, **이후 Windows 로 확장할 수 있도록 설계한다.** OS 에 종속된 코드는 `platform/` 모듈에 격리하고, 이식성 규칙(6절)을 처음부터 지킨다.

### 목표
- prompt 를 빠르게 저장, 검색, 재사용한다.
- TUI 와 CLI 가 **같은 core 로직**을 공유한다. 기능 차이가 생기면 안 된다.
- agent 가 파싱하기 쉬운 출력(`--json`)을 제공한다.

### 비목표 (v0.1)
- 클라우드 동기화, 계정, 원격 서버
- prompt 실행(LLM 호출) 자체. `ph` 는 저장소이고 실행기가 아니다.
- prompt 버전 이력 관리. 수정하면 이전 내용은 남지 않는다. 이력이 필요하면 local 저장소(`.ph/`)를 git 으로 관리한다. global 저장소는 이력이 없다. 필요성이 확인되면 이후 버전에서 재검토한다.

## 2. 핵심 개념

### Prompt
| 필드 | 타입 | 설명 |
|---|---|---|
| `id` | String (slug) | scope 안에서 고유한 식별자. title 에서 생성하며 중복이면 suffix 를 붙인다. 파일명으로 쓰이므로 **모든 OS 에서 유효한 이름**이어야 한다 (아래 id 규칙) |
| `scope` | `local` \| `global` | 저장 위치. 파일에는 저장하지 않고 읽을 때 위치로 결정한다 |
| `title` | String | 표시 이름 |
| `body` | String | prompt 본문. `{{var}}` 형태의 변수를 허용한다 |
| `tags` | Vec<String> | 분류용 태그 |
| `description` | Option<String> | 짧은 설명 |
| `created_at` / `updated_at` | RFC3339 | 생성 및 수정 시각 |

### id 규칙 (Windows 호환을 위해 처음부터 적용)
- **생성 방식:** title 을 그대로 id 로 쓰되 공백(연속 공백 포함)만 `-` 하나로 바꾼다. 한글은 로마자로 바꾸지 않고 그대로 두며, 영문도 대소문자를 바꾸지 않고 그대로 둔다. 앞뒤 공백은 제거한다.

  | title | id (파일명) |
  |---|---|
  | `코드 리뷰` | `코드-리뷰.md` |
  | `Code Review` | `Code-Review.md` |
  | `PR 리뷰  요청` | `PR-리뷰-요청.md` |

- 금지 문자: `/ \ : * ? " < > |` 와 제어 문자. title 에 있으면 생성하지 않고 에러로 처리한다 (조용히 지우거나 바꾸지 않는다).
- 끝이 `.` 이나 공백이면 안 된다.
- Windows 예약 이름(`CON`, `PRN`, `AUX`, `NUL`, `COM1`~`COM9`, `LPT1`~`LPT9`)은 쓸 수 없다. 대소문자 무관.
- id 는 **대소문자를 구분하지 않고** 중복을 검사한다 (macOS 와 Windows 는 대소문자를 구분하지 않는 파일시스템이 기본이다).
- 유니코드(한글)는 허용하고 NFC 로 정규화해서 저장한다 (macOS 는 파일명을 NFD 로 다루는 경우가 있다).
- 길이는 **UTF-8 기준 100바이트 이하**로 제한한다 (파일명 길이 제한은 바이트 기준이다. 한글은 글자당 3바이트).

### 변수 치환
- 본문의 `{{name}}` 은 `ph get <id> --var name=value` 로 치환한다.
- 값이 없는 변수는 기본적으로 에러로 처리한다. `--allow-missing` 을 주면 원문 그대로 둔다.

## 3. 저장소 (Storage)

### 3.1 저장소 범위 (Scope)

저장소는 두 종류이고 **둘 다 지원한다.**

| Scope | 경로 | 용도 |
|---|---|---|
| `global` | 플랫폼별 데이터 디렉터리 아래 `ph/prompts/` | 프로젝트와 무관하게 쓰는 개인 prompt |
| `local` | `<프로젝트 루트>/.ph/prompts/` | 프로젝트 전용 prompt. git 에 커밋해 팀과 공유할 수 있다 |

- **global 기본 경로:** Linux 는 `$XDG_DATA_HOME/ph/prompts/` (없으면 `~/.local/share/ph/prompts/`), macOS 는 `~/Library/Application Support/ph/prompts/` 다. 경로는 직접 조립하지 않고 `platform::paths` 가 제공한다 (Windows 확장 시 `%APPDATA%` 를 이 모듈에서만 추가한다).
- **local 탐색:** 현재 디렉터리에서 상위로 올라가며 `.ph/` 를 찾는다 (git 이 `.git` 을 찾는 방식). 홈 디렉터리나 파일시스템 루트에 닿으면 멈추고, 홈의 `.ph/` 는 local 로 인정하지 않는다.
- **`ph init`:** 현재 디렉터리에 `.ph/prompts/` 를 만든다. 자동 생성은 하지 않는다.
- **경로 덮어쓰기:** `PH_HOME` 이나 `--home <path>` 는 **global 경로만** 바꾼다. 테스트에서는 `PH_HOME` 을 임시 디렉터리로 지정하고, local 은 임시 디렉터리를 cwd 로 삼아 검증한다.

### 3.2 Scope 해석 규칙

**읽기 (`list`, `search`, `get`)**
- local 과 global 을 **합쳐서** 보여준다.
- 같은 `id` 가 양쪽에 있으면 **local 이 우선**한다 (global 것은 가려진다). `list` 에서는 가려진 항목을 `shadowed` 로 표시한다.
- `--local` 또는 `--global` 을 주면 해당 scope 만 대상으로 한다.
- 모든 출력에 scope 를 표시한다 (text 는 `[L]`/`[G]` 배지, JSON 은 `"scope": "local"|"global"` 필드).

**쓰기 (`add`, `edit`, `rm`, `tag`)**
- `--local` 또는 `--global` 이 명시되면 그대로 따른다.
- 명시하지 않으면 **local 이 있을 때는 local, 없으면 global** 에 쓴다. 어디에 썼는지 stderr 로 알린다.
- `--local` 인데 `.ph/` 가 없으면 에러로 처리하고 `ph init` 을 안내한다. 조용히 global 에 쓰지 않는다.
- `edit`, `rm`, `tag` 는 읽기 규칙으로 대상을 찾는다 (local 우선). 다른 scope 의 같은 id 를 다루려면 `--global` 을 명시한다.
- `ph move <id> --to local|global` 로 scope 를 옮길 수 있다.

**Agent 안전 규칙:** `ph get` 은 id 가 모호하면 (양쪽에 존재) local 을 반환하고 stderr 에 경고를 남긴다. 결과가 바뀌지 않도록 이 동작은 고정한다.

### 3.3 파일 형식

- prompt 하나는 **Markdown 파일 하나**(`<id>.md`)이고, 상단에 TOML frontmatter(`+++`)를 둔다. 사람이 직접 읽고, 편집하고, git 으로 관리하기 쉽다.

```
+++
title = "코드 리뷰 요청"
tags = ["review", "rust"]
description = "PR 리뷰용"
created_at = "2026-09-30T12:00:00+09:00"
updated_at = "2026-09-30T12:00:00+09:00"
+++
다음 diff 를 리뷰해줘. 관점: {{focus}}
```

- 쓰기는 임시 파일에 쓴 뒤 rename 하는 방식으로 atomic 하게 한다.
- 저장소 접근은 `Storage` trait 로 추상화한다. 테스트에서는 in-memory 구현을 쓴다.
- `PromptService` 는 scope 별 `Storage` 인스턴스(global 필수, local 선택)를 가지며, 3.2절의 병합, 우선순위, 쓰기 대상 결정을 **core 안에서** 처리한다. cli 와 tui 는 이 규칙을 다시 구현하지 않는다.

## 4. CLI 명세

```
ph                         # TUI 실행
ph tui                     # TUI 실행 (명시)
ph init                    # 현재 디렉터리에 local 저장소(.ph/) 생성
ph add <title> [--body <text> | --file <path> | --stdin] [--tag <t>]... [--desc <text>]
ph get <id> [--var k=v]... [--allow-missing]    # body 만 stdout 으로 출력
ph list [--tag <t>] [--json]
ph search <query> [--tag <t>] [--json]
ph edit <id>               # $EDITOR 로 편집
ph rm <id> [--yes]
ph tag <id> [--add <t>]... [--remove <t>]...
ph move <id> --to <local|global>
ph export [--out <path>]   # 전체를 JSON 으로 내보내기
ph import <path>
ph skill install [--dir <path>]   # Claude skill 파일 설치 (7절 참고)
```

### 공통 규칙
- `init`, `move` 를 제외한 저장소 대상 커맨드는 `--local` / `--global` 을 받는다. 동작은 3.2절을 따른다.
- 데이터는 **stdout**, 로그와 에러는 **stderr** 로 출력한다. `ph get` 은 파이프에 바로 쓸 수 있어야 한다.
- `--json` 은 안정적인 스키마를 가진다. 스키마를 바꾸면 버전을 올린다.
- 종료 코드: `0` 성공, `1` 일반 오류, `2` 사용법 오류, `3` 대상 없음(NotFound).
- 비대화형(stdin 이 TTY 가 아님) 환경에서는 확인 프롬프트를 띄우지 않고 `--yes` 를 요구한다.

## 5. TUI 명세

- 구성: 좌측 **목록/검색**, 우측 **미리보기**, 하단 **상태바 및 키 도움말**.
- 모드: Normal / Search / Edit / Confirm.
- 기본 키(초안, Designer 가 확정한다):

| 키 | 동작 |
|---|---|
| `j`/`k`, `↑`/`↓` | 이동 |
| `/` | 검색 |
| `Enter` | 선택한 prompt 를 클립보드로 복사 |
| `s` | scope 필터 순환 (all → local → global) |
| `a` | 새 prompt 추가 (저장 scope 선택, 기본값은 3.2절 규칙) |
| `e` | TUI 내 편집 (인라인 에디터) |
| `E` | 외부 텍스트 에디터로 편집 (`$VISUAL` → `$EDITOR`) |
| `d` | 삭제 (확인 필요) |
| `t` | 태그 편집 |
| `?` | 도움말 |
| `q` / `Esc` | 종료 / 뒤로 |

- 프레임워크는 **ratatui** (+ crossterm backend) 를 사용한다. 화면은 ratatui 의 `Layout`, `Block`, `List`, `Paragraph`, `Table` 등 내장 위젯으로 구성하고, 필요할 때만 `Widget` trait 로 커스텀 위젯을 만든다.
- 입력 처리는 crossterm 이벤트 루프로 하고, 터미널 초기화와 복구(raw mode, alternate screen)는 panic 시에도 복원되도록 hook 을 건다.
### 5.1 편집 방식 (두 가지 모두 지원)

| 방식 | 키 | 설명 |
|---|---|---|
| 인라인 에디터 | `e` | TUI 화면 안에서 바로 수정한다. 터미널을 벗어나지 않는다. |
| 외부 에디터 | `E` | `$VISUAL`, 없으면 `$EDITOR` 를 실행한다. 둘 다 없으면 `vi` 를 시도하고, 그것도 없으면 에러 메시지를 표시한다. |

**인라인 에디터**
- 본문(`body`) 편집 영역과 `title`, `tags`, `description` 입력 필드로 구성한다. `Tab` / `Shift+Tab` 으로 필드를 이동한다.
- 본문은 여러 줄 편집, 커서 이동, 선택, 삭제, undo/redo, 줄바꿈을 지원한다. 한글 등 wide 문자의 폭을 올바르게 계산해야 한다.
- 저장 `Ctrl+S`, 취소 `Esc`. 변경 사항이 있는 채로 `Esc` 를 누르면 저장 확인을 묻는다.
- 새 prompt 추가(`a`)도 같은 인라인 에디터를 쓴다. 저장 scope 는 여기서 선택한다 (기본값은 3.2절).
- 구현은 `tui-textarea` 같은 ratatui 호환 crate 사용을 우선 검토한다. 채택은 Architect 가 검증해 결정한다.

**외부 에디터**
- TUI 를 잠시 중단하고(raw mode 와 alternate screen 해제) 에디터를 실행한 뒤, 종료되면 화면을 복구하고 다시 로드한다. CLI 의 `ph edit` 과 같은 `platform::editor` 를 공유한다.
- 임시 파일에는 frontmatter 를 포함한 파일 전체를 열어 title, tags, description 도 함께 편집할 수 있게 한다.
- 저장 후 frontmatter 나 id 가 유효하지 않으면 원본을 보존하고, 에러 위치를 보여준 뒤 다시 편집할지 묻는다.
- 에디터가 비정상 종료(non-zero)하면 변경을 반영하지 않는다.

**공통**
- 두 방식 모두 저장은 `PromptService` 를 통해서만 한다. 검증 규칙(id, frontmatter)과 atomic write 는 동일하게 적용된다.
- 저장 시 `updated_at` 을 갱신한다.

### 5.2 화면 표시

- 목록의 각 항목에 scope 배지(`[L]`/`[G]`)를 표시하고, 상태바에 현재 scope 필터와 local 저장소 경로(없으면 "local 없음")를 보여준다. 가려진(shadowed) global 항목은 흐리게 표시한다.
- 화면 상태(`AppState`)와 렌더링(`view`)을 분리한다. 렌더링은 순수 함수에 가깝게 유지해 `ratatui::backend::TestBackend` 로 검증할 수 있게 한다.
- 터미널 크기가 작으면 미리보기 패널을 자동으로 숨긴다.
- 디자인 세부 사항(색상, 레이아웃, 키맵)은 `docs/DESIGN.md` 에 Designer 가 작성한다.

## 6. 아키텍처

단일 crate 안에서 모듈로 계층을 나눈다. 규모가 커지면 workspace 로 분리한다.

```
src/
├─ main.rs          # 진입점: 인자 파싱 후 tui 또는 cli 로 분기
├─ core/            # 도메인. UI 와 IO 에 의존하지 않는다
│  ├─ model.rs      # Prompt, PromptId
│  ├─ service.rs    # PromptService (CRUD, 검색, 변수 치환)
│  ├─ template.rs   # {{var}} 파서 및 치환
│  └─ error.rs      # PhError (thiserror)
├─ storage/         # Storage trait + fs 구현 + memory 구현
├─ platform/        # OS 종속 코드 격리: paths, editor 실행, atomic write, 터미널 보정
├─ cli/             # clap 정의, 커맨드 핸들러, 출력 포맷(text/json)
├─ tui/             # AppState, event, update, view, theme
└─ skill/           # Claude skill 템플릿 생성 및 설치
tests/              # 통합 테스트 (CLI 대상)
```

### 의존 방향 (반드시 지킨다)
`main → cli, tui → core ← storage`, `skill → core`, `storage, cli, tui → platform`
- `platform` 은 `core` 를 참조하지 않는다. `core` 도 `platform` 을 직접 참조하지 않는다.
- `core` 는 `cli`, `tui`, `storage` 의 구현체를 import 하지 않는다. `Storage` trait 만 정의한다.
- `cli` 와 `tui` 는 서로 참조하지 않는다. 둘 다 `PromptService` 만 통해 동작한다.

### 이식성 규칙 (Windows 확장 대비)
1. `#[cfg(unix)]`, `std::os::unix`, `libc` 같은 OS 종속 코드는 `platform/` 밖에서 쓰지 않는다.
2. 경로는 항상 `Path`/`PathBuf` 로 다룬다. `"/"` 를 문자열로 이어 붙이거나 `~` 를 직접 치환하지 않는다. 홈/데이터 디렉터리는 `directories` crate 등을 통해 `platform::paths` 에서만 얻는다.
3. atomic write(임시 파일 후 rename)는 `platform::fs` 함수 하나로 감싼다. Windows 는 rename 동작이 다르므로 이후 이 함수만 교체하면 되게 한다.
4. 외부 프로그램 실행(`$EDITOR`)은 `platform::editor` 로 감싼다. 셸 문자열 결합 대신 program 과 args 를 분리해 실행한다.
5. 파일 읽기는 CRLF 와 UTF-8 BOM 을 허용한다. 쓸 때는 LF 로 통일한다. frontmatter 파서는 둘 다 처리해야 한다.
6. 파일 잠금이나 권한(chmod) 같은 Unix 전용 개념에 의존하지 않는다. 필요하면 `platform` 에 trait 로 추상화한다.
7. 환경변수는 `PH_HOME`, `EDITOR`, `VISUAL` 만 읽는다. Unix 전용 변수(`XDG_*`, `HOME`)는 `platform::paths` 안에서만 읽는다.
8. 통합 테스트는 경로 구분자나 개행에 의존하는 assert 를 쓰지 않는다.
9. CI 에서 지금은 Linux 와 macOS 를 돌리고, Windows 는 `cargo check --target x86_64-pc-windows-msvc` 로 **컴파일이 깨지지 않는지만** 확인한다 (M0 이후 추가).

### 채택 기술
**TUI 프레임워크는 `ratatui` 로 확정한다** (backend 는 `crossterm`). 그 외 항목은 초안이며 Architect 가 검증한다.

| 용도 | crate |
|---|---|
| TUI (확정) | `ratatui`, `crossterm` |
| CLI 파싱 | `clap` (derive) |
| 직렬화 | `serde`, `serde_json`, `toml` |
| 에러 | `thiserror` (라이브러리), `anyhow` (main) |
| 시간 | `time` 또는 `chrono` |
| 인라인 텍스트 편집 | `tui-textarea` (검토 후보) |
| 플랫폼 경로 | `directories` |
| 클립보드 | `arboard` (실패 시 에러 표시만, 대체 동작 없음) |

## 7. Claude skill 확장

- `ph skill install` 은 `.claude/skills/ph/SKILL.md` 를 생성한다. `--dir` 로 위치를 바꿀 수 있고, 기본은 현재 프로젝트다. 사용자 전역 설치는 `~/.claude/skills/ph/` 로 한다.
- SKILL.md 는 agent 에게 다음을 알려준다.
  - `ph list --json`, `ph search <q> --json` 으로 prompt 를 찾는다.
  - `ph get <id> --var k=v` 로 본문을 가져와 사용한다.
  - `ph add ... --stdin` 으로 좋은 prompt 를 저장한다.
- agent 가 쓰는 커맨드는 항상 **비대화형**이고 **`--json` 지원** 이어야 한다. 이를 위해 TUI 전용 동작을 CLI 서브커맨드에 섞지 않는다.
- SKILL.md 본문은 `skill/` 모듈의 템플릿에서 생성하고, 커맨드 목록이 바뀌면 함께 갱신한다.

## 8. 품질 기준

### 테스트
- 프레임워크: **Rust 내장 `cargo test`** (`#[test]`, `#[cfg(test)]`). 별도 test crate 는 v0.1 에서 도입하지 않는다.
- 단위 테스트는 각 모듈 내부의 `#[cfg(test)] mod tests` 에 둔다.
- 통합 테스트는 `tests/` 에 두고 임시 디렉터리(`PH_HOME`)로 CLI 바이너리를 실행해 검증한다. 필요하면 `std::process::Command` 를 쓴다.
- TUI 는 `AppState` 의 update 로직을 단위 테스트한다. 렌더링은 `TestBackend` 로 버퍼 내용을 assert 한다.
- `core` 는 `Storage` 의 in-memory 구현으로 테스트한다. 실제 파일시스템이 필요 없다.
- 새 기능은 테스트와 함께 병합한다.

### 정적 검사 (완료 조건)
```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```
세 가지가 모두 통과해야 작업을 완료로 본다.

### 라이선스
- **MIT** 라이선스. 저작권자는 Hanjae Lee (jayhanjaelee@gmail.com). 전문은 루트의 [LICENSE](LICENSE) 에 있다.
- 배포 패키지(crate) 이름은 **`prompt-hub`**, 실행 파일 이름은 **`ph`** 다. `Cargo.toml` 에 `[package] name = "prompt-hub"` 와 `[[bin]] name = "ph"` (path = `src/main.rs`) 를 설정한다. 문서, 도움말, skill 에서 사용자에게 보이는 명령은 항상 `ph` 다.
- `Cargo.toml` 에는 `license = "MIT"`, `authors = ["Hanjae Lee <jayhanjaelee@gmail.com>"]` 를 설정한다.
- 새 의존성은 MIT 와 호환되는 라이선스(MIT, Apache-2.0, BSD, ISC 등 허용적 라이선스)만 추가한다. GPL/AGPL 계열은 Architect 승인 없이 추가하지 않는다.

### 코드 규칙
- `unwrap()` 과 `expect()` 는 테스트 코드 밖에서 금지한다. 불가피하면 이유를 주석으로 남긴다.
- 공개 API 에는 `///` 문서 주석을 단다.
- 에러 메시지는 사용자가 다음 행동을 알 수 있게 쓴다.

## 9. 마일스톤

| 단계 | 내용 |
|---|---|
| M0 | 프로젝트 골격, `core` 모델과 `Storage` trait, fs 저장소 |
| M1 | CLI 핵심 (`add`, `get`, `list`, `search`, `rm`, `edit`) 과 `--json` |
| M2 | TUI (목록, 검색, 미리보기, 추가, 편집, 삭제) |
| M3 | 변수 치환, 태그, import/export |
| M4 | Claude skill (`ph skill install`) 과 문서 |

세부 작업은 [agent/TASK.md](agent/TASK.md) 에서 관리한다.

## 10. 미결정 사항 (Open Questions)

작업 착수 전에 사용자와 확정한다.

1. 배포 방식. 후보는 GitHub Release 바이너리, `cargo install prompt-hub`, cargo-binstall, Homebrew tap 이다. v0.1 은 앞의 두 가지를 제안한다.

확정됨
- 배포 패키지 이름은 `prompt-hub`, 바이너리 이름은 `ph` 다 (8절 라이선스 항목 참고).
- 프로젝트별 local 저장소와 global 저장소를 **둘 다 지원**한다 (3.1~3.2절).
- prompt 버전 이력은 v0.1 에서 **제외**한다 (1절 비목표).
- 클립보드가 없는 환경(SSH, WSL 등)의 대체 동작(OSC52 등)은 v0.1 에서 **고려하지 않는다.** 복사에 실패하면 상태바에 에러 메시지만 표시한다.
