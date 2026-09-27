//! Checks for launching a registered Windows browser command directly, shared by the Windows and
//! WSL implementations of `open_browser()`.

/// Returns whether a registered browser `executable` can be launched directly, with the URL passed
/// as one argument using C-runtime quoting.
///
/// Scripts and known Windows hosts can interpret the URL as command text, so they are left to the
/// Shell. This does not detect every custom argument parser; other executables still need to follow
/// the usual C-runtime argument rules.
pub(crate) fn supports_direct_launch(executable: &str) -> bool {
    let name = executable
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    name.ends_with(".exe")
        && !matches!(
            name.as_str(),
            "cmd.exe"
                | "powershell.exe"
                | "powershell_ise.exe"
                | "pwsh.exe"
                | "wscript.exe"
                | "cscript.exe"
                | "mshta.exe"
        )
}

/// Substitutes `target` for the `%1`, `%L` and `%l` placeholders in a registered browser command's
/// arguments, dropping `%*`.
///
/// Returns `None` if the command has no target placeholder, or uses another placeholder, which
/// would need context that is not supplied.
pub(crate) fn substitute_target<'a>(
    args: impl IntoIterator<Item = &'a str>,
    target: &str,
) -> Option<Vec<String>> {
    let mut found_target = false;
    let mut result = Vec::new();
    for arg in args {
        if arg == "%*" {
            continue;
        }
        let mut substituted = String::with_capacity(arg.len());
        let mut chars = arg.chars();
        while let Some(c) = chars.next() {
            if c != '%' {
                substituted.push(c);
                continue;
            }
            match chars.next()? {
                '1' | 'L' | 'l' => {
                    substituted.push_str(target);
                    found_target = true;
                }
                _ => return None,
            }
        }
        result.push(substituted);
    }
    found_target.then_some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripts_and_known_command_hosts_skip_direct_launch() {
        for executable in [
            r"C:\Windows\System32\CMD.EXE",
            r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
            r"C:\Program Files\PowerShell\7\pwsh.exe",
            "powershell_ise.exe",
            "wscript.exe",
            "cscript.exe",
            "mshta.exe",
            r"C:\browser wrapper\open.BAT",
            r"C:\browser wrapper\open.cmd",
            "command.com",
            "open.ps1",
            r"C:\browser",
        ] {
            assert!(!supports_direct_launch(executable), "{executable}");
        }
        for executable in [
            r"C:\Program Files\Mozilla Firefox\firefox.exe",
            r"C:\Program Files\Browser\BROWSER.EXE",
            r"C:\cmd.exe\browser.exe",
            "C:/Program Files/Browser/browser.exe",
        ] {
            assert!(supports_direct_launch(executable), "{executable}");
        }
    }

    #[test]
    fn substitutes_shell_target_placeholders() {
        let target = "file:///C:/Users/Me%C5%82/index.html#section";
        assert_eq!(
            substitute_target(["--first", "%1", "--url=%L", "%*"], target),
            Some(vec![
                "--first".to_owned(),
                target.to_owned(),
                format!("--url={target}"),
            ])
        );
    }

    #[test]
    fn rejects_commands_without_a_target_placeholder() {
        for args in [["--first"], ["--urls=%*"]] {
            assert_eq!(substitute_target(args, "https://example.com"), None);
        }
    }

    #[test]
    fn unsupported_substitutions_reject_the_whole_command() {
        for unsupported in ["%2", "%V", "--urls=%*", "trailing%", "%UNKNOWN%"] {
            assert_eq!(
                substitute_target(["%1", unsupported], "file:///C:/index.html"),
                None,
                "{unsupported}"
            );
        }
    }

    #[test]
    fn substitution_does_not_reinterpret_the_target() {
        let target = "file:///C:/a%1%L%20.html?x=\"two words\"#fragment";
        assert_eq!(
            substitute_target(["%l"], target),
            Some(vec![target.to_owned()])
        );
    }
}
