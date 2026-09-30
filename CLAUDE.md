# CLAUDE.md

`ph` (Prompt Hub) 는 AI Agent 용 prompt 저장소 CLI 프로그램이다. Rust 로 작성하고, 기본은 TUI 이며 CLI 와 Claude skill 로도 동작한다.

## 참조 문서

- 상세 스펙: [SPEC.md](SPEC.md) — 모든 구현은 이 문서를 기준으로 한다. 스펙과 구현이 다르면 먼저 스펙 변경을 제안한다.
- 작업 현황: [agent/TASK.md](agent/TASK.md) — 작업을 시작하거나 끝낼 때마다 갱신한다.
- TUI 디자인: [docs/DESIGN.md](docs/DESIGN.md) — Designer 가 작성한다.
- 아키텍처: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — Architect 가 작성한다. 모듈 경계, 공개 API, CLI 구조, ADR.

## Multi Agent 작업 규칙

이 프로젝트는 **반드시 Multi Agent 로 작업한다.** sub agent 정의는 `.claude/agents/` 에 있다.

| Agent | 파일 | 역할 |
|---|---|---|
| team-orchestrator | [.claude/agents/team-orchestrator.md](.claude/agents/team-orchestrator.md) | 작업 분배, 지시, 결과 보고 |
| designer | [.claude/agents/designer.md](.claude/agents/designer.md) | TUI 디자인 |
| software-architect | [.claude/agents/software-architect.md](.claude/agents/software-architect.md) | 구조 설계, 유지보수성 |
| coder | [.claude/agents/coder.md](.claude/agents/coder.md) | 서비스 로직 구현 |
| qa-engineer | [.claude/agents/qa-engineer.md](.claude/agents/qa-engineer.md) | unit test 작성과 결과 검증 |

- 메인 세션은 직접 구현하지 않고 `team-orchestrator` 에 위임한다.
- 작업 흐름: Architect(구조) → Designer(TUI) → Coder(구현) → QA(검증) → Orchestrator(보고).
- 각 agent 는 자기 역할 범위 밖의 파일을 수정하지 않는다.

## 개발 규칙 요약

- TUI 는 `ratatui` (+ `crossterm`) 로 구현한다.
- 지원 OS 는 Linux, macOS 이며 Windows 로 확장 가능해야 한다. OS 종속 코드는 `platform/` 에만 둔다 (SPEC.md 6절 이식성 규칙).
- 테스트는 Rust 내장 `cargo test` 를 사용한다.
- 작업 완료 조건: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` 모두 통과.
- 모듈 의존 방향은 SPEC.md 6절을 따른다. `core` 는 UI 와 IO 구현에 의존하지 않는다.
- 사용자와의 소통과 문서는 한국어로 쓴다. 코드 식별자는 영어로 쓴다.
