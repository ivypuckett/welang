use std::path::PathBuf;
use std::process::Command;

use cucumber::{World, gherkin::Step, given, then, when};

#[derive(Debug, Default, World)]
struct WelangWorld {
    program_path: Option<PathBuf>,
    exit_code: Option<i32>,
    definitions: String,
    expression: Option<String>,
}

fn run_source(source: &str) -> i32 {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("test.we");
    std::fs::write(&src, source).unwrap();
    let we = env!("CARGO_BIN_EXE_we");
    if !Command::new(we)
        .arg(&src)
        .current_dir(tmp.path())
        .status()
        .unwrap()
        .success()
    {
        return 1;
    }
    Command::new(tmp.path().join("test"))
        .status()
        .unwrap()
        .code()
        .unwrap_or(1)
}

fn compile_source(source: &str) -> bool {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("test.we");
    std::fs::write(&src, source).unwrap();
    let we = env!("CARGO_BIN_EXE_we");
    Command::new(we)
        .arg(&src)
        .current_dir(tmp.path())
        .status()
        .unwrap()
        .success()
}

// ── Given ────────────────────────────────────────────────────────────────────

#[given(expr = "the welang program {string}")]
async fn given_program(world: &mut WelangWorld, name: String) {
    world.program_path = Some(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join(name),
    );
}

/// Store an inline welang expression for use with `Then it should evaluate to`.
#[given(expr = "the welang expression {string}")]
async fn given_expression(world: &mut WelangWorld, expr: String) {
    world.expression = Some(expr);
}

/// Store multiline welang function definitions (docstring) used by evaluate/call steps.
#[given("the welang definitions:")]
async fn given_definitions(world: &mut WelangWorld, step: &Step) {
    world.definitions = step.docstring().map_or("", |v| v).to_string();
}

// ── When ─────────────────────────────────────────────────────────────────────

#[when("I compile and run it")]
async fn compile_and_run(world: &mut WelangWorld) {
    let we = env!("CARGO_BIN_EXE_we");
    let source = world.program_path.as_ref().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    if !Command::new(we)
        .arg(source)
        .current_dir(tmp.path())
        .status()
        .unwrap()
        .success()
    {
        world.exit_code = Some(1);
        return;
    }
    let stem = source.file_stem().unwrap().to_str().unwrap();
    world.exit_code = Some(
        Command::new(tmp.path().join(stem))
            .status()
            .unwrap()
            .code()
            .unwrap_or(1),
    );
}

// ── Then ─────────────────────────────────────────────────────────────────────

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

/// Compile `definitions + main: {(equal [expr, N]): 0, _: 1}` and assert exit 0.
#[then(expr = "it should evaluate to {int}")]
async fn should_evaluate_to(world: &mut WelangWorld, expected: i64) {
    let expr = world.expression.as_deref().expect("no expression set");
    let src = format!(
        "{}\nmain: {{(equal [{}, {}]): 0, _: 1}}",
        world.definitions, expr, expected
    );
    assert_eq!(
        run_source(&src),
        0,
        "`{}` did not evaluate to {}",
        expr,
        expected
    );
}

/// Compile `definitions + main: {(equal [(func input), expected]): 0, _: 1}` and assert exit 0.
#[then(expr = "calling {string} with {int} should return {int}")]
async fn calling_with_should_return(
    world: &mut WelangWorld,
    func: String,
    input: i64,
    expected: i64,
) {
    let src = format!(
        "{}\nmain: {{(equal [({} {}), {}]): 0, _: 1}}",
        world.definitions, func, input, expected
    );
    assert_eq!(
        run_source(&src),
        0,
        "{}({}) did not return {}",
        func,
        input,
        expected
    );
}

/// Compile `definitions + main: 0` and assert compilation succeeds.
#[then("it should compile successfully")]
async fn should_compile_successfully(world: &mut WelangWorld) {
    let src = format!("{}\nmain: 0", world.definitions);
    assert!(compile_source(&src), "did not compile:\n{}", src);
}

#[tokio::main]
async fn main() {
    WelangWorld::run("tests/features").await;
}
