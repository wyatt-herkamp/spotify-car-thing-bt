pub mod deskthing;

pub(crate) fn get_deskthing_port() -> u16 {
    std::env::var("DESKTHING_PORT")
        .unwrap_or_else(|_| "36308".to_string())
        .parse()
        .expect("invalid port")
}
