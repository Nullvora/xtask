use anyhow::Result;
use strum::IntoEnumIterator;

use crate::{
    commands::WARN_IGNORED_ONLY_ARGS,
    endgroup, group,
    prelude::{Context, Environment},
    utils::{
        process::{run_process_for_package, run_process_for_workspace},
        workspace::{get_workspace_members, WorkspaceMember, WorkspaceMemberType},
    },
};

use super::Target;

#[tracel_xtask_macros::declare_command_args(Target, TestSubCommand)]
pub struct TestCmdArgs {}

pub fn handle_command(args: TestCmdArgs, env: Environment, _ctx: Context) -> anyhow::Result<()> {
    if args.target == Target::Workspace && !args.only.is_empty() {
        warn!("{}", WARN_IGNORED_ONLY_ARGS);
    }
    if !check_environment(&args, &env) {
        std::process::exit(1);
    }
    match args.get_command() {
        TestSubCommand::Unit => run_unit(&args.target, &args),
        TestSubCommand::Integration => run_integration(&args.target, &args),
        TestSubCommand::All => TestSubCommand::iter()
            .filter(|c| *c != TestSubCommand::All)
            .try_for_each(|c| {
                handle_command(
                    TestCmdArgs {
                        command: Some(c),
                        target: args.target.clone(),
                        exclude: args.exclude.clone(),
                        only: args.only.clone(),
                        threads: args.threads,
                        test: args.test.clone(),
                        jobs: args.jobs,
                        force: args.force,
                        features: args.features.clone(),
                        no_default_features: args.no_default_features,
                        no_capture: args.no_capture,
                    },
                    env.clone(),
                    _ctx.clone(),
                )
            }),
    }
}

/// Return true if the environment is OK.
/// Prevents from running test in production unless the `force` flag is set
pub fn check_environment(args: &TestCmdArgs, env: &Environment) -> bool {
    if *env == Environment::Production {
        if args.force {
            warn!("Force running tests in production (--force argument is set)");
            return true;
        } else {
            info!("Abort tests to avoid running them in production!");
            return false;
        }
    }
    true
}

fn push_optional_args(cmd_args: &mut Vec<String>, args: &TestCmdArgs) {
    // cargo options
    if let Some(jobs) = &args.jobs {
        cmd_args.extend(vec!["--jobs".to_string(), jobs.to_string()]);
    };
    if let Some(features) = &args.features {
        if !features.is_empty() {
            cmd_args.extend(vec!["--features".to_string(), features.join(",")]);
        }
    }
    if args.no_default_features {
        cmd_args.push("--no-default-features".to_string());
    }
    // test harness options
    cmd_args.extend(vec!["--".to_string(), "--color=always".to_string()]);
    if let Some(threads) = &args.threads {
        cmd_args.extend(vec!["--test-threads".to_string(), threads.to_string()]);
    };
    if args.no_capture {
        cmd_args.push("--nocapture".to_string());
    }
}

pub fn run_unit(target: &Target, args: &TestCmdArgs) -> Result<()> {
    match target {
        Target::Workspace => {
            info!("Workspace Unit Tests");
            let test = args.test.as_deref().unwrap_or("");
            let mut cmd_args = vec![
                "test",
                "--workspace",
                "--lib",
                "--bins",
                "--examples",
                test,
                "--color",
                "always",
            ]
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<String>>();
            push_optional_args(&mut cmd_args, args);
            run_process_for_workspace(
                "cargo",
                &cmd_args.iter().map(String::as_str).collect::<Vec<&str>>(),
                &args.exclude,
                Some(r".*target/[^/]+/deps/([^-\s]+)"),
                Some("Unit Tests"),
                "Workspace Unit Tests failed",
                Some("no library targets found"),
                Some("No library found to test for in workspace."),
            )?;
        }
        Target::Crates | Target::Examples => {
            let members = match target {
                Target::Crates => get_workspace_members(WorkspaceMemberType::Crate),
                Target::Examples => get_workspace_members(WorkspaceMemberType::Example),
                _ => unreachable!(),
            };

            for member in members {
                run_unit_test(&member, args)?;
            }
        }
        Target::AllPackages => {
            Target::iter()
                .filter(|t| *t != Target::AllPackages && *t != Target::Workspace)
                .try_for_each(|t| run_unit(&t, args))?;
        }
    }
    anyhow::Ok(())
}

