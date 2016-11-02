use std::collections::HashMap;
use std::iter::FromIterator;

use redis;
use redis::Commands;
use r2d2;
use r2d2_redis::RedisConnectionManager;
use iron;
use rand;
use rand::Rng;

use RawSession;
use SessionBackend;

use errors::*;
use iron::prelude::*;

const COOKIE_NAME: &'static str = "iron_session_id";

type RedisPool = r2d2::Pool<RedisConnectionManager>;

pub struct RedisSession {
    session_id: String,
    pool: RedisPool,
}

impl RawSession for RedisSession {
    fn get_raw(&self, key: &str) -> IronResult<Option<&str>> {
        let conn = itry!(self.pool.get());

        Ok(itry!(conn.hget(self.session_id, key)))
    }

    fn set_raw(&mut self, key: &str, value: String) -> IronResult<()> {
        itry!(self.pool.get().unwrap().hset(self.session_id, key, value));
        Ok(())
    }

    fn clear(&mut self) -> IronResult<()> {
        itry!(self.pool.get().unwrap().del(self.session_id).unwrap());
        self.session_id = "".to_owned();
        Ok(())
    }

    fn write(&self, res: &mut Response) -> IronResult<()> {
        let cookie = iron::headers::CookiePair::new(
            COOKIE_NAME.to_owned(),
            self.session_id
        );
        if let Some(mut cookies) = res.headers.get_mut::<iron::headers::SetCookie>() {
            debug_assert!(cookies.iter().all(|cookie| cookie.name != COOKIE_NAME));
            cookies.push(cookie);
            return;
        }
        res.headers.set(iron::headers::SetCookie(vec![cookie]));
        Ok(())
    }
}

pub struct RedisBackend {
    pool: RedisPool
}

impl RedisBackend {
    pub fn new<T: redis::IntoConnectionInfo>(params: T) -> Result<Self> {
        let config = Default::default();
        let manager = try!(RedisConnectionManager::new(params).chain_err(|| "Couldn't create redis connection manager"));
        let pool = try!(r2d2::Pool::new(config, manager).chain_err(|| "Couldn't create redis connection pool"));

        Ok(RedisBackend { pool: pool })
    }
}


impl SessionBackend for RedisBackend {
    type S = RedisSession;

    fn from_request(&self, req: &mut Request) -> Self::S {
        let session_id = req.headers.get::<iron::headers::Cookie>()
            .map(|header| header.to_cookie_jar(b""))  // FIXME: Our cookies are unsigned. Why do I need to specify a key?
            .and_then(|jar| jar.find(COOKIE_NAME))
            .map(|cookie| cookie.value)
            .unwrap_or_else(|| {
                let mut rng = rand::OsRng::new().unwrap();
                String::from_iter(rng.gen_ascii_chars())
            });

        RedisSession {
            session_id: session_id,
            pool: self.pool.clone(),
        }
    }
}
