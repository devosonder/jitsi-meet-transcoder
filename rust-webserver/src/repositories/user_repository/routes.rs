use crate::middlewares::auth::AuthorizationService;
use actix_web::http::StatusCode;
use actix_web::{post, get, web, HttpRequest, HttpResponse};

#[post("/startRecording")]
async fn startRecording(_: AuthorizationService) -> HttpResponse {

    
    HttpResponse::Ok().json("{}")
}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(startRecording);
}
