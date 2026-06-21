use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("version") => println!("git-it-agent {}", env!("CARGO_PKG_VERSION")),
        Some("status") => match args.get(2) {
            Some(repo) => match git_it_agent::status_json(repo) {
                Ok(json) => println!("{json}"),
                Err(e) => {
                    eprintln!("error: {e}");
                    exit(1);
                }
            },
            None => {
                eprintln!("usage: git-it-agent status <repo-path>");
                exit(2);
            }
        },
        Some("device-id") => println!("{}", git_it_agent::repos::device_id()),
        Some("pair") => match args.get(2) {
            Some(code) => {
                let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
                if let Err(e) = rt.block_on(git_it_agent::auth::run_pairing(code)) {
                    eprintln!("error: {e}");
                    exit(1);
                }
            }
            None => {
                // No code yet: print this Mac's device id so the operator can
                // enter it in the Git It app, which then shows a pairing code.
                println!("This Mac's device id:\n  {}", git_it_agent::repos::device_id());
                println!("Enter it in the Git It app, then run: git-it-agent pair <code>");
            }
        },
        Some("relay") => {
            let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
            if let Err(e) = rt.block_on(git_it_agent::relay::run()) {
                eprintln!("relay error: {e}");
                exit(1);
            }
        }
        _ => {
            eprintln!("usage: git-it-agent <version|device-id|status <repo-path>|pair [code]|relay>");
            exit(2);
        }
    }
}
