fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (code, out) = bezel_bridle::cli_run(&args);
    print!("{out}");
    std::process::exit(code);
}
