pub fn get_canister_bytecode_for(path: impl AsRef<std::path::Path>) -> Vec<u8> {
    let path = path.as_ref();
    match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => panic!(
            "{} does not exist. Consider building the factory with `--release` flag",
            path.display()
        ),
        Err(e) => panic!("{}", e),
    }
}
