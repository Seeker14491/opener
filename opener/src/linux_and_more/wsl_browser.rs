use crate::OpenError;
use std::ffi::{OsStr, OsString};
use std::io;
use std::process::{Command, Output, Stdio};
use url::Url;

const DISCOVER_BROWSER: &str = include_str!("wsl_browser.ps1");

pub(super) fn open_browser(path: &OsStr) -> Result<(), OpenError> {
    let Some(target) = path.to_str().filter(|s| {
        s.get(..5)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("file:"))
    }) else {
        return super::open(path);
    };

    // Match native Windows: unsupported associations or unavailable discovery fall back to open.
    // Do not fall back after attempting a browser launch, which could open the target twice.
    let prepared = (|| {
        let target = windows_file_url(target, |path| wslpath("-aw", path))?;
        let output = discover_browser()?;
        let (executable, args) = association_command(&output, &target)?;
        let executable = wslpath("-u", OsStr::new(&executable))?;
        Ok::<_, io::Error>((executable, args))
    })();
    let Ok((executable, args)) = prepared else {
        return super::open(path);
    };

    Command::new(&executable)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|source| OpenError::Spawn {
            cmds: executable.to_string_lossy().into_owned(),
            source,
        })?;
    Ok(())
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

fn wslpath(mode: &str, path: &OsStr) -> io::Result<OsString> {
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

fn discover_browser() -> io::Result<Vec<u8>> {
    let run = |executable: &OsStr| {
        let mut command = Command::new(executable);
        command.args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            DISCOVER_BROWSER,
        ]);
        checked_output(command)
    };
    match run(OsStr::new("powershell.exe")) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            // wslpath respects custom Windows drive mount locations.
            let executable = wslpath(
                "-u",
                OsStr::new(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"),
            )?;
            run(&executable)
        }
        result => result,
    }
}

fn invalid_data(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn windows_file_url(
    target: &str,
    convert: impl FnOnce(&OsStr) -> io::Result<OsString>,
) -> io::Result<String> {
    let parsed = Url::parse(target).map_err(|error| invalid_data(&error.to_string()))?;
    if parsed.scheme() != "file" {
        return Err(invalid_data("not a file URL"));
    }
    let path = parsed.path().as_bytes();
    if parsed.host_str().is_some()
        || (path.len() >= 4 && path[1].is_ascii_alphabetic() && path[2..4] == *b":/")
    {
        // Already a Windows drive or UNC URL. Preserve the original encoding exactly.
        return Ok(target.to_owned());
    }
    let path = parsed
        .to_file_path()
        .map_err(|()| invalid_data("invalid file URL path"))?;
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
            let path = std::env::temp_dir().join(format!(
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
        for scenario in ["browser", "fallback", "spawn-error", "override", "http"] {
            let fixture = Fixture::new();
            fixture.script(
                "powershell.exe",
                if scenario == "fallback" {
                    "exit 1"
                } else {
                    "printf 'C:\\\\browser.exe\\000--single-argument\\000%%1'"
                },
            );
            fixture.script("wslpath", "case \"$1\" in\n-aw) printf 'C:\\\\Meł\\\\name#%% two.html\\n';;\n-u) printf '%s/browser.exe\\n' \"$OPENER_WSL_FIXTURE\";;\nesac");
            let recorder = "printf '%s\\000' \"$0\" \"$@\" > \"$OPENER_WSL_FIXTURE/received\"";
            if scenario != "spawn-error" {
                fixture.script("browser.exe", recorder);
            }
            fixture.script("xdg-open", recorder);
            fixture.script("override", recorder);
            let status = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "linux_and_more::wsl_browser::tests::launch_helper",
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
        let Some(root) = std::env::var_os("OPENER_WSL_FIXTURE").map(PathBuf::from) else {
            return;
        };
        let scenario = std::env::var("OPENER_WSL_SCENARIO").unwrap();
        let target = if scenario == "http" {
            "https://example.com/a%20b#section"
        } else {
            "file:///mnt/c/Me%C5%82/name%23%25%20two.html?x=%23#section"
        };
        let result = if scenario == "override" {
            std::env::set_var("BROWSER", root.join("override"));
            crate::open_browser(target)
        } else {
            open_browser(OsStr::new(target))
        };
        if scenario == "spawn-error" {
            assert!(matches!(result, Err(OpenError::Spawn { .. })));
            assert!(!root.join("received").exists());
            return;
        }
        result.unwrap();
        let received = root.join("received");
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
        let expected = if scenario == "browser" {
            format!(
                "{}\0--single-argument\0file:///C:/Me%C5%82/name%23%25%20two.html?x=%23#section\0",
                root.join("browser.exe").display()
            )
        } else {
            let executable = if scenario == "override" {
                "override"
            } else {
                "xdg-open"
            };
            format!("{}\0{target}\0", root.join(executable).display())
        };
        assert_eq!(received, expected);
    }
}
