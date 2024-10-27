use std::io::stdin;

fn main() {
    println!("Waiting");
    let buf = &mut String::new();
    stdin().read_line(buf).unwrap();
    println!("Waiting done with input: [{}]", buf);
}
