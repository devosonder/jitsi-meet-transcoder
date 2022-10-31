use actix_web::{get, web, App, HttpServer, Responder};
mod repositories;
pub use repositories::AppState;
use std::{collections::HashMap, pin::Pin, sync::RwLock};


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| App::new()
    .app_data(web::Data::new(RwLock::new(AppState {
        map: HashMap::new(),
    })).clone())
    .service(web::scope("/user").configure(repositories::user_repository::init_routes)))
        .bind("0.0.0.0:8080")?
        .run()
        .await
}
