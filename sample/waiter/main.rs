use std::io::stdin;

fn main() {
    println!("Waiting");
    stdin().read_line(&mut String::new()).unwrap();
}
