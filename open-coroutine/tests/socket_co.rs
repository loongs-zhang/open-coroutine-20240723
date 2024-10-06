include!("../examples/socket_co.rs");

#[cfg(unix)]
#[test]
fn socket_co() -> std::io::Result<()> {
    main()
}
