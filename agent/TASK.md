# TASK.md — 작업 현황

> 이 파일은 `team-orchestrator` 가 관리한다. 다른 agent 는 자기 작업의 상태와 메모만 갱신한다.
> 스펙: [../SPEC.md](../SPEC.md)

## 상태 표기
`[ ]` 대기 · `[~]` 진행 중 · `[x]` 완료 · `[!]` 막힘

## 현재 단계
**준비 완료, 작업 착수 전.** 사용자 승인 후 M0 부터 시작한다.

## 결정 사항
- 언어: Rust
- 테스트: Rust 내장 `cargo test` (2026-09-30 사용자 확정)
- TUI 프레임워크: ratatui + crossterm (2026-09-30 사용자 확정)
- 저장소 범위: global + 프로젝트별 local(`.ph/`) 둘 다 지원. 읽기는 병합(local 우선), 쓰기는 명시 또는 local 존재 시 local (2026-09-30 사용자 확정, SPEC 3절)
- 버전 이력: v0.1 제외. 필요하면 local 저장소를 git 으로 관리 (2026-09-30 사용자 확정)
- 클립보드 대체 동작(OSC52 등): v0.1 미고려, 실패 시 에러 표시만 (2026-09-30 사용자 확정)
- 지원 OS: v0.1 은 Linux, macOS. Windows 는 미지원이나 확장 가능하게 설계 (OS 종속 코드는 `platform/` 에 격리, SPEC 6절 이식성 규칙) (2026-09-30 사용자 확정)
- TUI 편집: 인라인 에디터(`e`)와 외부 에디터 호출(`E`) 두 방식 모두 지원 (2026-09-30 사용자 확정, SPEC 5.1절)
- id(파일명) 규칙: 한글은 그대로, 영문도 대소문자 그대로, 공백만 `-` 로 치환. 예: `코드 리뷰` → `코드-리뷰.md` (2026-09-30 사용자 확정, SPEC 2절)
- 이름: 배포 패키지(crate) `prompt-hub`, 바이너리 `ph` (2026-09-30 사용자 확정). crates.io 에서 `ph`, `prompthub`, `ph-cli` 는 이미 사용 중이고 `prompt-hub` 는 비어 있음을 확인함
- 라이선스: MIT, 저작권자 Hanjae Lee (2026-09-30 사용자 확정, 루트 `LICENSE`)
- 저장 형식: Markdown + TOML frontmatter

## 미결정 (SPEC.md 10절)
- [ ] 배포 방식 (패키지 이름은 확정. 채널은 GitHub Release + `cargo install` 제안)

## M0. 골격과 core
- [ ] (architect) crate 구조와 의존 방향 확정, `docs/ARCHITECTURE.md` 작성
- [ ] (coder) `cargo init`, 모듈 골격 생성 (`Cargo.toml`: package name `prompt-hub`, `[[bin]] name = "ph"`, license, authors. SPEC 8절 라이선스)
- [ ] (coder) `platform` 모듈 (`paths`, `fs` atomic write, `editor`)
- [ ] (coder) `core::model`, `core::error`, id 검증 규칙(SPEC 2절)
- [ ] (qa) 이식성 검증: id 규칙(예약어, 대소문자, NFC), CRLF/BOM 파싱, OS 종속 코드가 `platform/` 밖에 없는지 grep 점검
- [ ] (coder) `Storage` trait, fs 구현, memory 구현
- [ ] (coder) scope 모델(`Scope`), local 탐색(상위 디렉터리 탐색), `PromptService` 의 병합, 우선순위, 쓰기 대상 규칙
- [ ] (qa) core 와 storage 단위 테스트

## M1. CLI 핵심
- [ ] (coder) clap 정의, `init`, `add`, `get`, `list`, `search`, `rm`, `edit`, `move`, `--local`/`--global`
- [ ] (coder) `--json` 출력과 종료 코드
- [ ] (qa) CLI 통합 테스트 (`tests/`)

## M2. TUI
- [ ] (designer) `docs/DESIGN.md` 작성 (레이아웃, 색상, 키맵)
- [ ] (architect) 인라인 에디터 crate 선정 (`tui-textarea` 등, 한글 폭 처리와 undo 지원 확인)
- [ ] (coder) `AppState`, event 루프, view
- [ ] (coder) 인라인 에디터 (`e`): 필드 이동, 저장/취소, 변경 확인
- [ ] (coder) 외부 에디터 (`E`): TUI 중단/복구, 임시 파일, 검증 실패 시 재편집
- [ ] (qa) update 로직 테스트, `TestBackend` 렌더링 테스트
- [ ] (qa) 편집 테스트: 인라인(한글 입력, 취소 시 원본 유지, 변경 확인), 외부(가짜 에디터 스크립트로 성공/비정상 종료/깨진 frontmatter)

## M3. 변수 치환, 태그, import/export
- [ ] (coder) `core::template`
- [ ] (coder) `tag`, `export`, `import`
- [ ] (qa) 테스트

## M4. Claude skill
- [ ] (architect) skill 생성 구조 검토
- [ ] (coder) `ph skill install`
- [ ] (qa) 생성된 SKILL.md 검증

## 작업 로그
| 날짜 | 담당 | 내용 |
|---|---|---|
| 2026-09-30 | orchestrator | SPEC.md, CLAUDE.md, sub agent 정의, TASK.md 초안 작성 |
