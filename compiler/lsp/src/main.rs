fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [] => {}
        [argument] if argument == "--stdio" => {}
        [argument] if argument == "--help" || argument == "-h" => {
            println!(
                "Usage: scoop-lsp [--stdio]\n\nScoop language server using the Language Server Protocol over standard input/output."
            );
            return;
        }
        [argument] if argument == "--version" || argument == "-V" => {
            println!("scoop-lsp {}", scoop_toolchain::VERSION);
            return;
        }
        _ => {
            eprintln!("usage: scoop-lsp [--stdio]");
            std::process::exit(2);
        }
    }
    match scoop_lsp::run_stdio() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("scoop-lsp: {error}");
            std::process::exit(1);
        }
    }
}
