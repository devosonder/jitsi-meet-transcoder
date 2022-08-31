use std::env;
use actix_web::{post, get, web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use jsonwebtoken::{decode ,decode_header,  Algorithm, DecodingKey, Validation};
use actix_web::{http::header::ContentType};
use futures::future::{err, ok, Ready};
use actix_web::error::ErrorUnauthorized;

#[derive(Debug, Serialize, Deserialize)]
pub struct Context {
    pub group: String
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub context: Context
}

#[derive(Serialize, Deserialize, Debug)]
struct PublicKey {
    e: String,
    n: String,
    kty: String
}

#[derive(Debug, Deserialize)]
struct Params {
    room_name: String,
}



#[get("/startRecorging")]
async fn start_recorging(_req: HttpRequest) -> HttpResponse {
    let params = web::Query::<Params>::from_query(_req.query_string()).unwrap();
    println!("{:?}", params);
    
    let _auth = _req.headers().get("Authorization");
    let _split: Vec<&str> = _auth.unwrap().to_str().unwrap().split("Bearer").collect();
    let token = _split[1].trim();
    let header  =  decode_header(&token);
    let request_url = env::var("SECRET_MANAGEMENT_SERVICE_PUBLIC_KEY_URL").unwrap_or("none".to_string());  
    let kid = "";

   let headerData = match header {
        Ok(_token) => _token.kid,
        Err(_e) => None,
    };
    let kid = headerData.as_deref().unwrap_or("default string");
        // create a Sha256 object
    let api_key_url =  format!("{}/{}", request_url, kid);
    let response = reqwest::get(api_key_url)
        .await
        .unwrap()
        .text()
        .await;
    let publicKey  = match response {
        Ok(_publickey)=>_publickey,
        _ => "default string".to_string(),
    };
    
    let deserialized: PublicKey = serde_json::from_str(&publicKey).unwrap();
    let decoded_claims = decode::<Claims>(
        &token,
        &DecodingKey::from_rsa_components(&"deserialized.n", &deserialized.e),
        &Validation::new(Algorithm::RS256));

        let decoded;
        let mut error = false;
        match decoded_claims {
            Ok(v) => {
                decoded = v;
            },
            Err(e) => {
              error = true;
            },
        }
        
        if error == true {
            println!("unauthorized");
            return HttpResponse::Unauthorized().json("{}");
        }

        

        HttpResponse::Ok().json("{}")

}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(start_recorging);
}
