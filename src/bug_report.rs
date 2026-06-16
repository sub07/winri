//! Surfaces errors and panics to the user as a message box, optionally opening
//! a pre-filled GitHub issue. Handles both fatal (will-exit) and non-fatal
//! (will-continue) reports.
use std::panic::PanicHookInfo;

use crate::{DEBUG_MODE, WINRI_VERSION, logger, system::message_box_query};

/// Path to the current session's log file, as a display string (with a
/// fallback message if the directory can't be resolved). Shown to the user so
/// they can attach logs to a report.
pub fn log_file_path() -> String {
    logger::log_dir().map_or_else(
        |_| "<could not determine log directory>".into(),
        |mut p| {
            p.push("winri.log");
            p.display().to_string()
        },
    )
}

/// Anything that can describe itself as a bug report. Implemented for both
/// `anyhow::Error` and panic info, and wrapped by the fatal/non-fatal markers
/// to produce the final user-facing text.
pub trait IntoBugReportInfo {
    fn title(&self) -> String;
    /// One-line summary (the issue title suffix and release-build body).
    fn short(&self) -> String;
    /// Full detail (the message box body and debug-build issue body).
    fn long(&self) -> String;
}

impl IntoBugReportInfo for anyhow::Error {
    fn title(&self) -> String {
        "Bug report".into()
    }

    fn short(&self) -> String {
        format!("{self}")
    }

    fn long(&self) -> String {
        format!("{self:#?}")
    }
}

impl IntoBugReportInfo for &PanicHookInfo<'_> {
    fn title(&self) -> String {
        "Panic report".into()
    }

    fn short(&self) -> String {
        if let Some(str) = self.payload_as_str() {
            str.to_string()
        } else {
            "<non-string panic payload>".into()
        }
        .trim()
        .to_string()
    }

    fn long(&self) -> String {
        self.to_string()
    }
}

struct FatalBugReport<B: IntoBugReportInfo>(B);
struct NonFatalBugReport<B: IntoBugReportInfo>(B);

impl<B: IntoBugReportInfo> IntoBugReportInfo for FatalBugReport<B> {
    fn title(&self) -> String {
        format!("Fatal {}", self.0.title())
    }

    fn short(&self) -> String {
        self.0.short()
    }

    fn long(&self) -> String {
        format!(
            r"
An unrecoverable error occurred, Application will now exit:

```
{}
```

Before it closes, would you like to submit a pre-filled github issue about this bug ?


>Please note:
>    1. Your browser will open
>    2. You can edit the issue before submitting it
>    3. Error reports are automatically included and may contain personal information such as window titles, browser tab names, etc.
>    4. To help debugging, you can include your session logs ({}). But note that it may contains personal information such as window titles, browser tab names, etc.
                ",
            if DEBUG_MODE {
                self.0.long()
            } else {
                self.0.short()
            },
            log_file_path()
        )
    }
}

impl<B: IntoBugReportInfo> IntoBugReportInfo for NonFatalBugReport<B> {
    fn title(&self) -> String {
        self.0.title()
    }

    fn short(&self) -> String {
        self.0.short()
    }

    fn long(&self) -> String {
        format!(
            r"
An error occurred, but winri will continue its execution. Some systems might not be working properly:

```
{}
```

Before resuming winri,
would you like to submit a pre-filled github issue about this bug ?


>Please note:
>    1. Your browser will open
>    2. You can edit the issue before submitting it
>    3. Error reports are automatically included and may contain personal information such as window titles, browser tab names, etc.
>    4. To help debugging, you can include your session logs ({}). But note that it may contains personal information such as window titles, browser tab names, etc.
                ",
            if DEBUG_MODE {
                self.0.long()
            } else {
                self.0.short()
            },
            log_file_path()
        )
    }
}

fn message_box_bug_report(report: impl IntoBugReportInfo) {
    let create_bug_report = message_box_query(report.title().as_str(), report.long().as_str());

    if create_bug_report {
        let create_issue_url = format!(
            "https://github.com/sub07/winri/issues/new?labels=crash logs&title=[v{}] {}: {}&body={}",
            WINRI_VERSION,
            report.title(),
            report.short(),
            report.long().replace('\n', "%0A")
        );
        if let Err(err) = webbrowser::open(&create_issue_url) {
            log::error!("Could not open web browser to create bug report: {err:?}");
        } else {
            log::info!("Opened web browser to create bug report: {create_issue_url}");
        }
    }
}

pub fn display_and_exit(report: impl IntoBugReportInfo) {
    message_box_bug_report(FatalBugReport(report));
}

pub fn display_and_continue(report: impl IntoBugReportInfo) {
    message_box_bug_report(NonFatalBugReport(report));
}
