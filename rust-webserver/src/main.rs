
#[macro_use]
extern crate bson;

use actix_web::{middleware, web, App, HttpServer};
mod middlewares;
mod repositories;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .wrap(middleware::Logger::default())
            .service(web::scope("/media").configure(repositories::user_repository::init_routes))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
