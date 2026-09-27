use super::{convert_path, hresult_error, open, starts_with_ascii_case_insensitive};
use crate::browser_command::{substitute_target, supports_direct_launch};
use crate::OpenError;
use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::process::{Command, Stdio};
use std::{io, ptr};
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::UI::Shell::{
    AssocQueryStringW, CommandLineToArgvW, SHEvaluateSystemCommandTemplate, ASSOCF_IS_PROTOCOL,
    ASSOCF_NOTRUNCATE, ASSOCSTR_COMMAND,
};

pub(crate) fn open_browser(path: &OsStr) -> Result<(), OpenError> {
    open_browser_with(path, resolve_association_command, open)
}

fn open_browser_with(
    path: &OsStr,
    resolve: impl FnOnce(&str, &OsStr) -> io::Result<Option<AssociationCommand>>,
    fallback: impl FnOnce(&OsStr) -> Result<(), OpenError>,
) -> Result<(), OpenError> {
    if !starts_with_ascii_case_insensitive(path, "file:") {
        return fallback(path);
    }

    // ShellExecute treats file URLs as shell objects and can canonicalize them before launching the
    // browser. Launching the registered browser command directly preserves the original URL.
    let command = match resolve("https", path) {
        Ok(Some(command)) => command,
        Ok(None) | Err(_) => return fallback(path),
    };

    // A failed spawn launched nothing, so falling back cannot open the target twice.
    if command.command().spawn().is_err() {
        return fallback(path);
    }
    Ok(())
}

struct AssociationCommand {
    executable: OsString,
    args: Vec<OsString>,
}

impl AssociationCommand {
    fn command(&self) -> Command {
        let mut command = Command::new(&self.executable);
        command
            .args(&self.args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }
}

fn resolve_association_command(
    association: &str,
    target: &OsStr,
) -> io::Result<Option<AssociationCommand>> {
    let template = query_association_command(association)?;
    command_from_template(&template, target)
}

fn command_from_template(
    template: &[u16],
    target: &OsStr,
) -> io::Result<Option<AssociationCommand>> {
    let (executable, args) = evaluate_command_template(template)?;
    // The checks work on Unicode text. Anything else is left to the Shell.
    let (Some(name), Some(target)) = (executable.to_str(), target.to_str()) else {
        return Ok(None);
    };
    if !supports_direct_launch(name) {
        return Ok(None);
    }
    let Some(args) = args
        .iter()
        .map(|arg| arg.to_str())
        .collect::<Option<Vec<_>>>()
    else {
        return Ok(None);
    };
    let Some(args) = substitute_target(args, target) else {
        return Ok(None);
    };

    Ok(Some(AssociationCommand {
        executable,
        args: args.into_iter().map(OsString::from).collect(),
    }))
}

fn query_association_command(association: &str) -> io::Result<Vec<u16>> {
    let association_wide = convert_path(OsStr::new(association))?;
    let operation = convert_path(OsStr::new("open"))?;
    let mut required = 0;

    // SAFETY: The input strings are NUL-terminated; a null output requests the required size.
    let result = unsafe {
        AssocQueryStringW(
            ASSOCF_IS_PROTOCOL | ASSOCF_NOTRUNCATE,
            ASSOCSTR_COMMAND,
            association_wide.as_ptr(),
            operation.as_ptr(),
            ptr::null_mut(),
            &mut required,
        )
    };
    if result < 0 || required == 0 {
        return if result < 0 {
            Err(hresult_error("AssocQueryStringW", result))
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("no command is registered for the {association:?} association"),
            ))
        };
    }

    let mut output = vec![0; required as usize];
    // Bound retries in case the association keeps changing while we read it.
    for _ in 0..3 {
        // SAFETY: `required` is the allocated UTF-16 capacity, and all input pointers remain valid.
        let result = unsafe {
            AssocQueryStringW(
                ASSOCF_IS_PROTOCOL | ASSOCF_NOTRUNCATE,
                ASSOCSTR_COMMAND,
                association_wide.as_ptr(),
                operation.as_ptr(),
                output.as_mut_ptr(),
                &mut required,
            )
        };
        if result == 0 {
            output.truncate(required as usize);
            if output.last() != Some(&0) {
                output.push(0);
            }
            return Ok(output);
        }
        if required as usize > output.len() {
            output.resize(required as usize, 0);
        } else {
            return Err(hresult_error("AssocQueryStringW", result));
        }
    }

    Err(io::Error::other(
        "browser association kept changing during lookup",
    ))
}

struct CoTaskMemString(windows_sys::core::PWSTR);

impl CoTaskMemString {
    /// # Safety
    /// `value` must be NULL or an exclusively owned, NUL-terminated UTF-16 string allocated with
    /// CoTaskMemAlloc. Ownership is transferred to this wrapper.
    unsafe fn from_shell_api(value: windows_sys::core::PWSTR) -> Self {
        Self(value)
    }

