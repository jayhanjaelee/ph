# TASK.md — 작업 현황

> 이 파일은 `team-orchestrator` 가 관리한다. 다른 agent 는 자기 작업의 상태와 메모만 갱신한다.
> 스펙: [../SPEC.md](../SPEC.md)

## 상태 표기
`[ ]` 대기 · `[~]` 진행 중 · `[x]` 완료 · `[!]` 막힘

## 현재 단계
**M1 완료.** M2(TUI) 는 사용자 확인 후 시작한다.

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

- SPEC 변경 승인 (2026-09-30 사용자): q/Esc 둘 다 종료, 검색 #태그 미지원(철회), COLORTERM/NO_COLOR 읽기, Enter=원문 복사, 수정 시 id 유지, tui-textarea→ratatui-textarea 정정
- 기본값 채택, 사용자 미확인: TUI 는 선택 항목 scope 기준 조작, 모드 목록에 TagInput/Help/Notice/Preview 추가
- 깨진 파일 get = InvalidFormat, local 없을 때 --local 읽기도 에러, 공백 파일명 skipped, 유니코드 공백 '-' 치환 (2026-09-30 사용자)
- 알려진 동작: 한 scope 의 깨진 파일 에러는 다른 scope 에 정상 항목이 있어도 get 에서 전파됨(우회: --global/--local)

## 미결정 (SPEC.md 10절)
- [ ] 배포 방식 (패키지 이름은 확정. 채널은 GitHub Release + `cargo install` 제안)

## M0. 골격과 core
- [x] (architect) crate 구조와 의존 방향 확정, `docs/ARCHITECTURE.md` 작성
- [x] (coder) `cargo init`, 모듈 골격 생성 (`Cargo.toml`: package name `prompt-hub`, `[[bin]] name = "ph"`, license, authors. SPEC 8절 라이선스)
- [x] (coder) `platform` 모듈 (`paths`, `fs` atomic write, `editor`)
- [x] (coder) `core::model`, `core::error`, id 검증 규칙(SPEC 2절)
- [x] (qa) 이식성 검증: id 규칙(예약어, 대소문자, NFC), CRLF/BOM 파싱, OS 종속 코드가 `platform/` 밖에 없는지 grep 점검
- [x] (coder) `Storage` trait, fs 구현, memory 구현
- [x] (coder) scope 모델(`Scope`), local 탐색(상위 디렉터리 탐색), `PromptService` 의 병합, 우선순위, 쓰기 대상 규칙
- [x] (qa) core 와 storage 단위 테스트

## M1. CLI 핵심
- [x] (coder) clap 정의, `init`, `add`, `get`, `list`, `search`, `rm`, `edit`, `move`, `--local`/`--global`
- [x] (coder) `--json` 출력과 종료 코드
- [x] (qa) CLI 통합 테스트 (`tests/`)

## M2. TUI
- [x] (designer) `docs/DESIGN.md` 작성 (레이아웃, 색상, 키맵) — SPEC 변경 제안 7건은 DESIGN.md 12절, orchestrator 승인 대기
- [x] (architect) 인라인 에디터 crate 선정 (`tui-textarea` 등, 한글 폭 처리와 undo 지원 확인) → `ratatui-textarea` 채택, 근거는 ARCHITECTURE.md 6절. M2 착수 시 coder 가 spike 로 한글 입력과 ratatui 버전 중복 확인
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
| 2026-09-30 | designer | `docs/DESIGN.md` 작성: 레이아웃/크기별 규칙, 색상 토큰(ANSI16·truecolor·mono), 키맵, 상태별 화면, 인라인 에디터(e/a), 외부 에디터(E) 흐름, TestBackend 검증 가이드. SPEC 변경 제안 7건(12절), architect 확인 요청 3건(13절) |
| 2026-09-30 | architect | `docs/ARCHITECTURE.md` 작성 (모듈, 의존 방향, 공개 API, 에러 전략, ADR). 인라인 에디터는 `tui-textarea`(ratatui 0.29 고정) 대신 `ratatui-textarea` 0.9 채택. SPEC 6절 crate 표 정정 요청 |
| 2026-09-30 | coder | M0 구현: `Cargo.toml`, `platform`(paths/fs/editor/time), `core`(model/error/id/format/service), `storage`(fs/memory), Scope 병합 |
| 2026-09-30 | qa | M0 검증: 이식성 grep 점검(platform 밖 OS 종속 코드 없음), core/storage 테스트 추가. 버그 2건 발견(frontmatter 구분자 손상, NFD 파일명 불일치) |
| 2026-09-30 | coder | BUG-1(직렬화 이스케이프, title 개행 거부), BUG-2(FsStorage resolve_path) 수정 |
| 2026-09-30 | orchestrator | M0 검증: fmt --check, clippy -D warnings, cargo test(113 passed) 통과 확인 |
| 2026-09-30 | architect | SPEC/CLAUDE.md/DESIGN.md/ARCHITECTURE.md 에 사용자 결정 반영, ARCHITECTURE 10절 M1 CLI 설계(ADR-11~14), SPEC crate 표 정리 |
| 2026-09-30 | coder | M1 구현: clap CLI(init/add/get/list/search/rm/edit/move), --json 스키마 v1, 종료 코드, `platform::env`, core 변경(InvalidFormat 전파, IdKeyIgnored) |
| 2026-09-30 | qa | M1 CLI 통합 테스트(pty 포함), 버그 없음 |
| 2026-09-30 | orchestrator | M1 검증: fmt --check, clippy -D warnings, cargo test 전부 통과(M0 재현 테스트 포함). architect 가 platform::env 등을 ARCHITECTURE 에 반영 |
| 2026-10-01 | orchestrator | main(708b32b) 병합: TASK.md 충돌 해소(M0 [x] 유지 + cargo init 문구 병합), coder 가 [package] name 을 prompt-hub 로 변경(bin/lib 은 ph 유지), fmt/clippy/test 통과 |
