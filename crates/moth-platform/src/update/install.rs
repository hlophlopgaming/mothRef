// SPDX-License-Identifier: GPL-3.0-or-later
use super::{
    Installation, Kind, Release, Session,
    release::{download, fingerprint},
    state_directory,
};
use serde::{Deserialize, Serialize};
use std::{
    env,
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

const HELPER: &str = "--mothref-update-helper";
const RESUME: &str = "MOTHREF_UPDATE_RESUME";
const TRIAL: &str = "MOTHREF_UPDATE_TRIAL";
const FAILED: &str = "MOTHREF_UPDATE_FAILED";
const STARTUP_LIMIT: Duration = Duration::from_secs(60);
const SESSION_LIMIT: u64 = 4 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct Plan {
    kind: Kind,
    target: PathBuf,
    executable: PathBuf,
    installed_hash: [u8; 32],
    artifact: PathBuf,
    replacement: PathBuf,
    release: Release,
    session: Session,
    software: bool,
}

/// A verified release staged beside the installation, so replacement stays on one volume.
pub struct Prepared {
    directory: tempfile::TempDir,
    helper: PathBuf,
    plan: Plan,
}

impl Prepared {
    /// Downloads and stages a matching package on a worker.
    pub fn download(release: Release) -> io::Result<Self> {
        let installation = Installation::detect()?.ok_or_else(|| {
            io::Error::other("This installation uses an external package manager")
        })?;
        let parent = installation
            .path
            .parent()
            .ok_or_else(|| io::Error::other("Missing installation directory"))?;
        let directory = tempfile::Builder::new()
            .prefix(".mothref-update-")
            .tempdir_in(parent)?;
        let installed = if installation.kind == Kind::MacBundle {
            &installation.executable
        } else {
            &installation.path
        };
        let installed_hash = fingerprint(installed)?;
        let artifact = download(&release, installation.kind, directory.path())?;
        let replacement = stage(installation.kind, &artifact, directory.path())?;
        if !matches!(installation.kind, Kind::AppImage | Kind::WindowsSetup) {
            fs::remove_file(&artifact)?;
        }
        Ok(Self {
            helper: helper_executable(&installation.executable)?,
            directory,
            plan: Plan {
                kind: installation.kind,
                target: installation.path,
                executable: installation.executable,
                installed_hash,
                artifact,
                replacement,
                release,
                session: Vec::new(),
                software: env::args_os().any(|arg| arg == "--software"),
            },
        })
    }

    /// Writes the restart plan and starts a waiting helper while errors can still be shown in the UI.
    pub fn arm(mut self, session: Session) -> io::Result<Armed> {
        self.plan.session = session;
        let directory = self.directory.path();
        let mut file = File::create_new(directory.join("plan.json"))?;
        serde_json::to_writer(&mut file, &self.plan)?;
        if file.metadata()?.len() > SESSION_LIMIT {
            return Err(io::Error::other("Update session exceeds limit"));
        }
        file.sync_all()?;
        drop(file);

        // The input pipe is an explicit handoff. A failed spawn leaves the stage owned by TempDir.
        let helper = Command::new(&self.helper)
            .arg(HELPER)
            .arg(directory)
            .arg(std::process::id().to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        Ok(Armed {
            directory: self.directory,
            helper,
        })
    }
}

/// A helper waiting for the GUI and its board workers to finish.
pub struct Armed {
    directory: tempfile::TempDir,
    helper: Child,
}

impl Armed {
    /// Authorizes replacement after the event loop and board workers have been dropped.
    pub fn commit(mut self) -> io::Result<()> {
        let directory = self.directory.keep();
        let handoff = self
            .helper
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("Missing updater pipe"))
            .and_then(|mut stdin| stdin.write_all(b"install"));
        if handoff.is_err() {
            let _ = self.helper.wait();
            let _ = fs::remove_dir_all(directory);
        }
        handoff
    }
}

#[cfg(not(windows))]
fn helper_executable(executable: &Path) -> io::Result<PathBuf> {
    Ok(executable.to_owned())
}

#[cfg(windows)]
fn helper_executable(executable: &Path) -> io::Result<PathBuf> {
    // The setup wizard and ReplaceFileW both need the original image unmapped.
    let directory = state_directory()?;
    let helper = directory.join("helper.exe");
    let mut file = tempfile::NamedTempFile::new_in(directory)?;
    io::copy(&mut File::open(executable)?, file.as_file_mut())?;
    file.as_file().sync_all()?;
    file.persist(&helper)?;
    Ok(helper)
}

fn stage(kind: Kind, artifact: &Path, directory: &Path) -> io::Result<PathBuf> {
    let replacement = directory.join("new");
    match kind {
        Kind::AppImage => {
            fs::rename(artifact, &replacement)?;
            executable(&replacement)?;
        }
        Kind::LinuxArchive => {
            // Stream one exact archive member into a file we opened ourselves.
            let output = File::create_new(&replacement)?;
            let status = Command::new("tar")
                .args(["-xOzf"])
                .arg(artifact)
                .args(["--", "mothref"])
                .stdout(output.try_clone()?)
                .stderr(Stdio::null())
                .status()?;
            if !status.success() || !(1..=512 * 1024 * 1024).contains(&output.metadata()?.len()) {
                return Err(io::Error::other("Could not extract Linux executable"));
            }
            output.sync_all()?;
            executable(&replacement)?;
        }
        Kind::WindowsPortable => {
            let mut zip = zip::ZipArchive::new(File::open(artifact)?).map_err(io::Error::other)?;
            let mut binary = zip.by_name("mothref.exe").map_err(io::Error::other)?;
            if !binary.is_file() || !(1..=512 * 1024 * 1024).contains(&binary.size()) {
                return Err(io::Error::other("Invalid Windows executable"));
            }
            let mut output = File::create_new(&replacement)?;
            io::copy(&mut binary, &mut output)?;
            output.sync_all()?;
        }
        Kind::WindowsSetup => return Ok(artifact.to_owned()),
        Kind::MacBundle => {
            // This archive is published by the release workflow and has already
            // been verified. BSD tar extracts only the bundle into a private directory.
            let status = Command::new("/usr/bin/tar")
                .args(["-xzf"])
                .arg(artifact)
                .arg("-C")
                .arg(directory)
                .arg("MothRef.app")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()?;
            let bundle = directory.join("MothRef.app");
            if !status.success()
                || !Command::new("/usr/bin/codesign")
                    .args(["--verify", "--deep", "--strict"])
                    .arg(&bundle)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()?
                    .success()
            {
                return Err(io::Error::other(
                    "Could not verify the macOS application bundle",
                ));
            }
            return Ok(bundle);
        }
    }
    Ok(replacement)
}

#[cfg(unix)]
fn executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn executable(_: &Path) -> io::Result<()> {
    Ok(())
}

/// Internal helper entry point. It receives control only after a verified update is staged.
pub fn run_helper(args: &[std::ffi::OsString]) -> Option<io::Result<()>> {
    if args.get(1).map(|arg| arg.as_os_str()) != Some(OsStr::new(HELPER)) {
        return None;
    }
    Some((|| {
        let [_, _, directory, pid] = args else {
            return Err(io::Error::other("Invalid update handoff"));
        };
        let pid = pid
            .to_str()
            .and_then(|pid| pid.parse().ok())
            .ok_or_else(|| io::Error::other("Invalid update parent process"))?;
        let stage = Path::new(directory);
        let mut handoff = String::new();
        io::stdin().take(8).read_to_string(&mut handoff)?;
        if handoff != "install" {
            return Err(io::Error::other("Update handoff was canceled"));
        }
        wait_for_parent(pid)?;
        let state = state_directory()?;
        let result = apply(stage, &state);
        if let Err(error) = &result {
            let _ = fs::write(state.join("last-error.txt"), error.to_string());
        }
        result
    })())
}

#[cfg(not(windows))]
fn wait_for_parent(pid: u32) -> io::Result<()> {
    let deadline = Instant::now() + STARTUP_LIMIT;
    while parent_pid() == Some(pid) {
        if Instant::now() >= deadline {
            return Err(io::Error::other("MothRef did not exit for the update"));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn parent_pid() -> Option<u32> {
    rustix::process::getppid().map(|pid| pid.as_raw_nonzero().get() as u32)
}

#[cfg(target_os = "macos")]
fn parent_pid() -> Option<u32> {
    // SAFETY: getppid takes no pointers and has no preconditions.
    u32::try_from(unsafe { libc::getppid() }).ok()
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn parent_pid() -> Option<u32> {
    None
}

#[cfg(windows)]
fn wait_for_parent(pid: u32) -> io::Result<()> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, ERROR_INVALID_PARAMETER, WAIT_OBJECT_0},
        System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
    };
    // SAFETY: the numeric PID came from the launching process and the handle is closed below.
    let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    if handle.is_null() {
        let error = io::Error::last_os_error();
        return if error.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) {
            Ok(())
        } else {
            Err(error)
        };
    }
    // SAFETY: the handle was opened with synchronization rights.
    let result = unsafe {
        let result = WaitForSingleObject(handle, 60_000);
        CloseHandle(handle);
        result
    };
    if result == WAIT_OBJECT_0 {
        Ok(())
    } else {
        Err(io::Error::other("MothRef did not exit for the update"))
    }
}

fn read_plan(directory: &Path) -> io::Result<Plan> {
    serde_json::from_reader(File::open(directory.join("plan.json"))?.take(SESSION_LIMIT))
        .map_err(io::Error::other)
}

fn apply(directory: &Path, state: &Path) -> io::Result<()> {
    let plan = read_plan(directory)?;
    let lock = lock_file(&state.join("install.lock"))?;
    let installed = if plan.kind == Kind::MacBundle {
        &plan.executable
    } else {
        &plan.target
    };
    let previous = directory.join("previous");
    let mut attempted = false;
    let mut swapped = false;
    let result = (|| {
        if fingerprint(installed)? != plan.installed_hash {
            return Err(io::Error::other(
                "Another process updated this installation",
            ));
        }
        // A bundle exchange retains the original under the staged bundle's path.
        if plan.kind != Kind::MacBundle {
            fs::copy(&plan.target, &previous)?;
            File::open(&previous)?.sync_all()?;
        }
        attempted = true;
        replace(&plan)?;
        swapped = true;
        start(&plan, directory, true, false)
    })();
    if let Err(error) = result {
        let _ = fs::write(state.join("last-error.txt"), error.to_string());
        // Never replace a raced installation with the copy from an earlier process.
        if attempted {
            if plan.kind == Kind::MacBundle {
                if swapped {
                    swap_bundle(&plan.replacement, &plan.target)?;
                }
            } else if fingerprint(installed).ok() != Some(plan.installed_hash) {
                crate::atomic_replace(&previous, &plan.target)?;
            }
        }
        start(&plan, directory, false, true)?;
    }
    let result = fs::remove_dir_all(directory);
    drop(lock);
    result
}

fn lock_file(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .read(true)
        .open(path)?;
    file.lock()?;
    Ok(file)
}

fn replace(plan: &Plan) -> io::Result<()> {
    match plan.kind {
        Kind::WindowsSetup => {
            let mut dir = std::ffi::OsString::from("/DIR=");
            let parent = plan
                .target
                .parent()
                .ok_or_else(|| io::Error::other("Missing setup directory"))?;
            dir.push(setup_path(parent));
            let status = Command::new(&plan.artifact)
                .args([
                    "/SP-",
                    "/SILENT",
                    "/SUPPRESSMSGBOXES",
                    "/NORESTART",
                    "/NOCLOSEAPPLICATIONS",
                    "/NORESTARTAPPLICATIONS",
                    "/RESTARTEXITCODE=5",
                ])
                .arg(dir)
                .status()?;
            if status.success() {
                Ok(())
            } else {
                Err(io::Error::other(format!("Setup exited with {status}")))
            }
        }
        Kind::MacBundle => swap_bundle(&plan.replacement, &plan.target),
        _ => crate::atomic_replace(&plan.replacement, &plan.target),
    }
}

#[cfg(windows)]
fn setup_path(path: &Path) -> std::ffi::OsString {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    let path: Vec<_> = path.as_os_str().encode_wide().collect();
    let unc: Vec<_> = r"\\?\UNC\".encode_utf16().collect();
    let disk: Vec<_> = r"\\?\".encode_utf16().collect();
    if let Some(path) = path.strip_prefix(unc.as_slice()) {
        let mut result = std::ffi::OsString::from(r"\\");
        result.push(std::ffi::OsString::from_wide(path));
        result
    } else {
        std::ffi::OsString::from_wide(path.strip_prefix(disk.as_slice()).unwrap_or(&path))
    }
}

#[cfg(not(windows))]
fn setup_path(path: &Path) -> std::ffi::OsString {
    path.as_os_str().to_owned()
}

#[cfg(target_os = "macos")]
fn swap_bundle(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let from = std::ffi::CString::new(from.as_os_str().as_bytes())?;
    let to = std::ffi::CString::new(to.as_os_str().as_bytes())?;
    // SAFETY: both paths are NUL-terminated and remain alive for the call.
    if unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_SWAP) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(target_os = "macos"))]
