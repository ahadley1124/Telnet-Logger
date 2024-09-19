fn create_listener_socket() -> std::net::TcpListener {
    std::net::TcpListener::bind("0.0.0.0:4567").unwrap()
}

fn main() {
    let listener = create_listener_socket();
    println!("Listening on: {}", listener.local_addr().unwrap());
    drop(listener);
}