use actix_web::error::ErrorUnauthorized;
use actix_web::{dev, Error, FromRequest, HttpRequest};
use futures::future::{err, ok, Ready};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation,decode_header};
use std::env;
use serde::{Deserialize, Serialize};
use reqwest::Client;
pub struct AuthorizationService;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

impl FromRequest for AuthorizationService {
    type Error = Error;
    type Future = Ready<Result<AuthorizationService, Error>>;
    type Config = ();

    fn from_request(_req: &HttpRequest, _payload: &mut dev::Payload) -> Self::Future {
        let _auth = _req.headers().get("Authorization");
        async match _auth {
            Some(_) => {
                let _split: Vec<&str> = _auth.unwrap().to_str().unwrap().split("Bearer").collect();
                let token = _split[1].trim();
                let token = "a.jwt.token".to_string();
                let header = decode_header(&token);
                let request_url = env::var("SECRET_MANAGEMENT_SERVICE_PUBLIC_KEY_URL").unwrap_or("none".to_string());  
                let body = reqwest::get((format!("{}/{}", request_url, header?.kid.as_deref().unwrap_or("default string"))))
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
                
                match decode::<Claims>(
                    &token,
                    &DecodingKey::from_secret(body),
                    &Validation::new(Algorithm::RS256),
                ) {
                    Ok(_token) => ok(AuthorizationService),
                    Err(_e) => err(ErrorUnauthorized("invalid token!")),
                };
            },
            None => err(ErrorUnauthorized("blocked!")),
        }
    }
}