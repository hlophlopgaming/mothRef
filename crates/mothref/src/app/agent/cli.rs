// SPDX-License-Identifier: GPL-3.0-or-later
//! Native subcommands and the JSON transport share the same typed operations.
use super::*;
use clap::{Arg, ArgAction, Command as CliCommand, FromArgMatches, Subcommand};
use std::ffi::{OsStr, OsString};

fn board_command(name: &str) -> bool {
    !matches!(
        name,
        "create" | "open" | "commands" | "capabilities" | "preview-latest"
    )
}
fn command() -> CliCommand {
    Operation::augment_subcommands(
        CliCommand::new("mothref")
            .version(env!("CARGO_PKG_VERSION"))
            .about("Native reference-board CLI. No command/file arguments starts the GUI.")
            .after_help("GUI: mothref [BOARD_OR_IMAGE ...] [--software]\nJSON-lines session: mothref --agent\nAll CLI operations run headless through the native journal/storage worker.")
            .subcommand_required(true)
            .arg(Arg::new("json").long("json").global(true).action(ArgAction::SetTrue).help("Machine-readable success and error replies"))
            .arg(Arg::new("dry_run").long("dry-run").global(true).action(ArgAction::SetTrue).help("Validate in private staging; do not save boards or export files"))
            .arg(Arg::new("output").long("output").short('o').global(true).value_parser(clap::value_parser!(PathBuf)).help("Save to a new .moth destination (required when converting .bee/.pur)")),
    ).mut_subcommands(|mut sub| {
        if board_command(sub.get_name()) {
            let names:Vec<_>=sub.get_positionals().map(|a|a.get_id().to_string()).collect();
            for (index,name) in names.iter().enumerate() {
                sub=sub.mut_arg(name,|arg|arg.index(index+2));
            }
            sub=sub.arg(Arg::new("board").value_name("BOARD").index(1).required(true).value_parser(clap::value_parser!(PathBuf)).help("Board to open before this operation"));
        }
        if sub.get_name()=="add" {
            sub=sub.arg(Arg::new("more_files").value_name("MORE_IMAGES").index(3).num_args(0..).conflicts_with("url").value_parser(clap::value_parser!(PathBuf)).help("Additional local images; automatic placement is used"));
        }
        sub
    }).subcommand(CliCommand::new("batch").about("Run a JSON operation array or {ops,dry_run} object; auto-save after success").arg(Arg::new("script").required(true).value_parser(clap::value_parser!(PathBuf)).help("Script filename, or - for stdin")))
}
pub(super) fn command_names() -> Vec<String> {
    command()
        .get_subcommands()
        .map(|c| c.get_name().to_owned())
        .collect()
}
/// File arguments retain GUI behavior; command names and CLI flags select headless mode.
pub(crate) fn recognizes(args: &[OsString]) -> bool {
    let Some(first) = args.get(1).and_then(|a| a.to_str()) else {
        return false;
    };
    if first == "--software" {
        return false;
    }
    first.starts_with('-')
        || first == "help"
        || command()
            .get_subcommands()
            .any(|c| c.get_name() == first || c.get_all_aliases().any(|a| a == first))
}
fn numbers<const N: usize>(value: &str) -> std::result::Result<[f32; N], String> {
    let values = value
        .split(',')
        .map(|v| {
            v.trim()
                .parse::<f32>()
                .map_err(|_| "expected comma-separated finite numbers".to_owned())
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if values.iter().any(|v| !v.is_finite()) {
        return Err("coordinates must be finite".into());
    }
    values
        .try_into()
        .map_err(|_| format!("expected {N} comma-separated numbers"))
}
pub(super) fn pair(value: &str) -> std::result::Result<[f32; 2], String> {
    numbers(value)
}
pub(super) fn rectangle(value: &str) -> std::result::Result<[f32; 4], String> {
    numbers(value)
}
pub(super) fn color(value: &str) -> std::result::Result<u32, String> {
    value
        .strip_prefix("0x")
        .map_or_else(|| value.parse(), |hex| u32::from_str_radix(hex, 16))
        .map_err(|e| e.to_string())
}
pub(super) fn flip(value: &str) -> std::result::Result<[bool; 2], String> {
    let (x, y) = value
        .split_once(',')
        .ok_or("expected true,false (horizontal,vertical)")?;
    Ok([
        x.trim().parse::<bool>().map_err(|e| e.to_string())?,
        y.trim().parse::<bool>().map_err(|e| e.to_string())?,
    ])
}
fn script(path: &std::path::Path) -> Result<Input> {
    let mut bytes = Vec::new();
    if path == std::path::Path::new("-") {
        std::io::stdin()
            .lock()
            .take(MAX_REQUEST + 1)
            .read_to_end(&mut bytes)?;
    } else {
        std::fs::File::open(path)?
            .take(MAX_REQUEST + 1)
            .read_to_end(&mut bytes)?;
    }
    ensure!(bytes.len() as u64 <= MAX_REQUEST, "script exceeds 4 MiB");
    let value: Value = serde_json::from_slice(&bytes)?;
    let input = if value.is_array() {
        Input {
            ops: serde_json::from_value(value)?,
            dry_run: false,
        }
    } else {
        serde_json::from_value(value)?
    };
    Ok(input)
}
fn execute(matches: &clap::ArgMatches) -> Result<Value> {
    let (name, sub) = matches.subcommand().context("command required")?;
    ensure!(
        !matches!(name, "undo" | "redo"),
        "undo/redo require a batch or a live --agent JSON session; history is not loaded from .moth files"
    );
    let output = matches.get_one::<PathBuf>("output").cloned();
    ensure!(
        output.is_none() || !matches!(name, "commands" | "capabilities" | "preview-latest"),
        "--output requires a board operation"
    );
    let mut input = if name == "batch" {
        script(
            sub.get_one::<PathBuf>("script")
                .context("script required")?,
        )?
    } else {
        let operation = Operation::from_arg_matches(matches)?;
        let mut ops = Vec::new();
        if board_command(name) {
            ops.push(Operation::Open {
                path: sub
                    .get_one::<PathBuf>("board")
                    .context("board required")?
                    .clone(),
            });
        }
        ops.push(operation);
        if name == "add"
            && let Some(files) = sub.get_many::<PathBuf>("more_files")
        {
            ensure!(
                matches!(ops.last(), Some(Operation::Add { at: None, .. })),
                "--at accepts one image; omit it to place multiple images safely"
            );
            let group_origin = matches!(
                ops.last(),
                Some(Operation::Add {
                    group_origin: true,
                    ..
                })
            );
            ops.extend(files.map(|path| Operation::Add {
                group_origin,
                path: Some(path.clone()),
                url: None,
                at: None,
            }));
        }
        Input {
            ops,
            dry_run: false,
        }
    };
    ensure!(
        !input.ops.is_empty() && input.ops.len() <= 1000,
        "batch needs 1..1000 operations"
    );
    input.dry_run |= matches.get_flag("dry_run");
    // A pathless explicit save can use --output without checkpointing the input first.
    if let Some(path) = &output {
        for operation in &mut input.ops {
            if let Operation::Save {
                path: destination @ None,
            } = operation
            {
                *destination = Some(path.clone());
            }
        }
    }
    let mutates = input.ops.iter().any(Operation::mutates);
    let save_empty = input
        .ops
        .iter()
        .any(|op| matches!(op, Operation::Create { .. }));
    let read_only = !mutates && output.is_none();
    let skip_setup = name != "batch" && board_command(name);
    let mut session = Session::new()?;
    let result = (|| -> Result<Value> {
        let mut results = Vec::new();
        let mut saved = false;
        for (index, operation) in input.ops.into_iter().enumerate() {
            let is_save = matches!(operation, Operation::Save { .. });
            let result = session.execute(operation, input.dry_run)?;
            saved |= is_save;
            if !(skip_setup && index == 0) {
                results.push(result);
            }
        }
        if mutates || output.is_some() {
            let destination = output
                .or_else(|| session.path.clone())
                .context("choose --output NEW.moth for imported/unnamed boards")?;
            if input.dry_run
                || session.dirty
                || (save_empty && !saved)
                || session.path.as_ref() != Some(&destination)
            {
                results.push(session.execute(
                    Operation::Save {
                        path: Some(destination),
                    },
                    input.dry_run,
                )?);
            }
        }
        if read_only {
            session.dirty = false;
        }
        Ok(json!({"protocol":1,"ok":true,"results":results,"dry_run":input.dry_run}))
    })();
    let finish = session.finish(result.is_ok(), input.dry_run);
    result.and_then(|value| finish.map(|()| value))
}
/// Returns an exit status after all worker cleanup, without initializing the GUI.
pub(crate) fn run(args: Vec<OsString>) -> i32 {
    let json = args.iter().any(|a| a == OsStr::new("--json"));
    let matches = match command().try_get_matches_from(args) {
        Ok(matches) => matches,
        Err(error) => {
            let status = error.exit_code();
            if json && status != 0 {
                let _ = serde_json::to_writer(
                    std::io::stdout().lock(),
                    &json!({"protocol":1,"ok":false,"error":error.to_string()}),
                );
            } else {
                let _ = error.print();
            }
            return status;
        }
    };
    let (reply, status) = match execute(&matches) {
        Ok(reply) => (reply, 0),
        Err(error) => (
            json!({"protocol":1,"ok":false,"error":format!("{error:#}"),"recovery":"Earlier unsaved operations remain in private recovery; explicit saves and exports are not rolled back."}),
            1,
        ),
    };
    let mut output = std::io::stdout().lock();
    let written = if json {
        serde_json::to_writer(&mut output, &reply)
    } else {
        serde_json::to_writer_pretty(&mut output, &reply)
    };
    if written.is_err() || output.write_all(b"\n").is_err() {
        return 1;
    }
    status
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_keeps_gui_file_arguments_and_recognizes_native_aliases() {
        for (args, native) in [
            (vec!["mothref"], false),
            (vec!["mothref", "board.moth"], false),
            (vec!["mothref", "image.png", "other.jpg"], false),
            (vec!["mothref", "--software"], false),
            (vec!["mothref", "./create"], false),
            (vec!["mothref", "arrange"], true),
            (vec!["mothref", "layout"], true),
            (vec!["mothref", "--json"], true),
            (vec!["mothref", "--help"], true),
        ] {
            let args: Vec<_> = args.into_iter().map(OsString::from).collect();
            assert_eq!(recognizes(&args), native, "{args:?}");
        }
    }
    #[test]
    fn command_graph_and_parsers_validate_before_opening_boards() {
        command().debug_assert();
        assert!(pair("1,2").is_ok());
        assert!(pair("NaN,2").is_err());
        assert!(rectangle("1,2,3").is_err());
        let matches = command()
            .try_get_matches_from([
                "mothref",
                "edit",
                "board.moth",
                "--id",
                "id",
                "--rotation",
                "0.5",
                "--flip",
                "true,false",
            ])
            .unwrap();
        assert!(matches!(
            Operation::from_arg_matches(&matches).unwrap(),
            Operation::Edit {
                transform: Some(_),
                ..
            }
        ));
    }
}