fn swap_bundle(_: &Path, _: &Path) -> io::Result<()> {
    Err(io::Error::other("macOS bundle replacement requires macOS"))
}

fn start(plan: &Plan, directory: &Path, trial: bool, failed: bool) -> io::Result<()> {
    let ready = directory.join("ready");
    let _ = fs::remove_file(&ready);
    let program = if plan.kind == Kind::AppImage {
        &plan.target
    } else {
        &plan.executable
    };
    let mut command = Command::new(program);
    command
        .env(RESUME, directory)
        .env_remove(TRIAL)
        .env_remove(FAILED)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if trial {
        command.env(TRIAL, "1");
    }
    if failed {
        command.env(FAILED, "1");
    }
    if plan.kind == Kind::AppImage {
        // AppRun sets runtime paths inside the old mount. The new image owns its environment.
        for name in [
            "APPIMAGE",
            "APPDIR",
            "OWD",
            "ARGV0",
            "LD_LIBRARY_PATH",
            "PATH",
            "QT_PLUGIN_PATH",
            "XDG_DATA_DIRS",
        ] {
            command.env_remove(name);
        }
        if let Some(path) = env::var_os("PATH") {
            let old = env::var_os("APPDIR").map(PathBuf::from);
            let paths = env::split_paths(&path)
                .filter(|p| old.as_ref().is_none_or(|root| !p.starts_with(root)));
            if let Ok(path) = env::join_paths(paths) {
                command.env("PATH", path);
            }
        }
    }
    if plan.software {
        command.arg("--software");
    }
    let mut child = command.spawn()?;
    let expected = if trial {
        plan.release.tag().strip_prefix('v')
    } else {
        None
    };
    monitor(&mut child, directory, expected)
}

