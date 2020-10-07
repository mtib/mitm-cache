#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate rocket;

use serde::Serialize;
use std::{collections::HashMap, sync::Mutex, sync::MutexGuard, time::SystemTime};

use rocket::Outcome;
use rocket::{
    http::RawStr,
    request::{self, FromParam, FromRequest, Request},
};
use rocket_contrib::templates::Template;

#[derive(Debug, Serialize)]
struct ApiKey(String);

struct Base64String(String);

impl<'r> FromParam<'r> for Base64String {
    type Error = &'r str;

    fn from_param(param: &'r RawStr) -> Result<Self, Self::Error> {
        Ok(Base64String(
            String::from_utf8(
                base64::decode_config(param, base64::URL_SAFE).map_err(|_| "Decode Error")?,
            )
            .map_err(|_| "String Encode Error")?,
        ))
    }
}

#[derive(Debug, Serialize, Clone)]
struct Response {
    timestamp: u64,
    cache: String,
    hits: usize,
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

impl<'a> FromParam<'a> for ApiKey {
    type Error = ();

    fn from_param(param: &'a RawStr) -> Result<Self, Self::Error> {
        if is_valid(param) {
            Ok(ApiKey(param.to_string()))
        } else {
            Err(())
        }
    }
}

lazy_static! {
    static ref CACHE: Mutex<HashMap<String, Response>> = Mutex::new(HashMap::new());
}

#[derive(Debug, Serialize)]
struct Memory {}

#[derive(Debug, Serialize)]
struct ResponseSummary {
    timestamp: u64,
    bytes: usize,
    hits: usize,
}

impl From<&Response> for ResponseSummary {
    fn from(r: &Response) -> Self {
        ResponseSummary {
            timestamp: r.timestamp,
            bytes: r.cache.len(),
            hits: r.hits,
        }
    }
}

#[derive(Debug, Serialize)]
struct IndexData<'r> {
    mem: Memory,
    cache: Vec<(&'r String, ResponseSummary)>,
    key: ApiKey,
}

#[get("/")]
fn index(key: ApiKey) -> Template {
    list_template(key)
}

#[get("/<key>", rank = 2)]
fn index_alt(key: ApiKey) -> Template {
    list_template(key)
}

fn list_template(key: ApiKey) -> Template {
    let cache = CACHE.lock().unwrap();
    let mut summary: Vec<(_, ResponseSummary)> = Vec::with_capacity(cache.len());
    for (key, val) in cache.iter() {
        summary.push((key, val.into()));
    }
    summary.sort_by_key(|a| u64::MAX - a.1.timestamp);
    Template::render(
        "list",
        IndexData {
            mem: Memory {},
            cache: summary,
            key,
        },
    )
}

#[get("/", rank = 3)]
fn index_noauth() -> &'static str {
    "Looking at the UI still requires authentication"
}

#[get("/request/<duration>/<url>", rank = 1)]
fn request_auth_by_header(_key: ApiKey, duration: u64, url: Base64String) -> Option<String> {
    proxy(duration, url)
}

#[get("/request/<duration>/<url>/<key>", rank = 2)]
fn request_auth_by_param(duration: u64, url: Base64String, key: ApiKey) -> Option<String> {
    proxy(duration, url)
}

fn proxy(duration: u64, url: Base64String) -> Option<String> {
    let mut cache = CACHE.lock().unwrap();
    let req_time = now();
    match cache.get_mut(&url.0) {
        Some(r) if r.timestamp + duration > req_time => {
            r.hits += 1;
            Some(r.cache.to_owned())
        }
        _ => match reqwest::blocking::get(&url.0).ok()?.text().ok() {
            Some(body) => {
                cache.insert(
                    url.0,
                    Response {
                        timestamp: req_time,
                        cache: body.to_string(),
                        hits: 1,
                    },
                );
                Some(body)
            }
            _ => None,
        },
    }
}

#[get("/request/<duration>/<url>", rank = 3)]
fn request_alt(
    _key: Option<ApiKey>,
    duration: &RawStr,
    url: Result<Base64String, &str>,
) -> Template {
    #[derive(Debug, Serialize)]
    struct FailureContext<'r> {
        duration: &'r str,
        url: &'r str,
        error: Option<&'r str>,
    }
    Template::render(
        "400",
        FailureContext {
            duration,
            url: match &url {
                Ok(b) => &b.0,
                Err(_) => "Not available",
            },
            error: match &url {
                Ok(_) => match _key {
                    Some(_) => None,
                    None => Some("Key error"),
                },
                Err(s) => Some(s),
            },
        },
    )
}

fn main() {
    rocket::ignite()
        .attach(Template::fairing())
        .mount(
            "/",
            routes![
                index,
                index_alt,
                index_noauth,
                request_auth_by_header,
                request_auth_by_param,
                request_alt
            ],
        )
        .launch();
}
