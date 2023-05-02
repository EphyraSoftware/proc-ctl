use std::env::args;
use std::process::Command;

pub fn main() {
    let mut args = args().skip(1);
    let child_program = args.next().unwrap();

    Command::new(child_program)
        .args(args)
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
}
