use actix_web::{get, web, App, HttpServer, Responder};
mod repositories;
pub use repositories::AppState;
pub use repositories::RedisActor;
pub use repositories::InfoCommandGet;
pub use repositories::InfoCommandSet;
pub use repositories::InfoCommandDel;
use std::env;
use std::{collections::HashMap, pin::Pin, sync::RwLock};
use redis::{Client, aio::MultiplexedConnection};
use actix::prelude::*;
use actix::Message;

impl RedisActor {
    pub async fn new(redis_url: String) -> Self {
        let client = Client::open(redis_url).unwrap();// not recommended
        let (conn, call) = client.get_multiplexed_async_connection().await.unwrap();
        actix_rt::spawn(call);
        RedisActor { conn }
    }
}

impl Handler<InfoCommandGet> for RedisActor {
    type Result = ResponseFuture<Result<Option<String>, redis::RedisError>>;
    fn handle(&mut self, _msg: InfoCommandGet, _: &mut Self::Context) -> Self::Result {
        let mut con = self.conn.clone();
        let mut cmd = redis::cmd(&_msg.command);

        let fut = async move {
            cmd
                .arg(&_msg.arg)
                .query_async(&mut con)
                .await
        };
        Box::pin(fut)
    }
}

impl Handler<InfoCommandSet> for RedisActor {

    type Result = ResponseFuture<Result<Option<String>, redis::RedisError>>;

    fn handle(&mut self, _msg: InfoCommandSet, _: &mut Self::Context) ->  Self::Result  {
        let mut con = self.conn.clone();
        let mut cmd = redis::cmd(&_msg.command);
        let mut pipe = redis::pipe();
        let fut = async move {
            pipe.cmd("SET")
            .arg(_msg.arg)
            .arg(_msg.arg2)
            .ignore();

            return pipe.query_async(&mut con).await; 
        };
        Box::pin(fut)
    }
}


impl Handler<InfoCommandDel> for RedisActor {
    
    type Result = ResponseFuture<Result<Option<String>, redis::RedisError>>;

    fn handle(&mut self, _msg: InfoCommandDel, _: &mut Self::Context) ->Self::Result {
        let mut con = self.conn.clone();
        let mut cmd = redis::cmd(&_msg.command);
        let mut pipe = redis::pipe();
        let fut = async move {
            pipe.cmd("DEL")
            .arg(_msg.arg)
            .ignore();

            return pipe.query_async(&mut con).await; 
        };
        Box::pin(fut)
    }
}

impl Actor for RedisActor {
    type Context = Context<Self>;
}


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let redis_url: String = env::var("REDIS_URL_GSTREAMER_PIPELINE").unwrap_or("none".to_string());
    let actor = RedisActor::new(redis_url).await;
    let addr = actor.start();
    
    HttpServer::new(move || App::new()
    .app_data(web::Data::new(RwLock::new(AppState {
        map: HashMap::new(),
        conn: addr.clone()
    })).clone())
    .service(web::scope("/user").configure(repositories::user_repository::init_routes)))
        .bind("0.0.0.0:8080")?
        .run()
        .await
}
