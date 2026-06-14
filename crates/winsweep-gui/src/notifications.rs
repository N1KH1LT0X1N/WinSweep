//! Windows toast notifications (via PowerShell WinRT bridge)
//!
//! Uses a hidden PowerShell process to call the Windows.UI.Notifications WinRT
//! API, which is the most compatible approach for traditional Win32 desktop
//! applications on Windows 10/11.  The PowerShell window is fully hidden
//! (`-WindowStyle Hidden`) so there is no visual flash.

use tracing::warn;

/// Show a Windows toast notification.
///
/// Fires and forgets — the spawned process is detached.
/// Failures are logged via `tracing::warn`.
pub fn show_toast(title: &str, body: &str) {
    let ps = format!(
        r#"
$ErrorActionPreference = 'Stop'
Try {{
    [Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
    $template = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02)
    $titleNode = $template.SelectSingleNode('//text[@id="1"]')
    $bodyNode  = $template.SelectSingleNode('//text[@id="2"]')
    if ($titleNode) {{ $titleNode.AppendChild($template.CreateTextNode('{title}')) | Out-Null }}
    if ($bodyNode)  {{ $bodyNode.AppendChild($template.CreateTextNode('{body}'))  | Out-Null }}
    $toast = [Windows.UI.Notifications.ToastNotification]::new($template)
    [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('WinSweep').Show($toast)
}} Catch {{
    # Silently swallow; the GUI already has its own status-bar feedback.
}}
"#,
        title = ps_escape(title),
        body = ps_escape(body),
    );

    let result = std::process::Command::new("powershell.exe")
        .args([
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &ps,
        ])
        .spawn();

    if let Err(e) = result {
        warn!("Failed to spawn toast notification process: {}", e);
    }
}

/// Escape a string for safe embedding inside a PowerShell single-quoted context.
///
/// PowerShell single-quoted strings treat `'` as needing doubling (`''`).
/// We also strip control characters and XML-unsafe bytes that could break the
/// underlying XML template manipulation.
fn ps_escape(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .flat_map(|c| match c {
            '\'' => vec!['\'', '\''], // PowerShell single-quote escape
            '&' => "&amp;".chars().collect(),
            '<' => "&lt;".chars().collect(),
            '>' => "&gt;".chars().collect(),
            '"' => "&quot;".chars().collect(),
            c => vec![c],
        })
        .collect()
}

/// Show a toast notification; failures are silently logged.
///
/// This is the preferred call-site function — it wraps `show_toast` and
/// ensures no panic can propagate from notification code.
pub fn show_toast_safe(title: &str, body: &str) {
    show_toast(title, body);
}

#[cfg(test)]
mod tests {
    use super::ps_escape;

    #[test]
    fn test_ps_escape_plain() {
        assert_eq!(ps_escape("Hello World"), "Hello World");
    }

    #[test]
    fn test_ps_escape_single_quote() {
        assert_eq!(ps_escape("it's"), "it''s");
    }

    #[test]
    fn test_ps_escape_ampersand() {
        assert_eq!(ps_escape("rock & roll"), "rock &amp; roll");
    }

    #[test]
    fn test_ps_escape_angle_brackets() {
        assert_eq!(ps_escape("<tag>"), "&lt;tag&gt;");
    }

    #[test]
    fn test_ps_escape_double_quote() {
        assert_eq!(ps_escape(r#"say "hi""#), "say &quot;hi&quot;");
    }

    #[test]
    fn test_ps_escape_strips_control_chars() {
        // Newlines, tabs, etc. should be removed
        assert_eq!(ps_escape("line\nnewline"), "linenewline");
    }
}
