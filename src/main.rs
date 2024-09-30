use std::io::{Read, Write};
use std::str;

use sha2::Digest;
use sqlx::Row;

fn open_telnet_server() -> std::net::TcpListener {
    std::net::TcpListener::bind("0.0.0.0:4567").unwrap()
}

async fn connect_to_mariadb() -> sqlx::Result<sqlx::Pool<sqlx::MySql>> {
    sqlx::MySqlPool::connect("mysql://root:AAUUSStinhh1124*@10.0.0.11:3306/tel-logger").await
}

struct Logbook {
    id: i32,
    name: String,
    owner: String,
    users_write: Vec<u16>,
    users_read: Vec<u16>,
}

impl Logbook {
    fn new(
        id: i32,
        name: String,
        owner: String,
        users_write: Vec<u16>,
        users_read: Vec<u16>,
    ) -> Self {
        Self {
            id,
            name,
            owner,
            users_write,
            users_read,
        }
    }
    fn get_username_from_id(&self, id: u16) -> String {
        let pool = connect_to_mariadb();
        let pool = futures::executor::block_on(pool).unwrap();
        let query = sqlx::query("SELECT Username FROM users WHERE ID = ?").bind(id);
        let query: Vec<sqlx::mysql::MySqlRow> =
            futures::executor::block_on(query.fetch_all(&pool)).unwrap();
        if query.len() == 0 {
            return "".to_string();
        }
        query[0].get("Username")
    }
}

struct Session {
    stream: std::net::TcpStream,
    username: Option<String>,
    logged_in: bool,
    logbook: Option<String>,
}

impl Session {
    fn new(
        stream: std::net::TcpStream,
        username: Option<String>,
        logged_in: bool,
        logbook: Option<String>,
    ) -> Self {
        Self {
            stream,
            username,
            logged_in,
            logbook,
        }
    }

    fn write(&mut self, message: &str) {
        self.stream.write(message.as_bytes()).unwrap();
    }
}

struct Contact {
    id: i32,
    callsign: String,
    time: std::time::SystemTime,
    frequency: f32,
    mode: String,
    report: String,
    notes: String,
    operator: String,
    station: String,
}

fn main() {
    let pool = connect_to_mariadb();
    let pool = futures::executor::block_on(pool).unwrap();
    let listener = open_telnet_server();
    let mut sessions: Vec<Session> = Vec::new();
    let mut line = String::new();
    loop {
        let (stream, _) = listener.accept().unwrap();
        sessions.push(Session::new(stream, None, false, None));
        let mut buffer = [0; 1024];
        let mut stream = sessions.last().unwrap().stream.try_clone().unwrap();
        stream.write(greet().as_bytes()).unwrap();
        // loop through all the streams buffer and search for a newline
        loop {
            let bytes_read = stream.read(&mut buffer).unwrap();
            line.push_str(str::from_utf8(&buffer[0..bytes_read]).unwrap());
            println!("\"{}\"", line);
            if line.contains("\r\n") {
                let newline = line.replace("\r\n", "");
                println!("Final Line: {}", newline);
                if newline.contains("login") {
                    let mut split = newline.split_whitespace();
                    let _ = split.next();
                    let username = split.next().unwrap();
                    let password = split.next().unwrap();
                    let usernames = list_usernames();
                    if check_username(usernames, username.to_string()) {
                        if check_password(username.to_string(), password.to_string()) {
                            stream.write("Login successful\r\n>".as_bytes()).unwrap();
                            sessions.last_mut().unwrap().username = Some(username.to_string());
                            sessions.last_mut().unwrap().logged_in = true;
                        } else {
                            stream.write("Incorrect password\r\n>".as_bytes()).unwrap();
                        }
                    } else {
                        stream
                            .write("Username does not exist\r\n>".as_bytes())
                            .unwrap();
                    }
                    line.clear();
                    continue;
                }
                if sessions.last().unwrap().logged_in != false {
                    if newline.contains("useradd") {
                        let mut split = newline.split_whitespace();
                        let _ = split.next();
                        let username = split.next().unwrap_or_else(|| {
                            stream
                                .write("Username not provided\r\n>".as_bytes())
                                .unwrap();
                            return "";
                        });
                        let password = split.next().unwrap_or_else(|| {
                            stream
                                .write("Password not provided\r\n>".as_bytes())
                                .unwrap();
                            return "";
                        });
                        let usernames = list_usernames();
                        if check_username(usernames, username.to_string()) {
                            stream
                                .write("Username already exists\r\n>".as_bytes())
                                .unwrap();
                        } else {
                            add_user(username.to_string(), password.to_string());
                            stream.write("User added\r\n>".as_bytes()).unwrap();
                        }
                        line.clear();
                        continue;
                    } else if newline.contains("userdel") {
                        let mut split = newline.split_whitespace();
                        let _ = split.next();
                        let username = split.next().unwrap();
                        let usernames = list_usernames();
                        if check_username(usernames, username.to_string()) {
                            remove_user(username.to_string());
                            stream.write("User removed\r\n>".as_bytes()).unwrap();
                        } else {
                            stream
                                .write("Username does not exist\r\n>".as_bytes())
                                .unwrap();
                        }
                        line.clear();
                        continue;
                    }
                } else {
                    stream
                        .write("Please login: login <username> <password>\r\n>".as_bytes())
                        .unwrap();
                    line.clear();
                    continue;
                }
                stream.write("Invalid command\r\n>".as_bytes()).unwrap();
                line.clear();
                continue;
            }
        }
    }
}

