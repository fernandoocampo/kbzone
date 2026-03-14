fn main() {
    if let Err(e) = kbzone::application::app::App::build().and_then(|app| app.run()) {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}