    fn to_os_string(&self) -> Option<OsString> {
        if self.0.is_null() {
            return None;
        }

        let mut length = 0;
        // SAFETY: This wrapper owns a NUL-terminated string returned by the Shell API.
        while unsafe { *self.0.add(length) } != 0 {
            length += 1;
        }
        // SAFETY: The scan above established the initialized string's length.
        Some(OsString::from_wide(unsafe {
            std::slice::from_raw_parts(self.0, length)
        }))
    }
}

impl Drop for CoTaskMemString {
    fn drop(&mut self) {
        // SAFETY: The Shell allocated this string with CoTaskMemAlloc; NULL is allowed.
        unsafe {
            CoTaskMemFree(self.0.cast());
        }
    }
}

fn evaluate_command_template(template: &[u16]) -> io::Result<(OsString, Vec<OsString>)> {
    if template.last() != Some(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "command template is not NUL-terminated",
        ));
    }
    let mut executable = ptr::null_mut();
    let mut parameters = ptr::null_mut();
    // SAFETY: The input is NUL-terminated, and outputs are valid writable pointers. Both returned
    // allocations are immediately wrapped so they are freed even if evaluation fails.
    let result = unsafe {
        SHEvaluateSystemCommandTemplate(
            template.as_ptr(),
            &mut executable,
            ptr::null_mut(),
            &mut parameters,
        )
    };
    // SAFETY: These are the owned CoTaskMemAlloc strings returned by the Shell API above (or NULL).
    let (executable, parameters) = unsafe {
        (
            CoTaskMemString::from_shell_api(executable),
            CoTaskMemString::from_shell_api(parameters),
        )
    };
    if result < 0 {
        return Err(hresult_error("SHEvaluateSystemCommandTemplate", result));
    }

    let executable = executable.to_os_string().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "the association command has no executable",
        )
    })?;
    let parameters = parameters.to_os_string().unwrap_or_default();
    let args = parse_command_line(&parameters)?;
    Ok((executable, args))
}

struct LocalArgv(*mut windows_sys::core::PWSTR);

impl Drop for LocalArgv {
    fn drop(&mut self) {
        // SAFETY: This owns the single allocation returned by CommandLineToArgvW.
        unsafe {
            LocalFree(self.0.cast());
        }
    }
}

