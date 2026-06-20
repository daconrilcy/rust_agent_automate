mod finalize;
mod render;
mod transport;

pub use finalize::{
    CompletedReport, ReportFailure, ReportSpec, detect_clean_implementation_audit, finalize_report,
    run_codex_report,
};
pub use render::{command_failure_exit_code, print_completed_report, print_report_failure};
pub use transport::{
    COMMAND_OUTCOME_PATH_ENV, CommandOutcome, command_failure_outcome, write_command_outcome,
};