fn monitor(child: &mut Child, directory: &Path, expected: Option<&str>) -> io::Result<()> {
    let deadline = Instant::now() + STARTUP_LIMIT;
    let ready = || {
        fs::read_to_string(directory.join("ready"))
            .is_ok_and(|version| expected.is_none_or(|expected| version == expected))
    };
    loop {
        if ready() {
            return Ok(());
        }
        let exited = child.try_wait()?;
        if exited.is_some() || Instant::now() >= deadline {
            let _decision = lock_file(&directory.join("decision.lock"))?;
            if ready() {
                return Ok(());
            }
            if expected.is_some() {
                File::create_new(directory.join("rejected"))?;
            }
            if let Some(status) = exited {
                return Err(io::Error::other(format!(
                    "MothRef stopped before startup completed: {status}"
                )));
            }
            if child.try_wait()?.is_none() {
                child.kill()?;
                child.wait()?;
            }
            return Err(io::Error::other("MothRef did not present its first window"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The restored session after the renderer has successfully presented a frame.
pub struct Startup {
    /// Open boards grouped by window.
    pub session: Session,
    /// Notes from the exact GitHub release that was just installed.
    pub release: Option<Release>,
    /// The previous executable was relaunched after an installation error.
    pub failed: bool,
}

/// Detects a supervised startup without performing I/O on the event loop.
pub fn resuming() -> bool {
    env::var_os(RESUME).is_some()
}

/// Acknowledges startup after the first presented frame, from a worker.
pub fn startup() -> io::Result<Startup> {
    let directory = env::var_os(RESUME)
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("No update session"))?;
    let plan = read_plan(&directory)?;
    let trial = env::var_os(TRIAL).is_some();
    let version = env!("CARGO_PKG_VERSION");
    if trial && plan.release.tag().strip_prefix('v') != Some(version) {
        return Err(io::Error::other(
            "The installer did not start the requested version",
        ));
    }
    let _decision = lock_file(&directory.join("decision.lock"))?;
    if trial && directory.join("rejected").exists() {
        return Err(io::Error::other("The update startup timed out"));
    }
    let mut ready = tempfile::NamedTempFile::new_in(&directory)?;
    ready.write_all(version.as_bytes())?;
    ready.persist(directory.join("ready"))?;
    Ok(Startup {
        session: plan.session,
        release: trial.then_some(plan.release),
        failed: env::var_os(FAILED).is_some(),
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn failed_first_launch_restores_the_previous_executable() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("pending-update");
        fs::create_dir(&directory).unwrap();
        let target = root.path().join("mothref");
        let old = b"#!/bin/sh\nprintf 1.0.0 > \"$MOTHREF_UPDATE_RESUME/ready\"\n";
        fs::write(&target, old).unwrap();
        executable(&target).unwrap();
        let replacement = directory.join("replacement");
        fs::write(&replacement, b"#!/bin/sh\nexit 12\n").unwrap();
        executable(&replacement).unwrap();
        let plan = Plan {
            kind: Kind::LinuxArchive,
            target: target.clone(),
            executable: target.clone(),
            installed_hash: fingerprint(&target).unwrap(),
            artifact: replacement.clone(),
            replacement,
            release: serde_json::from_str(r#"{"tag_name":"v2.0.0","assets":[]}"#).unwrap(),
            session: Vec::new(),
            software: false,
        };
        serde_json::to_writer(File::create(directory.join("plan.json")).unwrap(), &plan).unwrap();

        apply(&directory, root.path()).unwrap();
        assert_eq!(fs::read(&target).unwrap(), old);
        assert!(!directory.exists());
        assert!(root.path().join("last-error.txt").exists());
    }

    #[test]
    fn startup_monitor_distinguishes_a_presented_frame_from_an_early_exit() {
        let stage = tempfile::tempdir().unwrap();
        let mut crashed = Command::new("sh").args(["-c", "exit 1"]).spawn().unwrap();
        assert!(monitor(&mut crashed, stage.path(), Some("2.0.0")).is_err());

        let stage = tempfile::tempdir().unwrap();
        let mut previous = Command::new("sh").args(["-c", "exit 0"]).spawn().unwrap();
        fs::write(stage.path().join("ready"), "1.0.0").unwrap();
        assert!(monitor(&mut previous, stage.path(), Some("2.0.0")).is_err());

        let stage = tempfile::tempdir().unwrap();
        let mut started = Command::new("sh").args(["-c", "exit 0"]).spawn().unwrap();
        fs::write(stage.path().join("ready"), "2.0.0").unwrap();
        assert!(monitor(&mut started, stage.path(), Some("2.0.0")).is_ok());
        started.wait().unwrap();
    }
}
