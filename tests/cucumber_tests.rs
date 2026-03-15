use std::path::PathBuf;
use std::process::Command;

use cucumber::{World, given, then, when};

#[derive(Debug, Default, World)]
struct WelangWorld {
    program_path: Option<PathBuf>,
    exit_code: Option<i32>,
}

#[given(expr = "the welang program {string}")]
async fn given_program(world: &mut WelangWorld, name: String) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(name);
    world.program_path = Some(path);
}

#[when("I compile and run it")]
async fn compile_and_run(world: &mut WelangWorld) {
    let we = env!("CARGO_BIN_EXE_we");
    let source = world.program_path.as_ref().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    let compile_status = Command::new(we)
        .arg(source)
        .current_dir(tmp.path())
        .status()
        .unwrap();

    if !compile_status.success() {
        world.exit_code = Some(1);
        return;
    }

    let stem = source.file_stem().unwrap().to_str().unwrap();
    let binary = tmp.path().join(stem);

    let run_status = Command::new(&binary).status().unwrap();
    world.exit_code = Some(run_status.code().unwrap_or(1));
}

#[then("it should exit successfully")]
async fn should_succeed(world: &mut WelangWorld) {
    assert_eq!(
        world.exit_code,
        Some(0),
        "program {:?} exited with {:?}",
        world.program_path,
        world.exit_code,
    );
}

#[tokio::main]
async fn main() {
    WelangWorld::run("tests/features").await;
}
