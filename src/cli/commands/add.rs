//! `ph add`

use crate::cli::args::AddArgs;
use crate::cli::context::CliIo;
use crate::cli::output::{badge, to_json_line, AddJson, PromptJson, SCHEMA_VERSION};
use crate::core::error::PhError;
use crate::core::model::{NewPrompt, Scope};
use crate::core::service::PromptService;

/// prompt 를 추가한다.
pub fn run(args: AddArgs, svc: &PromptService, io: &mut CliIo) -> Result<(), PhError> {
    let body = read_body(&args, io)?;
    let written = svc.add(
        NewPrompt {
            title: args.title,
            body,
            tags: args.tags,
            description: args.description,
        },
        args.scope.target(),
    )?;
    if written.auto_selected {
        let loc = match written.scope {
            Scope::Local => svc.local_location().unwrap_or_default(),
            Scope::Global => svc.global_location(),
        };
        io.info(&format!(
            "저장됨: {} {} ({loc})",
            badge(written.scope),
            written.prompt.id
        ));
    }
    if args.json {
        let line = to_json_line(&AddJson {
            schema_version: SCHEMA_VERSION,
            prompt: PromptJson::new(&written.prompt),
            auto_selected: written.auto_selected,
        })?;
        io.out(&line)
    } else {
        io.out(&format!("{}\n", written.prompt.id))
    }
}

fn read_body(args: &AddArgs, io: &mut CliIo) -> Result<String, PhError> {
    if let Some(b) = &args.body {
        return Ok(b.clone());
    }
    let text = if let Some(path) = &args.file {
        let bytes =
            std::fs::read(path).map_err(|e| PhError::io(format!("{} 읽기", path.display()), e))?;
        String::from_utf8(bytes).map_err(|_| {
            PhError::Usage(format!(
                "{} 는 UTF-8 이 아닙니다. UTF-8 로 저장한 파일을 지정하세요",
                path.display()
            ))
        })?
    } else {
        let mut s = String::new();
        io.stdin
            .read_to_string(&mut s)
            .map_err(|e| PhError::io("표준 입력 읽기", e))?;
        s
    };
    Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_string())
}
