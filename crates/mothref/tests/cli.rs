// SPDX-License-Identifier: GPL-3.0-or-later
//! Exercise the shipped binary, headlessly and without a Python installation.
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

struct Native {
    root: PathBuf,
    board: PathBuf,
}
impl Native {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("mothref-native-cli-{}", moth_scene::Uuid::now_v7()));
        fs::create_dir_all(&root).unwrap();
        Self {
            board: root.join("references with spaces.moth"),
            root,
        }
    }
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_mothref"));
        command
            .env("MOTHREF_AGENT_DATA", self.root.join("recovery"))
            .env("PATH", "")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY");
        command
    }
    fn invoke(&self, args: &[&str]) -> Output {
        self.command().arg("--json").args(args).output().unwrap()
    }
    fn op(&self, name: &str, args: &[&str]) -> Value {
        let output = self
            .command()
            .arg("--json")
            .arg(name)
            .arg(&self.board)
            .args(args)
            .output()
            .unwrap();
        success(output)
    }
    fn info(&self) -> Value {
        self.op("info", &[])["results"][0].clone()
    }
    fn image(&self) -> PathBuf {
        corpus("images/rows.png")
    }
    fn agent(&self, requests: &[Value]) -> Output {
        let mut child = self
            .command()
            .arg("--agent")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        {
            let mut stdin = child.stdin.take().unwrap();
            for request in requests {
                serde_json::to_writer(&mut stdin, request).unwrap();
                stdin.write_all(b"\n").unwrap();
            }
        }
        child.wait_with_output().unwrap()
    }
}
impl Drop for Native {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn success(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let reply: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(reply["ok"], true, "{reply}");
    reply
}
fn failure(output: Output) -> Value {
    assert!(
        !output.status.success(),
        "unexpected success: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let reply: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(reply["ok"], false);
    reply
}
fn corpus(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus")
        .join(path)
}
fn separate(a: &Value, b: &Value) -> bool {
    let a: [f64; 4] = serde_json::from_value(a.clone()).unwrap();
    let b: [f64; 4] = serde_json::from_value(b.clone()).unwrap();
    a[0] + a[2] <= b[0] + 0.001
        || b[0] + b[2] <= a[0] + 0.001
        || a[1] + a[3] <= b[1] + 0.001
        || b[1] + b[3] <= a[1] + 0.001
}
fn read_json(path: impl AsRef<Path>) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn native_help_and_catalog_cover_the_compiled_actions() {
    let cli = Native::new();
    let output = cli.command().arg("--help").output().unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains("arrange") && help.contains("batch") && help.contains("--agent"));
    let catalog = success(cli.invoke(&["commands"]));
    let catalog = &catalog["results"][0];
    let actual = success(cli.invoke(&["capabilities"]));
    let actions = actual["results"][0]["gui_actions"].as_array().unwrap();
    let mappings = catalog["gui_actions"].as_object().unwrap();
    assert_eq!(actions.len(), mappings.len());
    for action in actions {
        let entry = &mappings[action.as_str().unwrap()];
        if let Some(operation) = entry["operation"].as_str() {
            assert!(
                catalog["operations"].get(operation).is_some(),
                "{action}: unknown operation {operation}"
            );
        } else {
            assert!(
                entry["gui_only"]
                    .as_str()
                    .is_some_and(|reason| !reason.is_empty())
            );
        }
    }
    let documented: std::collections::BTreeSet<_> = catalog["operations"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, value)| value["cli_name"].as_str().unwrap_or(name).to_owned())
        .chain(std::iter::once("batch".to_owned()))
        .collect();
    let compiled: std::collections::BTreeSet<String> =
        serde_json::from_value(catalog["command_names"].clone()).unwrap();
    assert_eq!(
        documented, compiled,
        "Document every new typed operation/command"
    );
    assert!(
        catalog["agent_guide"]
            .as_str()
            .unwrap()
            .contains("mothref create")
    );
    for (name, _) in catalog["operations"].as_object().unwrap() {
        let output = cli
            .command()
            .args([name.as_str(), "--help"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn create_multiple_images_arrange_and_export_without_python() {
    let cli = Native::new();
    cli.op("create", &[]);
    let image = cli.image();
    let path = image.to_str().unwrap();
    let result = cli.op("add", &[path, path]);
    let first = &result["results"][0];
    let second = &result["results"][1];
    assert!(separate(
        &first["item"]["bounds"],
        &second["item"]["bounds"]
    ));
    cli.op("note", &["Reference notes"]);
    cli.op("arrange", &["--mode", "grid", "--gap", "24"]);
    let info = cli.info();
    assert_eq!(info["items_total"], 3);
    assert_eq!(info["coordinates"]["space"], "board");
    assert_eq!(cli.op("check", &[])["results"][0]["valid"], true);
    for extension in ["png", "svg", "pdf"] {
        let file = cli.root.join(format!("scene.{extension}"));
        cli.op("export", &[file.to_str().unwrap(), "--side", "320"]);
        let bytes = fs::read(&file).unwrap();
        match extension {
            "png" => {
                assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
                let decoded = moth_io::decode(&bytes, 320).unwrap();
                assert_eq!(*decoded.original.iter().max().unwrap(), 320);
                assert!(
                    decoded.mips[0]
                        .bytes
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .any(|p| p[3] > 0)
                );
                assert!(decoded.mips[0].bytes.windows(2).any(|p| p[0] != p[1]));
            }
            "svg" => {
                let svg = String::from_utf8(bytes).unwrap();
                assert!(
                    svg.contains("<svg") && svg.contains("<image") && svg.contains("Reference")
                );
            }
            _ => {
                assert!(bytes.starts_with(b"%PDF-") && bytes.ends_with(b"%%EOF\n"));
                assert!(bytes.windows(15).any(|w| w == b"/Subtype /Image"));
            }
        }
    }
    let id = first["id"].as_str().unwrap();
    let original = cli.root.join("original.png");
    cli.op(
        "export",
        &[original.to_str().unwrap(), "--kind", "original", "--id", id],
    );
    assert_eq!(fs::read(original).unwrap(), fs::read(&image).unwrap());
    let folder = cli.root.join("originals");
    cli.op("export", &[folder.to_str().unwrap(), "--kind", "originals"]);
    assert_eq!(
        fs::read_dir(&folder).unwrap().count(),
        info["blobs_total"].as_u64().unwrap() as usize
    );
    for entry in fs::read_dir(folder).unwrap() {
        assert_eq!(
            fs::read(entry.unwrap().path()).unwrap(),
            fs::read(&image).unwrap()
        );
    }
    let archive = cli.root.join("originals.zip");
    cli.op("export", &[archive.to_str().unwrap(), "--kind", "archive"]);
    let bytes = fs::read(archive).unwrap();
    assert!(bytes.starts_with(b"PK\x03\x04"));
    let original = fs::read(image).unwrap();
    assert!(bytes.windows(original.len()).any(|chunk| chunk == original));
}

#[test]
fn all_layout_modes_and_scene_edits_use_native_flags() {
    let cli = Native::new();
    cli.op("create", &[]);
    let image = cli.image();
    let added = cli.op("add", &[image.to_str().unwrap()]);
    let a = added["results"][0]["id"].as_str().unwrap().to_owned();
    // The board keeps where the image came from, as an absolute path.
    assert_eq!(
        added["results"][0]["item"]["source"]["file"],
        json!(std::path::absolute(&image).unwrap().to_str().unwrap())
    );
    assert_eq!(
        cli.op("pixel-info", &["--id", &a, "--point", "0.5,0.5"])["results"][0]["pixel"],
        json!([0, 0])
    );
    assert!(
        cli.op("sample", &["--id", &a, "--point", "0.5,0.5"])["results"][0]["message"]
            .as_str()
            .unwrap()
            .starts_with('#')
    );
    let b = cli.op("note", &["caption"])["results"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    for mode in ["horizontal", "vertical", "grid", "optimal"] {
        cli.op("arrange", &["--mode", mode]);
    }
    cli.op("arrange", &["--mode", "grid", "--order", "path"]);
    let card = cli.op("gamut", &["--id", &a, "--view", "shares", "--sort", "warm"]);
    let card = &card["results"][0]["item"];
    assert_eq!(card["kind"], "gamut");
    assert_eq!(card["content"]["image"], json!(a));
    assert_eq!(card["content"]["view"], 2);
    assert!(!card["content"]["colors"].as_array().unwrap().is_empty());
    // Given the card, the image is measured again and the card keeps its view.
    let card_id = card["id"].as_str().unwrap().to_owned();
    let again = cli.op("gamut", &["--id", &card_id]);
    assert_eq!(again["results"][0]["item"]["content"]["view"], 2);
    assert_eq!(again["results"][0]["item"]["content"]["sort"], 1);
    for mode in [
        "left",
        "right",
        "top",
        "bottom",
        "horizontal",
        "vertical",
        "stack",
    ] {
        cli.op("align", &["--mode", mode]);
    }
    for mode in ["width", "height", "size", "area"] {
        cli.op("normalize", &["--mode", mode]);
    }
    cli.op(
        "arrange",
        &[
            "--mode",
            "horizontal",
            "--count",
            "2",
            "--gap",
            "12",
            "--order",
            "name",
        ],
    );
    cli.op("scale", &["--factor", "0.5"]);
    cli.op("move", &["--by", "20,-30"]);
    cli.op(
        "edit",
        &[
            "--id",
            &a,
            "--rect",
            "-120,-70,80,160",
            "--rotation",
            "0.6",
            "--flip",
            "true,false",
            "--opacity",
            "0.8",
            "--grayscale",
            "true",
            "--sampling",
            "2",
            "--name",
            "renamed.png",
            "--content-scale",
            "2",
            "--content-offset",
            "0.1,0.2",
            "--content-rotation",
            "0.2",
        ],
    );
    cli.op(
        "edit",
        &[
            "--id",
            &b,
            "--text",
            "edited caption",
            "--note-font-size",
            "16",
            "--note-wrap",
            "true",
        ],
    );
    let group = cli.op("group", &["--id", &a, &b, "--title", "Group"])["results"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    cli.op(
        "group-edit",
        &[
            "--id",
            &group,
            "--title",
            "RGB group",
            "--color",
            "0x0112ab34",
        ],
    );
    let before = cli.info();
    let copies = cli.op("duplicate", &["--id", &group]);
    assert_eq!(
        copies["results"][0]["inserted"].as_array().unwrap().len(),
        3
    );
    for copy in copies["results"][0]["changed"].as_array().unwrap() {
        assert!(separate(&before["bounds"], &copy["bounds"]));
    }
    let ids = copies["results"][0]["inserted"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap())
        .collect::<Vec<_>>();
    let mut args = vec!["--id"];
    args.extend(ids);
    cli.op("delete", &args);
    cli.op("parent", &["--id", &b]);
    cli.op("ungroup", &["--id", &group]);
    for mode in ["raise", "lower", "top", "bottom"] {
        cli.op("order", &["--id", &a, "--mode", mode]);
    }
    for mode in ["scale", "normalize", "crop", "all"] {
        cli.op("reset", &["--id", &a, "--mode", mode]);
    }
    let info = cli.info();
    let transformed = info["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == a)
        .unwrap();
    assert_eq!(transformed["transform"]["grayscale"], true);
    assert_eq!(transformed["transform"]["sampling"], 2);
    assert!((transformed["transform"]["opacity"].as_f64().unwrap() - 0.8).abs() < 1e-6);
    assert_eq!(info["items_total"], 3, "image, note and gamut card");
    assert!(
        info["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["name"] == "renamed.png")
    );
}

#[test]
fn explicit_zero_coordinates_and_dry_run_are_preserved() {
    let cli = Native::new();
    cli.op("create", &[]);
    let a = cli.op("note", &["first", "--at", "0,0"]);
    let b = cli.op("note", &["second", "--at", "0,0"]);
    assert_eq!(
        a["results"][0]["item"]["rect"][0],
        b["results"][0]["item"]["rect"][0]
    );
    let id = a["results"][0]["id"].as_str().unwrap();
    let copied = cli.op("duplicate", &["--id", id, "--by", "0,0"]);
    assert_eq!(
        copied["results"][0]["changed"][0]["rect"],
        a["results"][0]["item"]["rect"]
    );
    let before = fs::read(&cli.board).unwrap();
    cli.op("note", &["discard", "--dry-run"]);
    assert_eq!(fs::read(&cli.board).unwrap(), before);
    assert_eq!(cli.info()["items_total"], 3);
}

#[test]
fn flags_errors_and_existing_destinations_never_save_changes() {
    let cli = Native::new();
    cli.op("create", &[]);
    let before = fs::read(&cli.board).unwrap();
    let bad = cli.invoke(&["move", cli.board.to_str().unwrap(), "--by", "NaN,2"]);
    assert!(failure(bad)["error"].as_str().unwrap().contains("finite"));
    let bad = cli.invoke(&[
        "edit",
        cli.board.to_str().unwrap(),
        "--id",
        "bad-id",
        "--rect",
        "0,0,10,10",
    ]);
    assert!(failure(bad)["error"].as_str().unwrap().contains("UUID"));
    failure(cli.invoke(&["create", cli.board.to_str().unwrap()]));
    assert_eq!(fs::read(&cli.board).unwrap(), before);
    let note = cli.op("note", &["safe"]);
    assert!(note["ok"].as_bool().unwrap());
    let export = cli.root.join("keep.svg");
    fs::write(&export, b"user output").unwrap();
    failure(cli.invoke(&[
        "export",
        cli.board.to_str().unwrap(),
        export.to_str().unwrap(),
    ]));
    assert_eq!(fs::read(export).unwrap(), b"user output");
    assert!(!success(cli.invoke(&["commands"]))["results"][0].is_null());
}

#[test]
fn batch_autosaves_and_raw_json_lines_keep_stateful_undo_redo() {
    let cli = Native::new();
    let batch = cli.root.join("script.json");
    fs::write(
        &batch,
        serde_json::to_vec(&json!([
            {"op":"create","path":cli.board}, {"op":"note","text":"batch note"},
            {"op":"undo"}, {"op":"redo"}, {"op":"info"}
        ]))
        .unwrap(),
    )
    .unwrap();
    let reply = success(cli.invoke(&["batch", batch.to_str().unwrap()]));
    assert_eq!(reply["results"][4]["items_total"], 1);
    assert_eq!(cli.info()["items_total"], 1);
    let raw = cli.root.join("raw.moth");
    let output = cli.agent(&[
        json!({"ops":[{"op":"create","path":raw},{"op":"note","text":"raw"}]}),
        json!({"ops":[{"op":"undo"},{"op":"info"}]}),
        json!({"ops":[{"op":"redo"},{"op":"save"},{"op":"check"}]}),
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let replies: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(replies.len(), 3);
    assert_eq!(replies[1]["results"][1]["items_total"], 0);
    assert_eq!(replies[2]["results"][2]["valid"], true);
    assert!(raw.is_file());
}

#[test]
fn failed_batch_and_raw_error_keep_unsaved_recovery() {
    let cli = Native::new();
    cli.op("create", &[]);
    let before = fs::read(&cli.board).unwrap();
    let script = cli.root.join("bad.json");
    fs::write(&script,serde_json::to_vec(&json!([{"op":"open","path":cli.board},{"op":"note","text":"recover me"},{"op":"move","delta":[1,2],"ids":["bad"]}])).unwrap()).unwrap();
    failure(cli.invoke(&["batch", script.to_str().unwrap()]));
    assert_eq!(fs::read(&cli.board).unwrap(), before);
    let journals = fs::read_dir(cli.root.join("recovery"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().join("board.moth").is_file())
        .count();
    assert!(journals > 0);
    let output = cli.agent(&[json!({"ops":[{"op":"unknown"}]})]);
    assert!(!output.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["ok"],
        false
    );
}

#[test]
fn raw_transport_does_not_autosave_and_native_cli_handles_stdin_batches() {
    let cli = Native::new();
    let output = cli.agent(&[
        json!({"ops":[{"op":"create","path":cli.board},{"op":"note","text":"unsaved raw"}]}),
    ]);
    assert!(output.status.success());
    assert!(!cli.board.exists());
    let mut child = cli
        .command()
        .args(["--json", "batch", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    serde_json::to_writer(
        child.stdin.take().unwrap(),
        &json!({"ops":[{"op":"create","path":cli.board},{"op":"note","text":"stdin"}]}),
    )
    .unwrap();
    success(child.wait_with_output().unwrap());
    assert_eq!(cli.info()["items_total"], 1);
}

#[test]
fn foreign_boards_support_read_only_inspection_and_native_conversion() {
    for name in [
        "moth/v8.moth",
        "moth/v9.moth",
        "moth/v10.moth",
        "moth/v11.moth",
        "bee/tiny.bee",
        "pur/v2.1-redbar.pur",
    ] {
        let cli = Native::new();
        let source = corpus(name);
        let before = fs::read(&source).unwrap();
        let info = success(cli.invoke(&["info", source.to_str().unwrap()]));
        assert!(info["results"][0]["items_total"].as_u64().unwrap() > 0);
        success(cli.invoke(&[
            "open",
            source.to_str().unwrap(),
            "--output",
            cli.board.to_str().unwrap(),
        ]));
        assert_eq!(cli.op("check", &[])["results"][0]["valid"], true);
        assert_eq!(fs::read(source).unwrap(), before);
    }
}

#[test]
fn preview_bundles_report_native_pixels_world_coordinates_and_latest() {
    let cli = Native::new();
    cli.op("create", &[]);
    cli.op("note", &["Preview content", "--at", "-200,100"]);
    let root = cli.root.join("previews");
    let reply = cli.op("preview", &[root.to_str().unwrap(), "--side", "256"]);
    let bundle = &reply["results"][0];
    assert_eq!(bundle["protocol_version"], "preview-bundle/v1");
    assert_eq!(bundle["bundle_kind"], "capture");
    assert!(bundle["created_at"].as_str().unwrap().ends_with('Z'));
    let summary = read_json(bundle["_summary_path"].as_str().unwrap());
    let mapping = &bundle["artifacts"][0]["coordinates"];
    assert_eq!(summary["preview"], *mapping);
    let image = Path::new(bundle["_bundle_dir"].as_str().unwrap())
        .join(bundle["artifacts"][0]["path"].as_str().unwrap());
    let bytes = fs::read(image).unwrap();
    let decoded = moth_io::decode(&bytes, 256).unwrap();
    assert_eq!(mapping["pixel_size"], json!(decoded.original));
    assert_eq!(mapping["board_bounds"], summary["bounds"]);
    let latest = success(cli.invoke(&["preview-latest", root.to_str().unwrap()]));
    assert_eq!(latest["results"][0]["bundle_id"], bundle["bundle_id"]);
}

#[test]
fn invalid_image_url_fails_without_touching_board() {
    let cli = Native::new();
    cli.op("create", &[]);
    let before = fs::read(&cli.board).unwrap();
    failure(cli.invoke(&[
        "add",
        cli.board.to_str().unwrap(),
        "--url",
        "file:///etc/passwd",
    ]));
    assert_eq!(fs::read(&cli.board).unwrap(), before);
}

#[test]
fn url_download_uses_explicit_native_http_backend() {
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let bytes = fs::read(corpus("images/rows.png")).unwrap();
    let server = std::thread::spawn(move || {
        let (mut connection, _) = listener.accept().unwrap();
        let mut buffer = [0u8; 4096];
        let _ = connection.read(&mut buffer).unwrap();
        write!(connection,"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",bytes.len()).unwrap();
        connection.write_all(&bytes).unwrap();
    });
    let cli = Native::new();
    cli.op("create", &[]);
    cli.op(
        "add",
        &["--url", &format!("http://{address}/reference.png")],
    );
    server.join().unwrap();
    assert_eq!(cli.info()["items_total"], 1);
}

#[test]
fn auto_insert_accounts_for_rotation_groups_and_global_paginated_bounds() {
    let cli = Native::new();
    cli.op("create", &[]);
    let image = cli.image();
    let id = cli.op("add", &[image.to_str().unwrap()])["results"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    cli.op(
        "edit",
        &[
            "--id",
            &id,
            "--rect",
            "-120,-70,80,160",
            "--rotation",
            "0.6",
        ],
    );
    cli.op("group", &["--id", &id, "--title", "Existing"]);
    let before = cli.info();
    let added = cli.op("note", &["beside rotated group"]);
    assert!(separate(
        &before["bounds"],
        &added["results"][0]["item"]["bounds"]
    ));
    let after = cli.info();
    for item in before["items"].as_array().unwrap() {
        assert!(after["items"].as_array().unwrap().iter().any(|i| i == item));
    }
    let paged = cli.op("info", &["--offset", "1", "--limit", "1"]);
    assert_eq!(paged["results"][0]["bounds"], after["bounds"]);
    assert_eq!(paged["results"][0]["items"].as_array().unwrap().len(), 1);
    let added = cli.op("add", &[image.to_str().unwrap()]);
    assert!(separate(
        &after["bounds"],
        &added["results"][0]["item"]["bounds"]
    ));
}

#[test]
fn labels_and_lists_keep_their_kind_and_text_after_reopen() {
    let cli = Native::new();
    cli.op("create", &[]);
    cli.op("note", &["--kind", "label", "# Label"]);
    cli.op("note", &["--kind", "list", "First\nSecond"]);
    let info = cli.info();
    let items = info["items"].as_array().unwrap();
    assert_eq!(items[0]["transform"]["note_kind"], "label");
    assert_eq!(items[0]["transform"]["note_markup"], false);
    assert_eq!(items[0]["content"]["text"], "# Label");
    assert_eq!(items[1]["transform"]["note_kind"], "list");
    assert_eq!(items[1]["content"]["text"], "- First\n- Second");
}

#[test]
fn annotation_commands_persist_strokes_and_reflow_scaled_cards() {
    let cli = Native::new();
    cli.op("create", &[]);
    let note = cli.op("note", &["# Costume study\n- [ ] Add warm light"]);
    let note = &note["results"][0]["item"];
    let width = note["rect"][2].as_f64().unwrap();
    let height = note["rect"][3].as_f64().unwrap();
    let id = note["id"].as_str().unwrap();
    cli.op("edit", &["--id", id, "--note-font-size", "48"]);
    let stroke = cli.op(
        "stroke",
        &[
            "--style", "arrow", "--point", "0,0", "--point", "30,60", "--color", "0x159954",
        ],
    );
    let stroke = &stroke["results"][0]["item"];
    assert_eq!(stroke["kind"], "stroke");
    assert_eq!(stroke["content"]["color"], json!([21, 153, 84]));
    let reopened = cli.info();
    let items = reopened["items"].as_array().unwrap();
    let reflowed = items.iter().find(|item| item["id"] == id).unwrap();
    assert_eq!(reflowed["transform"]["note_markup"], true);
    assert!(reflowed["rect"][2].as_f64().unwrap() > width * 1.8);
    assert!(reflowed["rect"][3].as_f64().unwrap() > height * 1.8);
    assert_eq!(
        items
            .iter()
            .find(|item| item["id"] == stroke["id"])
            .unwrap(),
        stroke
    );
}

#[test]
fn coordinate_limit_errors_leave_saved_bytes_unchanged() {
    let cli = Native::new();
    cli.op("create", &[]);
    cli.op("note", &["limit", "--at", "1000000000,0"]);
    let before = fs::read(&cli.board).unwrap();
    let error = failure(cli.invoke(&["note", cli.board.to_str().unwrap(), "too far"]));
    assert!(error["error"].as_str().unwrap().contains("rectangle"));
    assert_eq!(fs::read(&cli.board).unwrap(), before);
}

#[test]
fn recovered_tail_requires_a_clean_save_as_before_check_passes() {
    let cli = Native::new();
    // An older fixture would be compacted during migration instead of appending.
    let fixture = format!("moth/v{}.moth", moth_format::board::SCHEMA);
    fs::copy(corpus(&fixture), &cli.board).unwrap();
    fs::OpenOptions::new()
        .append(true)
        .open(&cli.board)
        .unwrap()
        .write_all(b"interrupted checkpoint")
        .unwrap();
    let output =
        cli.agent(&[json!({"ops":[{"op":"open","path":cli.board},{"op":"save"},{"op":"info"}]})]);
    let reply = success(output);
    assert_eq!(reply["results"][2]["recovered"], true);
    failure(cli.invoke(&["check", cli.board.to_str().unwrap()]));
    let clean = cli.root.join("clean.moth");
    let output = cli.agent(&[
        json!({"ops":[{"op":"open","path":cli.board},{"op":"save","path":clean},{"op":"check"}]}),
    ]);
    let reply = success(output);
    assert_eq!(reply["results"][2]["valid"], true);
    assert_eq!(
        success(cli.invoke(&["check", clean.to_str().unwrap()]))["results"][0]["valid"],
        true
    );
}

#[test]
fn stored_origins_drive_manual_and_automatic_groups() {
    let cli = Native::new();
    success(cli.invoke(&["create", cli.board.to_str().unwrap()]));
    let a = cli.root.join("one").join("refs");
    let b = cli.root.join("two").join("refs");
    fs::create_dir_all(&a).unwrap();
    fs::create_dir_all(&b).unwrap();
    fs::copy(cli.image(), a.join("first.png")).unwrap();
    fs::copy(cli.image(), a.join("second.png")).unwrap();
    fs::copy(cli.image(), b.join("third.png")).unwrap();
    let first = cli.op(
        "add",
        &[a.join("first.png").to_str().unwrap(), "--at", "13,-27"],
    )["results"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    cli.op("add", &[b.join("third.png").to_str().unwrap()]);
    cli.op("group-origin", &[]);
    let before = cli.info();
    let first_group = before["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["id"] == first)
        .unwrap()["parent"]
        .clone();
    assert!(first_group.is_string());
    let second = cli.op(
        "add",
        &[a.join("second.png").to_str().unwrap(), "--group-origin"],
    );
    assert_eq!(second["results"][0]["item"]["parent"], first_group);
    let after = cli.info();
    let groups: Vec<_> = after["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["kind"] == "group")
        .collect();
    assert_eq!(groups.len(), 2);
    assert!(groups.iter().all(|g| g["content"]["title"] == "refs"));
    assert_ne!(groups[0]["origin"], groups[1]["origin"]);
    cli.op("group-origin", &[]);
    assert_eq!(cli.info()["items"], after["items"]);
}

#[test]
fn native_point_snapping_moves_both_axes_and_preserves_relative_positions() {
    let cli = Native::new();
    success(cli.invoke(&["create", cli.board.to_str().unwrap()]));
    let first = cli.op("note", &["first", "--at", "137,-73"])["results"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let second = cli.op("note", &["second", "--at", "177,-33"])["results"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    cli.op("snap-grid", &["--id", &first, &second, "--step", "100"]);
    let info = cli.info();
    let items = info["items"].as_array().unwrap();
    let a = &items.iter().find(|i| i["id"] == first).unwrap()["rect"];
    let b = &items.iter().find(|i| i["id"] == second).unwrap()["rect"];
    assert_eq!(a[0], 100.);
    assert_eq!(a[1], -100.);
    assert_eq!(b[0], 140.);
    assert_eq!(b[1], -60.);
    let rejected = cli
        .command()
        .arg("--json")
        .arg("snap-grid")
        .arg(&cli.board)
        .args(["--step", "0"])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
}