fn run_unit_test(member: &WorkspaceMember, args: &TestCmdArgs) -> Result<(), anyhow::Error> {
    group!("Unit Tests: {}", member.name);
    let test = args.test.as_deref().unwrap_or("");
    let mut cmd_args = vec![
        "test",
        test,
        "--lib",
        "--bins",
        "--examples",
        "-p",
        &member.name,
        "--color=always",
    ]
    .into_iter()
    .map(|s| s.to_string())
    .collect::<Vec<String>>();
    push_optional_args(&mut cmd_args, args);
    run_process_for_package(
        "cargo",
        &member.name,
        &cmd_args.iter().map(String::as_str).collect::<Vec<&str>>(),
        &args.exclude,
        &args.only,
        &format!("Failed to execute unit test for '{}'", &member.name),
        Some("no library targets found"),
        Some(&format!(
            "No library found to test for in the crate '{}'.",
            &member.name
        )),
    )?;
    endgroup!();
    anyhow::Ok(())
}

pub fn run_integration(target: &Target, args: &TestCmdArgs) -> anyhow::Result<()> {
    match target {
        Target::Workspace => {
            info!("Workspace Integration Tests");
            let test = args.test.as_deref().unwrap_or("*");
            let mut cmd_args = vec!["test", "--workspace", "--test", test, "--color", "always"]
                .into_iter()
                .map(|s| s.to_string())
                .collect::<Vec<String>>();
            push_optional_args(&mut cmd_args, args);
            run_process_for_workspace(
                "cargo",
                &cmd_args.iter().map(String::as_str).collect::<Vec<&str>>(),
                &args.exclude,
                Some(r".*target/[^/]+/deps/([^-\s]+)"),
                Some("Integration Tests"),
                "Workspace Integration Tests failed",
                Some("no test target matches pattern"),
                Some("No tests found matching the pattern `test_*` in workspace."),
            )?;
        }
        Target::Crates | Target::Examples => {
            let members = match target {
                Target::Crates => get_workspace_members(WorkspaceMemberType::Crate),
                Target::Examples => get_workspace_members(WorkspaceMemberType::Example),
                _ => unreachable!(),
            };

            for member in members {
                run_integration_test(&member, args)?;
            }
        }
        Target::AllPackages => {
            Target::iter()
                .filter(|t| *t != Target::AllPackages && *t != Target::Workspace)
                .try_for_each(|t| run_integration(&t, args))?;
        }
    }
    anyhow::Ok(())
}

fn run_integration_test(member: &WorkspaceMember, args: &TestCmdArgs) -> Result<()> {
    group!("Integration Tests: {}", &member.name);
    let mut cmd_args = vec![
        "test",
        "--test",
        "*",
        "-p",
        &member.name,
        "--color",
        "always",
    ]
    .into_iter()
    .map(|s| s.to_string())
    .collect::<Vec<String>>();
    push_optional_args(&mut cmd_args, args);
    run_process_for_package(
        "cargo",
        &member.name,
        &cmd_args.iter().map(String::as_str).collect::<Vec<&str>>(),
        &args.exclude,
        &args.only,
        &format!("Failed to execute integration test for '{}'", &member.name),
        Some("no test target matches pattern"),
        Some(&format!(
            "No tests found matching the pattern `test_*` for '{}'.",
            &member.name
        )),
    )?;
    endgroup!();
    anyhow::Ok(())
}
