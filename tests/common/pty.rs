//! 가짜 터미널(pty)에서 `ph` 를 실행하는 헬퍼. python3 의 `pty` 모듈을 쓴다 (Unix 전용).
//! python3 가 없으면 `None` 을 돌려주고 호출한 테스트는 건너뛴다.
#![allow(dead_code)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::cli::{Out, Sandbox};

const DRIVER: &str = r#"
import os, pty, sys, json, signal
spec = json.load(open(sys.argv[1]))
signal.alarm(60)
pid, fd = pty.fork()
if pid == 0:
    os.chdir(spec["cwd"])
    o = os.open(spec["stdout_path"], os.O_WRONLY | os.O_CREAT | os.O_TRUNC)
    os.dup2(o, 1)
    e = os.open(spec["stderr_path"], os.O_WRONLY | os.O_CREAT | os.O_TRUNC)
    os.dup2(e, 2)
    os.execve(spec["argv"][0], spec["argv"], spec["env"])
os.write(fd, spec["input"].encode())
while True:
    try:
        data = os.read(fd, 4096)
    except OSError:
        break
    if not data:
        break
_, status = os.waitpid(pid, 0)
sys.exit(os.waitstatus_to_exitcode(status))
"#;

pub fn python_available() -> bool {
    Command::new("python3")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// stdin 이 터미널인 환경에서 실행한다. `input` 은 미리 tty 에 써 둘 키 입력이다.
/// stdout 과 stderr 는 각각 파일로 받아 분리해 돌려준다.
pub fn run_tty(sb: &Sandbox, args: &[&str], input: &str, env: &[(&str, &str)]) -> Option<Out> {
    if !python_available() {
        eprintln!("python3 가 없어 pty 테스트를 건너뜁니다");
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    let out_path = dir.path().join("stdout.txt");
    let err_path = dir.path().join("stderr.txt");
    let driver = dir.path().join("driver.py");
    std::fs::write(&driver, DRIVER).unwrap();

    let mut argv = vec![env!("CARGO_BIN_EXE_ph").to_string()];
    argv.extend(args.iter().map(|s| s.to_string()));
    let mut envmap = serde_json::Map::new();
    envmap.insert("PH_HOME".into(), sb.ph_home.display().to_string().into());
    envmap.insert("HOME".into(), sb.home.display().to_string().into());
    envmap.insert("TERM".into(), "dumb".into());
    for (k, v) in env {
        envmap.insert((*k).into(), (*v).into());
    }
    let spec = serde_json::json!({
        "argv": argv,
        "env": envmap,
        "cwd": sb.proj.display().to_string(),
        "input": input,
        "stdout_path": out_path.display().to_string(),
        "stderr_path": err_path.display().to_string(),
    });
    let spec_path = dir.path().join("spec.json");
    std::fs::write(&spec_path, spec.to_string()).unwrap();

    let st = Command::new("python3")
        .arg(&driver)
        .arg(&spec_path)
        .env_clear()
        .status()
        .unwrap();
    Some(Out {
        code: st.code().unwrap_or(-1),
        stdout: std::fs::read_to_string(&out_path).unwrap_or_default(),
        stderr: std::fs::read_to_string(&err_path).unwrap_or_default(),
    })
}

/// 실행 가능한 셸 스크립트를 만든다. 경로에 공백이 없어야 한다 (`EDITOR` 는 공백으로 분리된다).
pub fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(!p.to_string_lossy().contains(' '));
    p
}
