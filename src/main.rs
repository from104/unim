use hangul::builder_3bul::HangulBuilder3Bul;
use std::io::{self, BufRead};


pub mod hangul;

fn main() -> io::Result<()> {
    let mut hangul_builder = HangulBuilder3Bul::new();
    let stdin = io::stdin();
    let reader = stdin.lock();

    for line in reader.lines() {
        let input_line = line?;
        if input_line == "." {
            break;
        }

        let result = hangul_builder.convert_string(&input_line);
        println!("{}", result);
    }

    Ok(())
}

