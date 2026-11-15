#![cfg_attr(test, allow(clippy::field_reassign_with_default))]

//! PeaceControl command line companion.

mod cli;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut output = String::new();
    match cli::execute(&args, &mut output) {
        Ok(()) => {
            if output.is_empty() {
                print!("{}", cli::HELP);
            } else {
                print!("{output}");
            }
        }
        Err(error) => {
            eprintln!("peacecontrol-cli: {error}");
            std::process::exit(1);
        }
    }
}
