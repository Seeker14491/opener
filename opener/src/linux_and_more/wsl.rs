use crate::OpenError;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::{env, io};
use url::Url;

const DISCOVER_BROWSER: &str = include_str!("wsl_browser.ps1");

// The target is read from the environment, so it is never parsed as command-line text.
// Exit code 2 means the Windows shell tried to open the target and failed.
const SHELL_EXECUTE: &str = r"$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
$info = New-Object System.Diagnostics.ProcessStartInfo($env:OPENER_TARGET)
$info.UseShellExecute = $true
$info.Verb = 'open'
try { [void][System.Diagnostics.Process]::Start($info) }
catch { [Console]::Error.Write($_.Exception.GetBaseException().Message); exit 2 }";

pub(super) fn open(path: &OsStr) -> Result<(), OpenError> {
    // Until the Windows shell has tried to open the target, nothing has been launched, so falling
    // back cannot open it twice.
    let target = match windows_target(path, |path| wslpath("-aw", path)) {
        Ok(target) => target,
        // The target cannot be expressed for Windows, so another launcher would fail too.
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => {
            return Err(OpenError::Io(error));
        }
        Err(_) => return super::open_with_xdg_open(path),
    };
    let result = run_powershell(SHELL_EXECUTE, Some(&target), |mut command| {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
    });
    match result {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) if output.status.code() == Some(2) => Err(OpenError::ExitStatus {
            cmd: "powershell.exe",
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }),
        _ => super::open_with_xdg_open(path),
    }
}

pub(super) fn open_browser(path: &OsStr) -> Result<(), OpenError> {
    let Some(target) = path.to_str().filter(|s| {
        s.get(..5)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("file:"))
    }) else {
        return open(path);
    };

    // Match native Windows: unsupported associations, unavailable discovery, and failed spawns
    // fall back to open. None of these launched anything, so the target cannot open twice.
    let prepared = (|| {
        let target = windows_file_url(target, |path| wslpath("-aw", path))?;
        let output = discover_browser()?;
        let (executable, args) = association_command(&output, &target)?;
        let executable = wslpath("-u", OsStr::new(&executable))?;
        Ok::<_, io::Error>((executable, args))
    })();
    let Ok((executable, args)) = prepared else {
        return open(path);
    };

    // For example, WSL cannot execute browsers installed from the Microsoft Store.
    let spawned = Command::new(&executable)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    if spawned.is_err() {
        return open(path);
    }
    Ok(())
}

/// Converts an argument for a Windows `BROWSER` executable, or returns `None` to pass it unchanged.
pub(crate) fn windows_browser_argument(path: &OsStr) -> Option<OsString> {
    if let Some(target) = path.to_str() {
        if let Ok(url) = Url::parse(target) {
            if url.scheme() == "file" {
                return windows_file_url(target, |path| wslpath("-aw", path))
                    .ok()
                    .map(OsString::from);
            }
            // Single-letter schemes are Windows drive paths such as `C:\file`, not URLs.
            if url.scheme().len() > 1 {
                return None;
            }
        }
    }
    wslpath("-w", path).ok()
}

fn checked_output(mut command: Command) -> io::Result<Vec<u8>> {
    let Output { status, stdout, .. } = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()?;
    if !status.success() {
        return Err(io::Error::other(format!("command failed: {status}")));
    }
    Ok(stdout)
}

pub(super) fn wslpath(mode: &str, path: &OsStr) -> io::Result<OsString> {
    use std::os::unix::ffi::OsStringExt;
    let mut command = Command::new("wslpath");
    command.args([OsStr::new(mode), path]);
    let mut output = checked_output(command)?;
    // Remove only wslpath's terminator; whitespace can be part of a filename.
    if output.last() == Some(&b'\n') {
        output.pop();
    }
    if output.is_empty() || output.contains(&0) {
        return Err(io::Error::other("invalid wslpath output"));
    }
    Ok(OsString::from_vec(output))
}

