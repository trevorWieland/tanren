use cucumber::{given, then, when};
use tanren_testkit::{
    StandardsCliRunner, create_temp_project_dir, write_malformed_standard, write_project_config,
    write_valid_standard,
};

use crate::{StandardsContext, TanrenWorld};

#[given(expr = "a repository with installed standards including {string}")]
fn given_repo_with_standards(world: &mut TanrenWorld, standard_name: String) {
    let project_dir = create_temp_project_dir("standards").expect("create temp project dir");
    write_project_config(&project_dir, "standards").expect("write project config");
    let standards_dir = project_dir.join("standards");
    write_valid_standard(&standards_dir, standard_name).expect("write valid standard");
    world.standards = Some(StandardsContext {
        project_dir,
        last_result: None,
    });
}

#[given(expr = "a repository with standards at root {string} including {string}")]
fn given_repo_with_relocated_standards(
    world: &mut TanrenWorld,
    root: String,
    standard_name: String,
) {
    let project_dir =
        create_temp_project_dir("standards-relocated").expect("create temp project dir");
    write_project_config(&project_dir, &root).expect("write project config");
    let standards_dir = project_dir.join(&root);
    write_valid_standard(&standards_dir, standard_name).expect("write valid standard");
    let _ = (root,);
    world.standards = Some(StandardsContext {
        project_dir,
        last_result: None,
    });
}

#[given(expr = "a repository with a configured standards root but no standards directory")]
fn given_repo_missing_standards_dir(world: &mut TanrenWorld) {
    let project_dir =
        create_temp_project_dir("standards-missing").expect("create temp project dir");
    write_project_config(&project_dir, "standards").expect("write project config");
    world.standards = Some(StandardsContext {
        project_dir,
        last_result: None,
    });
}

#[given(expr = "a repository with a malformed standards file")]
fn given_repo_malformed_standard(world: &mut TanrenWorld) {
    let project_dir =
        create_temp_project_dir("standards-malformed").expect("create temp project dir");
    write_project_config(&project_dir, "standards").expect("write project config");
    let standards_dir = project_dir.join("standards");
    write_malformed_standard(&standards_dir, "broken".to_owned())
        .expect("write malformed standard");
    world.standards = Some(StandardsContext {
        project_dir,
        last_result: None,
    });
}

#[when(expr = "I inspect the installed standards")]
async fn when_inspect_standards(world: &mut TanrenWorld) {
    let ctx = world
        .standards
        .as_mut()
        .expect("standards context initialized");
    let runner = StandardsCliRunner::new().expect("locate tanren-cli binary");
    let result = runner.inspect(&ctx.project_dir).await;
    ctx.last_result = Some(result);
}

#[then(expr = "the command succeeds")]
fn then_command_succeeds(world: &mut TanrenWorld) {
    let ctx = world
        .standards
        .as_ref()
        .expect("standards context initialized");
    let result = ctx.last_result.as_ref().expect("command was executed");
    assert!(
        result.success,
        "expected success, got stderr: {}",
        result.stderr
    );
}

#[then(expr = "the command fails")]
fn then_command_fails(world: &mut TanrenWorld) {
    let ctx = world
        .standards
        .as_ref()
        .expect("standards context initialized");
    let result = ctx.last_result.as_ref().expect("command was executed");
    assert!(
        !result.success,
        "expected failure, got stdout: {}",
        result.stdout
    );
}

#[then(expr = "the output includes {string}")]
fn then_output_includes(world: &mut TanrenWorld, expected: String) {
    let ctx = world
        .standards
        .as_ref()
        .expect("standards context initialized");
    let result = ctx.last_result.as_ref().expect("command was executed");
    let combined = format!("{}\n{}", result.stdout, result.stderr);
    assert!(
        combined.contains(&expected),
        "expected output to contain {:?}, got stdout: {:?}, stderr: {:?}",
        expected,
        result.stdout,
        result.stderr,
    );
    let _ = (expected,);
}

#[then(expr = "the error output includes {string}")]
fn then_error_output_includes(world: &mut TanrenWorld, expected: String) {
    let ctx = world
        .standards
        .as_ref()
        .expect("standards context initialized");
    let result = ctx.last_result.as_ref().expect("command was executed");
    assert!(
        result.stderr.contains(&expected),
        "expected stderr to contain {:?}, got: {:?}",
        expected,
        result.stderr,
    );
    let _ = (expected,);
}
