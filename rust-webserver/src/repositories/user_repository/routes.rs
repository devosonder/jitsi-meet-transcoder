#![feature(libc)]
extern crate libc;

use std::env;
use actix_web::{post, get, web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use jsonwebtoken::{decode ,decode_header,  Algorithm, DecodingKey, Validation};
use actix_web::{http::header::ContentType};
use futures::future::{err, ok, Ready};
use actix_web::error::ErrorUnauthorized;
use std::process::Command;
static GST_MEET_PARAMS_AUDIO_AND_VIDEO: &str = "../streaming-service-bridge/target/debug/gst-meet --web-socket-url=wss://api.sariska.io/api/v1/media/websocket  --xmpp-domain=sariska.io  --muc-domain=muc.sariska.io  --room-name=roomname     --recv-pipeline='compositor name=video ! videoconvert ! queue ! x264enc ! mpegtsmux ! filesink location=testvideo.mp4'";
static GST_MEET_PARAMS_LIVESTREAM: &str = "../streaming-service-bridge/target/debug/gst-meet --web-socket-url=wss://api.sariska.io/api/v1/media/websocket \
--xmpp-domain=sariska.io  --muc-domain=muc.sariska.io \
 --recv-video-scale-width=640 \
 --recv-video-scale-height=360 \
 --room-name=roomname  \
 --recv-pipeline='compositor name=video sink_1::xpos=640 \
    ! queue \
    ! x264enc cabac=1 bframes=2 ref=1 \
    ! video/x-h264,profile=main \
    ! flvmux streamable=true name=mux \
    ! rtmpsink location=rtmp://43.205.21.202:1935/app/jitsistream \
    audiotestsrc is-live=1 wave=ticks \
       ! mux.'";

use std::process::Child;
use std::sync::Mutex;
use std::{collections::HashMap, pin::Pin, sync::RwLock};
use libc::{kill, SIGTERM};

// This struct represents state
#[derive(Clone)]
pub struct AppState {
    pub map: HashMap<String,  String>
}

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

fn print_type_of<T>(_: &T) {
    println!("{}", std::any::type_name::<T>())
}

#[get("/startRecording")]
async fn start_recorging(_req: HttpRequest, child_processes: web::Data<RwLock<AppState>>) -> HttpResponse {
    let params = web::Query::<Params>::from_query(_req.query_string()).unwrap();
    println!("{:?}", params);

    
//     let _auth = _req.headers().get("Authorization");
//     let _split: Vec<&str> = _auth.unwrap().to_str().unwrap().split("Bearer").collect();
//     let token = _split[1].trim();
//     let header  =  decode_header(&token);
//     let request_url = env::var("SECRET_MANAGEMENT_SERVICE_PUBLIC_KEY_URL").unwrap_or("none".to_string());  
//     let kid = "";

//    let headerData = match header {
//         Ok(_token) => _token.kid,
//         Err(_e) => None,
//     };
//     let kid = headerData.as_deref().unwrap_or("default string");
//         // create a Sha256 object
//     let api_key_url =  format!("{}/{}", request_url, kid);
//     let response = reqwest::get(api_key_url)
//         .await
//         .unwrap()
//         .text()
//         .await;
//     let publicKey  = match response {
//         Ok(_publickey)=>_publickey,
//         _ => "default string".to_string(),
//     };
    
//     let deserialized: PublicKey = serde_json::from_str(&publicKey).unwrap();
//     let decoded_claims = decode::<Claims>(
//         &token,
//         &DecodingKey::from_rsa_components(&"deserialized.n", &deserialized.e),
//         &Validation::new(Algorithm::RS256));

//         let decoded;
//         let mut error = false;
//         match decoded_claims {
//             Ok(v) => {
//                 decoded = v;
//             },
//             Err(e) => {
//               error = true;
//             },
//         }
        
//         if error == true {
//             println!("unauthorized");
//             return HttpResponse::Unauthorized().json("{}");
//         }


        // let output = Command::new("sh")
        // .spawn()
        // .expect("failed to execute process");
        let child = Command::new("sh")
        .arg("-c")
        .arg(GST_MEET_PARAMS_LIVESTREAM)
        .spawn()
        .expect("failed to execute process");
         child_processes.write().unwrap().map.insert(params.room_name.to_string(), child.id().to_string());

        // child_processes.insert(params.room_name.to_string(),child);
        // let hello = output.stdout;

        // println!("qwnenqwnenwq enqwne nqwen {:?}", output);

        HttpResponse::Ok().json("{}")
}

#[get("/stopRecording")]
async fn stop_recording(_req: HttpRequest, child_processes: web::Data<RwLock<AppState>>) -> HttpResponse {
    let params = web::Query::<Params>::from_query(_req.query_string()).unwrap();
    let mut child_ids = &child_processes.read().unwrap().map;
    let child_os_id = child_ids.get(&params.room_name.to_string());  
    let my_int = child_os_id.unwrap().parse::<i32>().unwrap();
    unsafe {
        kill(my_int, SIGTERM);
    }
    HttpResponse::Ok().json("{}")
}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(start_recorging);
    cfg.service(stop_recording);
}