fn parse_command_line(parameters: &OsStr) -> io::Result<Vec<OsString>> {
    if parameters.is_empty() {
        return Ok(Vec::new());
    }

    let mut command_line: Vec<u16> = OsStr::new("placeholder.exe ").encode_wide().collect();
    command_line.extend(parameters.encode_wide());
    command_line.push(0);

    let mut count = 0;
    // SAFETY: `command_line` is NUL-terminated and `count` is a valid output pointer.
    let parsed = unsafe { CommandLineToArgvW(command_line.as_ptr(), &mut count) };
    if parsed.is_null() {
        return Err(io::Error::last_os_error());
    }
    let parsed = LocalArgv(parsed);
    if count < 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "CommandLineToArgvW returned an invalid argument count",
        ));
    }

    let mut args = Vec::with_capacity((count - 1) as usize);
    for index in 1..count {
        // SAFETY: CommandLineToArgvW returned `count` pointers to NUL-terminated strings.
        let value = unsafe { *parsed.0.add(index as usize) };
        let mut length = 0;
        // SAFETY: `value` points to one of those strings, still owned by `parsed`.
        while unsafe { *value.add(length) } != 0 {
            length += 1;
        }
        // SAFETY: The scan above established the initialized string's length.
        args.push(OsString::from_wide(unsafe {
            std::slice::from_raw_parts(value, length)
        }));
    }
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn command_interpreter_template_uses_fallback_without_spawning() {
        let interpreter = Path::new(&std::env::var_os("SystemRoot").unwrap())
            .join("System32")
            .join("cmd.exe");
        let mut template = OsString::from("\"");
        template.push(interpreter);
        template.push("\" /C \"%1\"");
        let template = convert_path(&template).unwrap();
        let target = OsStr::new("file:///C:/index.html?q=\"&|^%#fragment");
        let mut called = false;
        open_browser_with(
            target,
            |_, target| {
                let command = command_from_template(&template, target).unwrap();
                assert!(command.is_none());
                Ok(command)
            },
            |path| {
                assert_eq!(path, target);
                called = true;
                Ok(())
            },
        )
        .unwrap();
        assert!(called);
    }

    #[test]
    fn identifies_file_urls() {
        assert!(starts_with_ascii_case_insensitive(
            OsStr::new("File:///C:/index.html"),
            "file:"
        ));
        assert!(!starts_with_ascii_case_insensitive(
            OsStr::new("https://example.com"),
            "file:"
        ));
        assert!(!starts_with_ascii_case_insensitive(
            OsStr::new("C:\\index.html"),
            "file:"
        ));
    }

    #[test]
    fn parses_quoted_association_parameters() {
        assert_eq!(
            parse_command_line(OsStr::new(r#"--flag "two words" "%1""#)).unwrap(),
            vec![
                OsString::from("--flag"),
                OsString::from("two words"),
                OsString::from("%1"),
            ]
        );
    }

    #[test]
    #[ignore = "requires a configured Windows HTTPS association"]
    fn resolves_the_system_browser_command_without_changing_the_url() {
        let target = OsStr::new("file:///C:/Users/Me%C5%82/index.html#section");
        let command = resolve_association_command("https", target)
            .unwrap()
            .expect("the browser command should use a supported placeholder");

        assert!(command.args.iter().any(|arg| {
            let arg: Vec<u16> = arg.encode_wide().collect();
            let target: Vec<u16> = target.encode_wide().collect();
            arg.windows(target.len()).any(|window| window == target)
        }));
    }

    #[test]
    fn discovery_failure_preserves_fallback_input() {
        let url = OsStr::new("file:///C:/Me%C5%82/index.html#section");
        let mut called = false;
        open_browser_with(
            url,
            |protocol, input| {
                assert_eq!(protocol, "https");
                assert_eq!(input, url);
                Ok(None)
            },
            |input| {
                called = true;
                assert_eq!(input, url);
                Ok(())
            },
        )
        .unwrap();
        assert!(called);
    }

    #[test]
    fn non_file_inputs_skip_discovery() {
        for input in [
            "index.html",
            "C:\\Me%C5%82\\index.html",
            "https://example.com/Me%C5%82?q=%23#section",
            "http://example.com/#section",
            "mailto:me@example.com",
        ] {
            let mut called = false;
            open_browser_with(
                OsStr::new(input),
                |_, _| panic!("only file URLs should query browser associations"),
                |path| {
                    called = true;
                    assert_eq!(path, input);
                    Ok(())
                },
            )
            .unwrap();
            assert!(called);
        }
    }

    #[test]
    fn lookup_errors_return_the_fallback_error() {
        let error = open_browser_with(
            OsStr::new("file:///C:/index.html"),
            |_, _| Err(io::Error::other("lookup failed")),
            |_| Err(OpenError::Io(io::Error::other("fallback failed"))),
        )
        .unwrap_err();
        match error {
            OpenError::Io(error) => assert_eq!(error.to_string(), "fallback failed"),
            error => panic!("expected fallback error, got {error:?}"),
        }
    }

    #[test]
    fn spawn_failure_falls_back() {
        let url = OsStr::new("file:///C:/index.html");
        let mut called = false;
        open_browser_with(
            url,
            |_, _| {
                Ok(Some(AssociationCommand {
                    executable: OsString::from("\0"),
                    args: vec![],
                }))
            },
            |input| {
                called = true;
                assert_eq!(input, url);
                Ok(())
            },
        )
        .unwrap();
        assert!(called);
    }

    #[test]
    fn browser_override_takes_precedence() {
        // Set BROWSER only in a subprocess so parallel tests cannot observe an environment change.
        // A missing override must return its own spawn error, even for a supported URL scheme.
        let missing_browser = std::env::temp_dir()
            .join(format!("opener-missing-browser-{}", std::process::id()))
            .join("browser.exe");
        assert!(!missing_browser.exists());
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "windows::browser::tests::check_browser_override",
            ])
            .env("BROWSER", &missing_browser)
            .env("OPENER_TEST_OVERRIDE", "1")
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[test]
    #[ignore = "subprocess helper for browser_override_takes_precedence"]
    fn check_browser_override() {
        if std::env::var_os("OPENER_TEST_OVERRIDE").is_none() {
            return;
        }
        let error = crate::open_browser("file:///C:/Me%C5%82/index.html#section").unwrap_err();
        match error {
            OpenError::Spawn { cmds, .. } => assert_eq!(cmds, std::env::var("BROWSER").unwrap()),
            error => panic!("expected the override's spawn error, got {error:?}"),
        }
    }

    #[test]
    fn browser_receives_original_url_as_one_argument() {
        let url = OsStr::new(
            "file:///C:/Me%C5%82/space%20and%23/ł/index.html?x=%25&y=\"two words\"&p=%1%L|^#section\\\\",
        );
        let output = std::env::temp_dir().join(format!(
            "opener-browser-{}-{}.txt",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut template = OsString::from("\"");
        template.push(std::env::current_exe().unwrap());
        // Keep a quoted target first and retain the registered arguments following it.
        // The URL is also a test filter. Only the named ignored helper test matches.
        template.push(
            "\" \"%1\" --ignored --exact windows::browser::tests::record_argument --nocapture",
        );
        let command = command_from_template(&convert_path(&template).unwrap(), url)
            .unwrap()
            .expect("the template should be supported");
        let status = command
            .command()
            .env("OPENER_TEST_ARGUMENT_OUTPUT", &output)
            .status()
            .unwrap();
        assert!(status.success());
        let received = std::fs::read_to_string(&output).unwrap();
        std::fs::remove_file(output).unwrap();
        assert_eq!(OsStr::new(&received), url);
    }

    #[test]
    #[ignore = "subprocess helper for browser_receives_original_url_as_one_argument"]
    fn record_argument() {
        let Some(output) = std::env::var_os("OPENER_TEST_ARGUMENT_OUTPUT") else {
            return;
        };
        let args: Vec<_> = std::env::args_os().collect();
        assert_eq!(args.len(), 6);
        assert_eq!(args[5], "--nocapture");
        std::fs::write(output, args[1].to_str().unwrap()).unwrap();
    }
}
