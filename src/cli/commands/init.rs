//! `ph init`

use crate::cli::args::InitArgs;
use crate::cli::context::CliIo;
use crate::cli::output::{to_json_line, InitJson, SCHEMA_VERSION};
use crate::core::error::PhError;
use crate::platform::paths::resolve_dirs;
use crate::storage::fs::init_local;

/// 현재 디렉터리에 local 저장소를 만든다.
pub fn run(args: InitArgs, io: &mut CliIo) -> Result<(), PhError> {
    // 홈을 알 수 없어도 init 은 동작해야 한다.
    let home = resolve_dirs(io.home_override).ok().and_then(|d| d.home);
    let out = init_local(io.cwd, home.as_deref())?;
    let path = out.ph_dir.display().to_string();
    if out.created {
        io.info(&format!("local 저장소를 만들었습니다: {path}"));
    } else {
        io.info(&format!("이미 초기화되어 있습니다: {path}"));
    }
    if let Some(parent) = &out.parent_local {
        io.info(&format!(
            "참고: 상위 {} 에도 local 저장소가 있습니다",
            parent.display()
        ));
    }
    if args.json {
        let line = to_json_line(&InitJson {
            schema_version: SCHEMA_VERSION,
            path,
            created: out.created,
        })?;
        io.out(&line)
    } else {
        io.out(&format!("{path}\n"))
    }
}
