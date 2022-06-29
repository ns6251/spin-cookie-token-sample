use anyhow::Result;
use cookie::{Cookie, CookieJar};
use http::header::{CONTENT_TYPE, SET_COOKIE};
use spin_sdk::{
    http::{Request, Response},
    http_component, redis,
};

const REDIS_ADDRESS_ENV: &str = "REDIS_ADDRESS";

fn get_token_from_request(req: &Request) -> Option<String> {
    let mut jar = CookieJar::new();
    for header in req.headers().get_all("Cookie").iter() {
        let rs = match std::str::from_utf8(header.as_bytes()) {
            Ok(s) => s,
            Err(_) => continue,
        };

        for cookie_str in rs.split(';').map(|s| s.trim()) {
            if let Ok(cookie) = Cookie::parse_encoded(cookie_str) {
                jar.add_original(cookie.into_owned());
            }
        }
    }

    jar.get("waiting-token").map(|cookie| cookie.value().into())
}

fn generate_token() -> String {
    let ulid = ulid::Ulid::new();
    ulid.to_string().to_lowercase()
}

fn store_token(token: &str) -> anyhow::Result<()> {
    let address = std::env::var(REDIS_ADDRESS_ENV)?;
    redis::set(&address, token, chrono::Utc::now().to_rfc3339().as_bytes())
        .map_err(|err| anyhow::anyhow!("{:?}", err))?;
    Ok(())
}

fn find_token(token: &str) -> anyhow::Result<String> {
    let address = std::env::var(REDIS_ADDRESS_ENV)?;

    // FIXME: Error Handling
    // 無効・不正なTokenである場合 `None` だと嬉しい
    let value = redis::get(&address, token)
        .map(|value| String::from_utf8(value))
        .map_err(|err| anyhow::anyhow!("{:?}", err))??;

    Ok(value)
}

#[http_component]
fn spin_hello_world(req: Request) -> Result<Response> {
    let token = get_token_from_request(&req);

    println!("{:?}", token);

    let mut resp = http::Response::builder()
        .header(CONTENT_TYPE, "text/plain")
        .status(200);

    match token {
        Some(token) => {
            let time = chrono::DateTime::parse_from_rfc3339(&find_token(&token)?)?;
            println!("{:?}", time);
        }
        None => {
            let token = generate_token();
            store_token(&token)?;

            let set_cookie = Cookie::build("waiting-token", token)
                .path("/hello")
                .finish();
            resp = resp.header(SET_COOKIE, set_cookie.to_string());
        }
    }

    Ok(resp.body(Some("Hello, Fermyon".into()))?)
}
