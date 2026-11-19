use odd_app::playtest::{parse_session_json, summarize_markdown};
use std::{env, fs, process::ExitCode};

fn main() -> ExitCode {
    let paths: Vec<_> = env::args_os().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: playtest-summary <session.json> [...]");
        return ExitCode::from(2);
    }
    let mut logs = Vec::new();
    for path in paths {
        let result = fs::read_to_string(&path)
            .map_err(|error| error.to_string())
            .and_then(|json| parse_session_json(&json).map_err(str::to_owned));
        match result {
            Ok(log) => logs.push(log),
            Err(error) => {
                eprintln!("{}: {error}", path.to_string_lossy());
                return ExitCode::FAILURE;
            }
        }
    }
    print!("{}", summarize_markdown(&logs));
    ExitCode::SUCCESS
}