/// Runs a PowerShell script, passing `target` to it as `$env:OPENER_TARGET`.
fn run_powershell<T>(
    script: &str,
    target: Option<&OsStr>,
    run: impl Fn(Command) -> io::Result<T>,
) -> io::Result<T> {
    let command = |executable: &OsStr| {
        let mut command = Command::new(executable);
        command.args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            script,
        ]);
        if let Some(target) = target {
            // WSLENV lists the variables that are shared with Windows processes.
            let mut wslenv = env::var_os("WSLENV").unwrap_or_default();
            if !wslenv.is_empty() {
                wslenv.push(":");
            }
            wslenv.push("OPENER_TARGET");
            command.env("OPENER_TARGET", target).env("WSLENV", wslenv);
        }
        command
    };
    match run(command(OsStr::new("powershell.exe"))) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            // wslpath respects custom Windows drive mount locations.
            let executable = wslpath(
                "-u",
                OsStr::new(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"),
            )?;
            run(command(&executable))
        }
        result => result,
    }
}

fn discover_browser() -> io::Result<Vec<u8>> {
    run_powershell(DISCOVER_BROWSER, None, checked_output)
}

fn invalid_data(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn invalid_input(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn is_windows_drive_path(path: &[u8]) -> bool {
    path.len() >= 4 && path[1].is_ascii_alphabetic() && path[2..4] == *b":/"
}

/// Percent-decodes a file URL's path, ignoring any host. Drive letters like `C|` become `C:`.
///
/// Like Python's `urllib` and Windows, escaped separators such as `%2F` are decoded too.
fn file_url_path(url: &Url) -> io::Result<PathBuf> {
    // to_file_path rejects hosts here, so decode the path from a local copy.
    let mut local = Url::parse("file:///").unwrap();
    local.set_path(url.path());
    local
        .to_file_path()
        .map_err(|()| invalid_input("invalid file URL path"))
}

/// Converts `path` to a target for the Windows shell. URLs other than file URLs are unchanged.
fn windows_target(
    path: &OsStr,
    convert: impl FnOnce(&OsStr) -> io::Result<OsString>,
) -> io::Result<OsString> {
    let Some(target) = path.to_str() else {
        return convert(path);
    };
    let url = match Url::parse(target) {
        Ok(url) if url.scheme() == "file" => url,
        // Includes Windows drive paths such as `C:\file`, which parse as single-letter schemes.
        Ok(_) => return Ok(path.to_owned()),
        Err(_) => return convert(path),
    };
    // Like on Windows, the shell opens a file path, so the query and fragment are discarded.
    let host = url.host_str();
    let path = file_url_path(&url)?;
    if host.is_none() && !is_windows_drive_path(path.as_os_str().as_bytes()) {
        return convert(path.as_os_str());
    }
    let path = path
        .to_str()
        .ok_or_else(|| invalid_input("non-Unicode Windows path"))?
        .replace('/', "\\");
    Ok(match host {
        Some(host) => format!(r"\\{host}{path}"),
        None => path[1..].to_owned(),
    }
    .into())
}

fn windows_file_url(
    target: &str,
    convert: impl FnOnce(&OsStr) -> io::Result<OsString>,
) -> io::Result<String> {
    let parsed = Url::parse(target).map_err(|error| invalid_data(&error.to_string()))?;
    if parsed.scheme() != "file" {
        return Err(invalid_data("not a file URL"));
    }
    let path = file_url_path(&parsed)?;
    if parsed.host_str().is_some() || is_windows_drive_path(path.as_os_str().as_bytes()) {
        // Already a Windows drive or UNC URL. Preserve the original encoding exactly.
        return Ok(target.to_owned());
    }
    let converted = convert(path.as_os_str())?;
    let converted = converted
        .to_str()
        .ok_or_else(|| invalid_data("non-Unicode Windows path"))?;
    let normalized = converted.replace('\\', "/");
    let mut result = Url::parse("file:///").unwrap();
    let path = if let Some(unc) = normalized.strip_prefix("//") {
        let (host, path) = unc
            .split_once('/')
            .ok_or_else(|| invalid_data("invalid UNC path"))?;
        if host.is_empty() {
            return Err(invalid_data("empty UNC host"));
        }
        result
            .set_host(Some(host))
            .map_err(|error| invalid_data(&error.to_string()))?;
        path
    } else {
        let bytes = normalized.as_bytes();
        if bytes.len() < 3 || !bytes[0].is_ascii_alphabetic() || bytes[1..3] != *b":/" {
            return Err(invalid_data("Windows path is not absolute"));
        }
        &normalized
    };
    // Segment setters encode literal %, # and ? in filenames instead of treating them as URL syntax.
    result
        .path_segments_mut()
        .unwrap()
        .clear()
        .extend(path.split('/'));
    result.set_query(parsed.query());
    result.set_fragment(parsed.fragment());
    Ok(result.into())
}

fn association_command(output: &[u8], target: &str) -> io::Result<(String, Vec<String>)> {
    let output =
        std::str::from_utf8(output).map_err(|_| invalid_data("non-UTF-8 browser command"))?;
    let mut parts = output.split('\0');
    let executable = parts.next().unwrap_or_default();
    let name = executable
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !name.ends_with(".exe")
        || matches!(
            name.as_str(),
            "cmd.exe"
                | "powershell.exe"
                | "powershell_ise.exe"
                | "pwsh.exe"
                | "wscript.exe"
                | "cscript.exe"
                | "mshta.exe"
        )
    {
        return Err(invalid_data("unsupported browser executable"));
    }
    let mut found = false;
    let mut args = Vec::new();
    for arg in parts {
        if arg == "%*" {
            continue;
        }
        let mut result = String::new();
        let mut chars = arg.chars();
        while let Some(c) = chars.next() {
            if c == '%' {
                match chars.next() {
                    Some('1' | 'l' | 'L') => {
                        result.push_str(target);
                        found = true;
                    }
                    _ => return Err(invalid_data("unsupported browser placeholder")),
                }
            } else {
                result.push(c);
            }
        }
        args.push(result);
    }
    if !found {
        return Err(invalid_data("browser command has no URL placeholder"));
    }
    Ok((executable.to_owned(), args))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    #[test]
    fn converts_linux_file_url_and_preserves_url_components() {
        let result = windows_file_url(
            "file:///mnt/c/Me%C5%82/name%23%25%20two.html?x=%23&y=two%20words#section%201",
            |path| {
                assert_eq!(path, "/mnt/c/Meł/name#% two.html");
                Ok(OsString::from(r"C:\Meł\name#% two.html"))
            },
        )
        .unwrap();
        assert_eq!(
            result,
            "file:///C:/Me%C5%82/name%23%25%20two.html?x=%23&y=two%20words#section%201"
        );
    }

    #[test]
    fn converts_linux_filesystem_to_unc_url() {
        let result = windows_file_url("file://localhost/home/me/report.html?#", |path| {
            assert_eq!(path, "/home/me/report.html");
            Ok(OsString::from(
                r"\\wsl.localhost\archlinux\home\me\report.html",
            ))
        })
        .unwrap();
        assert_eq!(
            result,
            "file://wsl.localhost/archlinux/home/me/report.html?#"
        );
    }

    #[test]
    fn preserves_existing_windows_urls_exactly() {
        for target in [
            "file:///C:/Me%C5%82/a%23.html?q=%25#section",
            "FILE:///c:/a%2fb.html#fragment",
            "file://server/share/a%20b.html#section",
            "file:///C:",
            "file:///C|/Me%C5%82/a.html#section",
        ] {
            assert_eq!(
                windows_file_url(target, |_| panic!("already a Windows URL")).unwrap(),
                target
            );
        }
    }

    #[test]
    fn conversion_failures_do_not_produce_a_wrong_url() {
        assert!(
            windows_file_url("file:///tmp/a", |_| Err(io::Error::other("no wslpath"))).is_err()
        );
        assert!(windows_file_url("file:///tmp/a", |_| Ok(OsString::from("relative"))).is_err());
    }

    #[test]
    fn converts_targets_for_the_windows_shell() {
        let convert = |path: &OsStr| -> io::Result<OsString> {
            Ok(format!("converted:{}", path.to_str().unwrap()).into())
        };
        for (input, expected) in [
            (
                "https://example.com/a%20b?q=1#section",
                "https://example.com/a%20b?q=1#section",
            ),
            (r"C:\Users\Me\a b.txt", r"C:\Users\Me\a b.txt"),
            ("file:///C:/Me%C5%82/a%23.html?q#section", r"C:\Meł\a#.html"),
            ("file:///C:", r"C:\"),
            (
                "file://server/share/a%20b.html#section",
                r"\\server\share\a b.html",
            ),
            (
                "file:///mnt/c/Me%C5%82/a%23.html#section",
                "converted:/mnt/c/Meł/a#.html",
            ),
            (
                "file://localhost/home/me/report.html",
                "converted:/home/me/report.html",
            ),
            ("report.html", "converted:report.html"),
            ("/home/me/a b.html", "converted:/home/me/a b.html"),
            ("file:///C|/Me%C5%82/a.html#section", r"C:\Meł\a.html"),
            // Like Python's urllib, escaped separators are decoded.
            ("file:///C:/docs/a%2Fb.html", r"C:\docs\a\b.html"),
            ("file:///C:/docs/a%5Cb.html", r"C:\docs\a\b.html"),
            ("file://server/share/a%5cb.html", r"\\server\share\a\b.html"),
            ("file:///mnt/c/a%2Fb.html", "converted:/mnt/c/a/b.html"),
            // wslpath maps `\` in Linux file names to a character Windows allows.
            ("file:///mnt/c/a%5Cb.html", r"converted:/mnt/c/a\b.html"),
        ] {
            assert_eq!(
                windows_target(OsStr::new(input), convert).unwrap(),
                expected,
                "{input}"
            );
        }
    }

    #[test]
    fn browser_override_receives_non_file_urls_unchanged() {
        for url in [
            "https://example.com/a%20b?q=1#section",
            "mailto:me@example.com",
        ] {
            assert_eq!(windows_browser_argument(OsStr::new(url)), None);
        }
    }

    #[test]
    fn substitutes_url_once_without_interpreting_its_contents() {
        let target = "file:///C:/Me%C5%82/a%20b.html?x=\"two words\"&p=%1%L|^#section";
        let (exe, args) = association_command(
            b"C:\\Program Files\\Browser\\browser.exe\0--url=%L\0--after\0%*",
            target,
        )
        .unwrap();
        assert_eq!(exe, r"C:\Program Files\Browser\browser.exe");
        assert_eq!(args, [format!("--url={target}"), "--after".to_owned()]);
    }

    #[test]
    fn rejects_commands_that_need_shell_interpretation() {
        for executable in [
            "cmd.exe",
            "PowerShell.EXE",
            "powershell_ise.exe",
            "pwsh.exe",
            "wscript.exe",
            "cscript.exe",
            "mshta.exe",
            "browser.cmd",
            "browser",
        ] {
            assert!(
                association_command(format!("C:\\{executable}\0%1").as_bytes(), "url").is_err()
            );
        }
        for args in [
            "%2",
            "%V",
            "--urls=%*",
            "%UNKNOWN%",
            "trailing%",
            "--no-target",
        ] {
            assert!(
                association_command(format!("C:\\browser.exe\0{args}").as_bytes(), "url").is_err()
            );
        }
        assert!(association_command(b"\xff\0%1", "url").is_err());
    }

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let path = env::temp_dir().join(format!(
                "opener-wsl-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn script(&self, name: &str, body: &str) {
            let path = self.0.join(name);
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn launches_converted_url_and_handles_fallbacks_in_subprocesses() {
        // Environment changes are confined to subprocesses, so parallel tests stay independent.
        for scenario in [
            "browser",
            "shell",
            "http",
            "shell-error",
            "no-powershell",
            "spawn-failure",
            "nul-byte",
            "override",
        ] {
            let fixture = Fixture::new();
            if scenario != "no-powershell" {
                // Without OPENER_TARGET, this is browser discovery; with it, a shell launch.
                fixture.script(
                    "powershell.exe",
                    r#"if [ -z "$OPENER_TARGET" ]; then
    [ "$OPENER_WSL_SCENARIO" = shell ] && exit 1
    printf 'C:\\browser.exe\000--single-argument\000%%1'
else
    case ":$WSLENV:" in *:OPENER_TARGET:*) ;; *) exit 1;; esac
    [ "$OPENER_WSL_SCENARIO" = shell-error ] && printf 'No application' >&2 && exit 2
    printf '%s\000' "$0" "$OPENER_TARGET" > "$OPENER_WSL_FIXTURE/received"
fi"#,
                );
            }
            fixture.script(
                "wslpath",
                r#"case "$1" in
-aw) printf 'C:\\Meł\\name#%% two.html\n';;
-u) case "$2" in
    *powershell.exe) printf '%s/missing.exe\n' "$OPENER_WSL_FIXTURE";;
    *) printf '%s/browser.exe\n' "$OPENER_WSL_FIXTURE";;
    esac;;
esac"#,
            );
            let recorder = "printf '%s\\000' \"$0\" \"$@\" > \"$OPENER_WSL_FIXTURE/received\"";
            if scenario != "spawn-failure" {
                fixture.script("browser.exe", recorder);
            }
            fixture.script("xdg-open", recorder);
            fixture.script("override", recorder);
            let status = Command::new(env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "linux_and_more::wsl::tests::launch_helper",
                ])
                .env("PATH", &fixture.0)
                .env("OPENER_WSL_FIXTURE", &fixture.0)
                .env("OPENER_WSL_SCENARIO", scenario)
                .env_remove("BROWSER")
                .status()
                .unwrap();
            assert!(status.success(), "scenario {scenario}");
        }
    }

    #[test]
    #[ignore = "subprocess helper"]
    fn launch_helper() {
        let Some(root) = env::var_os("OPENER_WSL_FIXTURE").map(PathBuf::from) else {
            return;
        };
        let scenario = env::var("OPENER_WSL_SCENARIO").unwrap();
        let target = match scenario.as_str() {
            "http" | "shell-error" => "https://example.com/a%20b#section",
            "nul-byte" => "file:///mnt/c/a%00b.html",
            _ => "file:///mnt/c/Me%C5%82/name%23%25%20two.html?x=%23#section",
        };
        let result = if scenario == "override" {
            env::set_var("BROWSER", root.join("override"));
            crate::open_browser(target)
        } else {
            open_browser(OsStr::new(target))
        };
        let received = root.join("received");
        match scenario.as_str() {
            // The shell tried to open the target, so its failure is reported without falling back.
            "shell-error" => assert!(matches!(
                result,
                Err(OpenError::ExitStatus { cmd: "powershell.exe", ref stderr, .. })
                    if stderr == "No application"
            )),
            // No launcher can accept the decoded NUL, so this fails without trying xdg-open.
            "nul-byte" => assert!(matches!(
                result,
                Err(OpenError::Io(ref error)) if error.kind() == io::ErrorKind::InvalidInput
            )),
            _ => result.unwrap(),
        }
        if matches!(scenario.as_str(), "shell-error" | "nul-byte") {
            assert!(!received.exists());
            return;
        }
        // Opening is asynchronous; wait for the recorder to finish writing the final NUL.
        let mut bytes = Vec::new();
        for _ in 0..250 {
            bytes = std::fs::read(&received).unwrap_or_default();
            if bytes.last() == Some(&0) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let received = String::from_utf8(bytes).unwrap();
        let (executable, args) = match scenario.as_str() {
            "browser" => (
                "browser.exe",
                "--single-argument\0file:///C:/Me%C5%82/name%23%25%20two.html?x=%23#section",
            ),
            "shell" | "spawn-failure" => ("powershell.exe", r"C:\Meł\name#% two.html"),
            "http" => ("powershell.exe", target),
            "no-powershell" => ("xdg-open", target),
            "override" => ("override", target),
            _ => unreachable!(),
        };
        assert_eq!(
            received,
            format!("{}\0{args}\0", root.join(executable).display())
        );
    }
}
