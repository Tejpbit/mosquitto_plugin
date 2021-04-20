// Has to be included, to get the errors and success parameters that are used in the
// generate_dynamic_library macro invocation
use mosquitto_plugin::*;
use serde::Deserialize;
use simple_server::{Method, Server, StatusCode};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// Some simple nonsense structure to use as an example
#[derive(Debug)]
pub struct Test {
    i: i32,
    s: String,
    users: Arc<Mutex<HashMap<String, String>>>,
}

#[derive(Debug, Deserialize)]
struct User {
    user: String,
    password: String,
}

// Required trait implementation
impl MosquittoPlugin for Test {
    fn init(opts: std::collections::HashMap<&str, &str>) -> Self {
        // These are the strings provided after "auth_opt_<key> value" in the mosquitto.conf
        // only that they are provided on a hashmap form here
        let default = "hej";
        let topic = opts.get("topic").unwrap_or(&default);
        let level = opts.get("level").unwrap_or(&default);
        let level = level.parse().unwrap_or(0);

        let host = "127.0.0.1";
        let port = "7878";

        let users = HashMap::new();
        let users = Arc::new(Mutex::new(users));

        let users_clone = Arc::clone(&users);
        let server = Server::new(move |request, mut response| {
            println!("Request received. {} {}", request.method(), request.uri());

            match (request.method(), request.uri().path()) {
                (&Method::GET, "/hello") => Ok(response
                    .body("<h1>Hi!</h1><p>Hello Rust!</p>".as_bytes().to_vec())
                    .expect("Failed to write hello")),
                (&Method::POST, "/new-user") => {
                    Ok(match serde_json::from_slice::<User>(request.body()) {
                        Ok(user) => {
                            let mut users_clone = users_clone.lock().unwrap();
                            users_clone.insert(user.user, user.password);
                            response
                                .body("<h1>Hi!</h1><p>Created new user!</p>".as_bytes().to_vec())
                                .expect("Failed to write new user response")
                        }
                        Err(e) => response.body(
                            format!(
                                "<h1>Hi!</h1><p> Failed to create new user with error: {}</p>",
                                e
                            )
                            .as_bytes()
                            .to_vec(),
                        )?,
                    })
                }
                (_, _) => {
                    response.status(StatusCode::NOT_FOUND);
                    Ok(response
                        .body("<h1>404</h1><p>Not found!<p>".as_bytes().to_vec())
                        .expect("Failed to write 'not found' response"))
                }
            }
        });
        std::thread::spawn(move || {
            server.listen(host, port);
        });

        Test {
            i: level,
            s: topic.to_string(),
            users,
        }
    }
    fn username_password(
        &mut self,
        client_id: &str,
        u: Option<&str>,
        p: Option<&str>,
    ) -> Result<Success, Error> {
        println!("USERNAME_PASSWORD({}) {:?} - {:?}", client_id, u, p);
        if u.is_none() || p.is_none() {
            return Err(Error::Auth);
        }
        let u = u.unwrap();
        let p = p.unwrap();

        let user_in_users = {
            let users = self.users.lock().unwrap();
            if users.contains_key(u) && users.get(u).unwrap() == p {
                true
            } else {
                false
            }
        };
        println!("user_in_users {}", user_in_users);
        if user_in_users {
            // Declare the accepted new client
            self.broker_broadcast_publish(
                "new_client",
                "very_client is a friend. Lets make it feel at home!".as_bytes(),
                QOS::AtMostOnce,
                false,
            )?;
            // Welcome the new client privately
            self.broker_publish_to_client(
                client_id,
                "greeting",
                format!("Welcome {}", client_id).as_bytes(),
                QOS::AtMostOnce,
                false,
            )?;
            Ok(Success)
        } else {
            println!("USERNAME_PASSWORD failed for {}", client_id);
            // Snitch to all other clients what a bad client that was.
            self.broker_broadcast_publish(
                "snitcheroo",
                format!("{} is a bad bad client. No cookies for it.", client_id).as_bytes(),
                QOS::AtMostOnce,
                false,
            )?;
            Err(Error::Auth)
        }
    }

    fn acl_check(
        &mut self,
        client_id: &str,
        level: AclCheckAccessLevel,
        msg: MosquittoAclMessage,
    ) -> Result<Success, mosquitto_plugin::Error> {
        println!("allowed topic: {}", self.s);
        println!("topic: {}", msg.topic);
        println!("level requested: {}", level);

        // only the topic provided in the mosquitto.conf by the value auth_opt_topic <value> is
        // allowed, errors will not be reported to the clients though, they will only not be able
        // to send/receive messages and thus silently fail due to limitations in MQTT protocol
        if msg.topic == self.s {
            Ok(Success)
        } else {
            Err(Error::AclDenied)
        }
    }
}

// This generates the dynamic c bindings functions that are exported and usable by mosquitto
create_dynamic_library!(Test);
