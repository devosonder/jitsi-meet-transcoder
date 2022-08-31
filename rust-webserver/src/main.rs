use actix_web::{get, web, App, HttpServer, Responder};
mod repositories;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| App::new()
    .service(web::scope("/user").configure(repositories::user_repository::init_routes)))
        .bind("127.0.0.1:8080")?
        .run()
        .await
}
