// Copyright (C) 2024 Tristan Gerritsen <tristan@thewoosh.org>
// All Rights Reserved.

use std::{error::Error, path::Path, process::{Command, ExitStatus}};

use babbelaar::parse_string_to_tree;
use babbelaar_compiler::{Pipeline, Platform};
use temp_dir::TempDir;

#[test]
fn simple_return_0() {
    let result = create_and_run_single_object_executable("
        werkwijze hoofd() -> g32 {
            bekeer 123;
        }
    ");

    assert_eq!(result.signal, None);
    assert_eq!(result.exit_code, Some(123));
}

#[test]
fn simple_call_other_than_returns_100() {
    let result = create_and_run_single_object_executable("
        werkwijze a() -> g32 {
            bekeer b();
        }

        werkwijze hoofd() -> g32 {
            bekeer a();
        }

        werkwijze b() -> g32 {
            bekeer 100;
        }
    ");

    assert_eq!(result.signal, None);
    assert_eq!(result.exit_code, Some(100));
}

#[test]
fn method_call() {
    let result = create_and_run_single_object_executable("
        structuur MijnGeavanceerdeStructuur {
            werkwijze krijgGetal() -> g32 {
                bekeer 3;
            }
        }

        werkwijze hoofd() -> g32 {
            stel a = nieuw MijnGeavanceerdeStructuur {};
            bekeer a.krijgGetal();
        }
    ");

    assert_eq!(result.signal, None);
    assert_eq!(result.exit_code, Some(3));
}

fn create_and_run_single_object_executable(code: &str) -> ProgramResult {
    let dir = TempDir::new().unwrap().panic_on_cleanup_error();

    let executable = create_single_object_executable(code, &dir);
    let exit_status = run(executable).unwrap();

    let mut result = ProgramResult {
        exit_code: exit_status.code(),
        signal: None, // only set on UNIX-platforms below
    };

    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        result.signal = exit_status.signal();
    }

    result
}

fn create_single_object_executable(code: &str, dir: &TempDir) -> std::path::PathBuf {
    let tree = parse_string_to_tree(code).unwrap();

    let mut pipeline = Pipeline::new(Platform::host_platform());
    pipeline.compile_trees(&[tree]);
    pipeline.create_object(dir.path(), "BabBestand").unwrap();

    let executable = pipeline.link_to_executable(dir.path(), "BabUitvoerbare").unwrap();
    executable
}

fn run(path: impl AsRef<Path>) -> Result<ExitStatus, Box<dyn Error>> {
    let mut command = Command::new(path.as_ref());
    let mut process = command.spawn()?;
    Ok(process.wait()?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProgramResult {
    exit_code: Option<i32>,
    signal: Option<i32>,
}