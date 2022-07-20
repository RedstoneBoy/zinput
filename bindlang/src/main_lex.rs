fn main() {
    let source = std::fs::read_to_string("example.bind").unwrap();
    let res = bindlang::parse(&source);

    match res {
        Ok(module) => println!("{}", module.display(&source)),
        Err(err) => println!("{}", err),
    }
}
