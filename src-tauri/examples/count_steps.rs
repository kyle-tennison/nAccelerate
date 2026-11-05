//! Prints how many steps a notebook should log: `cargo run --example count_steps -- path/to/file.ntop`

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: count_steps <file.ntop>...");
        std::process::exit(2);
    }
    for path in paths {
        match naccelerate_lib::notebook::count_steps(std::path::Path::new(&path)) {
            Ok(c) if c.groups > 1 => println!("{path}: {} steps ({} groups; nested groups are untested)", c.steps, c.groups),
            Ok(c) => println!("{path}: {} steps", c.steps),
            Err(e) => println!("{path}: could not count steps: {e}"),
        }
    }
}
