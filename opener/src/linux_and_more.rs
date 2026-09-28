use crate::OpenError;
use std::ffi::OsStr;
use std::io;
use std::process::{Command, Stdio};

// Generated from xdg-utils v1.2.1:
// https://gitlab.freedesktop.org/xdg/xdg-utils/-/tree/v1.2.1/scripts
// Local change, marked "opener:" in the script: file_url_to_path keeps file URLs for other hosts
// unchanged instead of turning them into relative paths.
const XDG_OPEN_SCRIPT: &str = include_str!("xdg-open");
// The script is passed as one argument, which Linux limits to 128 KiB.
const _: () = assert!(XDG_OPEN_SCRIPT.len() < 128 * 1024);

#[cfg(target_os = "linux")]
mod wsl;
#[cfg(target_os = "linux")]
pub(crate) use self::wsl::windows_browser_argument as wsl_to_windows_browser_argument;

#[cfg(target_os = "linux")]
pub(crate) fn open_browser(path: &OsStr) -> Result<(), OpenError> {
    if crate::is_wsl() {
        wsl::open_browser(path)
    } else {
        open(path)
    }
}

pub(crate) fn open(path: &OsStr) -> Result<(), OpenError> {
    #[cfg(target_os = "linux")]
    if crate::is_wsl() {
        return wsl::open(path);
    }

    open_with_xdg_open(path)
}

fn open_with_xdg_open(path: &OsStr) -> Result<(), OpenError> {
    // xdg-open may not exit until the opened application does, so its exit status isn't checked.
    // Its most common failure, a missing file, is reported here instead.
    ensure_local_file_exists(path)?;

    if open_with_system_xdg_open(path).is_err() {
        open_with_internal_xdg_open(path)?;
    }

    Ok(())
}

/// Returns a `NotFound` error if xdg-open would treat `target` as a path, and nothing exists there.
///
/// File URLs are not checked.
fn ensure_local_file_exists(target: &OsStr) -> Result<(), OpenError> {
    if has_url_scheme(target) {
        return Ok(());
    }
    match std::fs::metadata(target) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Err(OpenError::Io(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "file '{}' does not exist",
                    std::path::Path::new(target).display()
                ),
            )))
        }
        // Other errors, such as a lack of permission, are left for xdg-open to handle.
        _ => Ok(()),
    }
}

/// Matches xdg-open's test for a URL scheme, `^[[:alpha:]][[:alpha:][:digit:]+.-]*:`.
fn has_url_scheme(target: &OsStr) -> bool {
    let target = target.as_encoded_bytes();
    let Some(colon) = target.iter().position(|&b| b == b':') else {
        return false;
    };
    let scheme = &target[..colon];
    scheme.first().is_some_and(u8::is_ascii_alphabetic)
        && scheme
            .iter()
            .all(|&b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'.' | b'-'))
}

#[cfg(all(feature = "reveal", target_os = "linux"))]
pub(crate) fn reveal(path: &std::path::Path) -> Result<(), OpenError> {
    if crate::is_wsl() {
        reveal_in_windows_explorer(path)
    } else {
        crate::freedesktop::reveal_with_dbus(path).or_else(|_| reveal_fallback(path))
    }
}

#[cfg(all(feature = "reveal", not(target_os = "linux")))]
pub(crate) fn reveal(path: &std::path::Path) -> Result<(), OpenError> {
    reveal_fallback(path)
}

#[cfg(feature = "reveal")]
fn reveal_fallback(path: &std::path::Path) -> Result<(), OpenError> {
    let path = path.canonicalize().map_err(OpenError::Io)?;
    let parent = path.parent().unwrap_or(std::path::Path::new("/"));
    open(parent.as_os_str())
}

fn open_with_system_xdg_open(path: &OsStr) -> io::Result<()> {
    crate::spawn_detached(
        Command::new("xdg-open")
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
    )
}

fn open_with_internal_xdg_open(path: &OsStr) -> Result<(), OpenError> {
    // Passing the script with -c keeps stdin free, so programs it launches get no script input.
    crate::spawn_detached(
        Command::new("sh")
            .arg("-c")
            .arg(XDG_OPEN_SCRIPT)
            .arg("xdg-open")
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
    )
    .map_err(|err| OpenError::Spawn {
        cmds: "sh".into(),
        source: err,
    })
}

#[cfg(all(feature = "reveal", target_os = "linux"))]
fn reveal_in_windows_explorer(path: &std::path::Path) -> Result<(), OpenError> {
    let converted_path = wsl::wslpath("-w", path.as_os_str()).ok();
    let converted_path = converted_path.as_deref();
    let path = match converted_path {
        None => path,
        Some(x) => std::path::Path::new(x),
    };
    crate::spawn_detached(
        Command::new("explorer.exe")
            .arg("/select,")
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
    )
    .map_err(|err| OpenError::Spawn {
        cmds: "explorer.exe".into(),
        source: err,
    })
}

#[cfg(target_os = "linux")]
pub(crate) fn is_wsl() -> bool {
    if is_docker() {
        return false;
    }

    if let Ok(true) = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|osrelease| osrelease.to_ascii_lowercase().contains("microsoft"))
    {
        return true;
    }

    if let Ok(true) = std::fs::read_to_string("/proc/version")
        .map(|version| version.to_ascii_lowercase().contains("microsoft"))
    {
        return true;
    }

    false
}

#[cfg(target_os = "linux")]
fn is_docker() -> bool {
    let has_docker_env = std::fs::metadata("/.dockerenv").is_ok();

    let has_docker_cgroup = std::fs::read_to_string("/proc/self/cgroup")
        .map(|cgroup| cgroup.to_ascii_lowercase().contains("docker"))
        .unwrap_or(false);

    has_docker_env || has_docker_cgroup
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn open_reports_missing_local_file() {
        let path = env::temp_dir().join(format!("opener-missing-{}", std::process::id()));
        let error = open_with_xdg_open(path.as_os_str()).unwrap_err();
        assert!(matches!(error, OpenError::Io(ref e) if e.kind() == io::ErrorKind::NotFound));
    }

    #[test]
    fn missing_paths_without_url_scheme_are_reported() {
        // These don't match xdg-open's URL scheme pattern, so it treats them as relative paths.
        for path in ["1a:b", ":b", "-a:b"] {
            let error = ensure_local_file_exists(OsStr::new(path)).unwrap_err();
            assert!(
                matches!(error, OpenError::Io(ref e) if e.kind() == io::ErrorKind::NotFound),
                "{path}"
            );
        }
    }

    #[test]
    fn existing_paths_are_accepted() {
        ensure_local_file_exists(env::temp_dir().as_os_str()).unwrap();
    }

    #[test]
    fn urls_are_not_checked() {
        // xdg-open treats these as URLs, even though some could also be relative paths.
        for url in [
            "https://example.com/missing",
            "mailto:someone@example.com",
            "a+b.c-d9:missing",
            "file:///opener-missing",
        ] {
            ensure_local_file_exists(OsStr::new(url)).unwrap();
        }
    }
}