fn list_usernames() -> Vec<String> {
    let pool = connect_to_mariadb();
    let pool = futures::executor::block_on(pool).unwrap();
    let query = sqlx::query("SELECT username FROM users");
    let query: Vec<sqlx::mysql::MySqlRow> =
        futures::executor::block_on(query.fetch_all(&pool)).unwrap();
    let mut usernames: Vec<String> = Vec::new();
    for row in query {
        usernames.push(row.get("username"));
    }
    usernames
}

fn check_username(usernames: Vec<String>, username: String) -> bool {
    for name in usernames {
        if name == username {
            return true;
        }
    }
    false
}

fn hash_password(password: String) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(password);
    let result = hasher.finalize();
    format!("{:x}", result)
}

fn check_password(username: String, password: String) -> bool {
    let pool = connect_to_mariadb();
    let pool = futures::executor::block_on(pool).unwrap();
    let query = sqlx::query("SELECT password FROM users WHERE username = ?").bind(username);
    let query: Vec<sqlx::mysql::MySqlRow> =
        futures::executor::block_on(query.fetch_all(&pool)).unwrap();
    let hash = hash_password(password);
    let mut password_hashes: Vec<String> = Vec::new();
    for row in query {
        password_hashes.push(row.get("password"));
    }
    for pass in password_hashes {
        if pass == hash {
            return true;
        }
    }
    false
}

fn add_user(username: String, password: String) -> bool {
    let pool = connect_to_mariadb();
    let pool = futures::executor::block_on(pool).unwrap();
    let query = sqlx::query("INSERT INTO users (username, password) VALUES (?, ?)")
        .bind(username)
        .bind(hash_password(password));
    let _query: Vec<sqlx::mysql::MySqlRow> =
        futures::executor::block_on(query.fetch_all(&pool)).unwrap();
    true
}

fn remove_user(username: String) -> bool {
    let pool = connect_to_mariadb();
    let pool = futures::executor::block_on(pool).unwrap();
    let query = sqlx::query("DELETE FROM users WHERE username = ?").bind(username);
    let _query: Vec<sqlx::mysql::MySqlRow> =
        futures::executor::block_on(query.fetch_all(&pool)).unwrap();
    true
}

fn create_logbook(name: String) -> bool {
    let pool = connect_to_mariadb();
    let pool = futures::executor::block_on(pool).unwrap();
    let query = sqlx::query("CREATE TABLE ? (id INT AUTO_INCREMENT PRIMARY KEY, callsign VARCHAR(255), time DATETIME, frequency FLOAT, mode VARCHAR(255), report VARCHAR(255), notes VARCHAR(255), operator VARCHAR(255), station VARCHAR(255)")
        .bind(name);
    let _query: Vec<sqlx::mysql::MySqlRow> =
        futures::executor::block_on(query.fetch_all(&pool)).unwrap();
    true
}

fn greet() -> String {
    "Welcome to KE8YGW's Telnet Logging Server\r\n>".to_string()
}
