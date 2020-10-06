#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate rocket;

use std::{collections::HashMap, sync::Mutex, time::SystemTime};

use rocket::Outcome;
use rocket::{
    http::RawStr,
    request::{self, FromParam, FromRequest, Request},
};

struct ApiKey(String);

struct Base64String(String);

impl<'r> FromParam<'r> for Base64String {
    type Error = &'r str;

    fn from_param(param: &'r RawStr) -> Result<Self, Self::Error> {
        let b64 = match base64::decode_config(param, base64::URL_SAFE) {
            Ok(s) => s,
            Err(_) => return Err("Cannot base64 decode"),
        };
        let inner = match String::from_utf8(b64) {
            Ok(s) => s,
            Err(_) => return Err("Failed to decode URL into utf8"),
        };
        Ok(Base64String(inner))
    }
}

struct Response {
    timestamp: u64,
    cache: String,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Returns true if `key` is a valid API key string.
fn is_valid(key: &str) -> bool {
    match std::env::var("MITM_KEY") {
        Ok(s) => key == s,
        _ => true,
    }
}

impl<'a, 'r> FromRequest<'a, 'r> for ApiKey {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, Self::Error> {
        let keys: Vec<_> = request.headers().get("x-mitm").collect();
        match keys.len() {
            0 => Outcome::Forward(()),
            1 if is_valid(keys[0]) => Outcome::Success(ApiKey(keys[0].to_string())),
            1 => Outcome::Forward(()),
            _ => Outcome::Forward(()),
        }
    }
}

lazy_static! {
    static ref CACHE: Mutex<HashMap<String, Response>> = Mutex::new(HashMap::new());
}

#[get("/")]
fn index() -> &'static str {
    "I'll put an UI here later"
}

#[get("/request/<duration>/<url>", rank = 1)]
fn request(_key: ApiKey, duration: u64, url: Base64String) -> Option<String> {
    let mut cache = CACHE.lock().unwrap();
    let req_time = now();
    match cache.get(&url.0) {
        Some(r) if r.timestamp + duration > req_time => Some(r.cache.to_owned()),
        _ => match reqwest::blocking::get(&url.0).ok()?.text().ok() {
            Some(body) => {
                cache.insert(
                    url.0,
                    Response {
                        timestamp: req_time,
                        cache: body.to_string(),
                    },
                );
                Some(body)
            }
            _ => None,
        },
    }
}

#[get("/request/<duration>/<url>", rank = 2)]
fn request_alt(duration: usize, url: Base64String) -> &'static str {
    "Invalid auth!"
}

fn main() {
    rocket::ignite()
        .mount("/", routes![index, request, request_alt])
        .launch();
}
