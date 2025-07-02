use std::io::{self, Write};

pub fn ask_once(prompt: &str) -> bool {
    print!("{prompt}\nDo you want to proceed? (yes/no): ");
    io::stdout().flush().expect("stdout should be flushed");

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .expect("should be able to read stdin line");
    input.trim().to_lowercase() == "yes" || input.trim().to_lowercase() == "y"
}
