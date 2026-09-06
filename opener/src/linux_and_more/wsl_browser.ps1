# Discovery only: no caller-controlled URL or command text is evaluated here.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public static class OpenerBrowserAssociation
{
    [DllImport("shlwapi.dll", CharSet = CharSet.Unicode, ExactSpelling = true)]
    static extern int AssocQueryStringW(uint flags, uint str, string association,
        string extra, StringBuilder output, ref uint length);

    [DllImport("shell32.dll", CharSet = CharSet.Unicode, ExactSpelling = true)]
    static extern int SHEvaluateSystemCommandTemplate(string template,
        out IntPtr application, IntPtr commandLine, out IntPtr parameters);

    [DllImport("shell32.dll", CharSet = CharSet.Unicode, ExactSpelling = true,
        SetLastError = true)]
    static extern IntPtr CommandLineToArgvW(string commandLine, out int count);

    [DllImport("kernel32.dll", ExactSpelling = true)]
    static extern IntPtr LocalFree(IntPtr memory);

    public static string[] Command()
    {
        // ASSOCF_IS_PROTOCOL | ASSOCF_NOTRUNCATE, ASSOCSTR_COMMAND.
        const uint flags = 0x1000 | 0x20;
        uint length = 0;
        int result = AssocQueryStringW(flags, 1, "https", "open", null, ref length);
        if (result < 0) Marshal.ThrowExceptionForHR(result);
        if (length == 0) throw new InvalidOperationException("No HTTPS browser command");

        string template = null;
        for (int attempt = 0; attempt < 3; attempt++)
        {
            var buffer = new StringBuilder(checked((int)length));
            result = AssocQueryStringW(flags, 1, "https", "open", buffer, ref length);
            if (result == 0) { template = buffer.ToString(); break; }
            if (length <= buffer.Capacity) Marshal.ThrowExceptionForHR(result);
        }
        if (template == null) throw new InvalidOperationException("Browser lookup failed");

        IntPtr application = IntPtr.Zero, parameters = IntPtr.Zero, argv = IntPtr.Zero;
        try
        {
            result = SHEvaluateSystemCommandTemplate(template, out application,
                IntPtr.Zero, out parameters);
            if (result < 0) Marshal.ThrowExceptionForHR(result);
            if (application == IntPtr.Zero) throw new InvalidOperationException("No browser executable");
            var values = new List<string>();
            values.Add(Marshal.PtrToStringUni(application));
            string args = Marshal.PtrToStringUni(parameters);
            if (!String.IsNullOrEmpty(args))
            {
                int count;
                argv = CommandLineToArgvW("placeholder.exe " + args, out count);
                if (argv == IntPtr.Zero) throw new System.ComponentModel.Win32Exception();
                for (int i = 1; i < count; i++)
                    values.Add(Marshal.PtrToStringUni(Marshal.ReadIntPtr(argv, i * IntPtr.Size)));
            }
            return values.ToArray();
        }
        finally
        {
            if (argv != IntPtr.Zero) LocalFree(argv);
            Marshal.FreeCoTaskMem(application);
            Marshal.FreeCoTaskMem(parameters);
        }
    }
}
'@

# NUL separators preserve spaces, quotes and newlines without shell parsing.
[Console]::Write([string]::Join([string][char]0, [OpenerBrowserAssociation]::Command()))
