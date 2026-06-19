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
        _ => {
            eprintln!("usage: git-it-agent <version|status <repo-path>>");
            exit(2);
        }
    }
}
