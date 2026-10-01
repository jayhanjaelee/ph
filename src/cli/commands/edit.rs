//! `ph edit`

use crate::cli::args::EditArgs;
use crate::cli::context::CliIo;
use crate::cli::output::{ambiguous_warning, badge};
use crate::core::error::PhError;
use crate::core::service::{PromptService, ScopeFilter, WriteNote};
use crate::platform::editor::{resolve_editor, EditorExit, TempEditFile};

/// 외부 에디터로 편집한다. 검증에 실패하면 원본을 보존하고 다시 편집할지 묻는다.
pub fn run(args: EditArgs, svc: &PromptService, io: &mut CliIo) -> Result<(), PhError> {
    if !io.interactive {
        return Err(PhError::NonInteractive(
            "ph edit 은 터미널에서만 쓸 수 있습니다. 스크립트는 ph add/rm 을 쓰세요".to_string(),
        ));
    }
    let resolved = svc.get(&args.id, args.scope.filter())?;
    let id = resolved.prompt.id.as_str().to_string();
    if resolved.ambiguous {
        io.info(&ambiguous_warning(&id));
    }
    let only = ScopeFilter::Only(resolved.prompt.scope);
    let raw = svc.export_raw(&id, only)?;
    let tmp = TempEditFile::create(&id, &raw).map_err(|e| PhError::io("임시 파일 만들기", e))?;
    let cmd = resolve_editor(io.env);
    let program = cmd.program.to_string_lossy().into_owned();

    loop {
        match io.editor.run(&cmd, tmp.path()) {
            Err(e) => {
                return Err(PhError::Editor(format!(
                    "에디터를 실행하지 못했습니다 ('{program}'): {e}. $VISUAL 또는 $EDITOR 를 설정하세요"
                )))
            }
            Ok(EditorExit::Failed(code)) => {
                let what = match code {
                    Some(c) => format!("코드 {c}"),
                    None => "시그널".to_string(),
                };
                return Err(PhError::Editor(format!(
                    "에디터가 비정상 종료했습니다 ({what}). 변경은 반영되지 않았습니다"
                )));
            }
            Ok(EditorExit::Success) => {}
        }

        let saved = match tmp.read() {
            Ok(edited) => {
                if edited == raw {
                    io.info("변경 없음");
                    return Ok(());
                }
                svc.save_raw(&id, &edited, only)
            }
            Err(e) => Err(PhError::InvalidFormat {
                id: Some(id.clone()),
                line: None,
                reason: format!("편집한 파일을 읽지 못했습니다 ({e}). UTF-8 로 저장하세요"),
            }),
        };
        match saved {
            Ok(w) => {
                io.info(&format!("저장됨: {} {}", badge(w.scope), w.prompt.id));
                if w.notes.contains(&WriteNote::IdKeyIgnored) {
                    io.info("참고: id 는 변경되지 않습니다");
                }
                return Ok(());
            }
            Err(
                e @ (PhError::InvalidFormat { .. } | PhError::InvalidId { .. } | PhError::Usage(_)),
            ) => {
                io.info(&format!("오류: {e}"));
                io.info("원본 prompt 는 변경되지 않았습니다.");
                io.prompt("다시 편집할까요? [Y/n] ");
                let answer = io.read_line()?.to_lowercase();
                if answer == "n" || answer == "no" {
                    return Err(e);
                }
            }
            Err(e) => return Err(e),
        }
    }
}
