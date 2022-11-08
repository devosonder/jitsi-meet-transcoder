#![feature(libc)]
extern crate libc;
extern crate strfmt;
use actix::Addr;
use futures::FutureExt;
use strfmt::strfmt;
use std::env;
use std::f32::consts::E;
use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use jsonwebtoken::{decode ,decode_header,  Algorithm, DecodingKey, Validation};
use std::process::Command;
use std::time::{SystemTime};
use rand::distributions::{Alphanumeric, DistString};
use reqwest::header::{HeaderMap};
use redis::{Client, aio::MultiplexedConnection};
use actix::Message;
use std::panic;



#[derive(Message, Debug)]
#[rtype(result = "Result<Option<String>, redis::RedisError>")]
pub struct InfoCommandGet {
    pub command: String,
    pub arg: String,
    pub arg2: Option<String>
}


#[derive(Message)]
#[rtype(result = "Result<Option<String>, redis::RedisError>")]
pub struct InfoCommandSet {
    pub command: String,
    pub arg: String,
    pub arg2: String
}

#[derive(Message)]
#[rtype(result = "Result<Option<String>, redis::RedisError>")]
pub struct InfoCommandDel {
    pub command: String,
    pub arg: String
}


#[derive(Clone)]
pub struct RedisActor {
    pub conn: MultiplexedConnection
}

// need to change this later when load balancer giving all correct IP's
static RTMP_OUT_LOCATION: &str = "rtmp://3.7.148.117:1935";
use std::{collections::HashMap, sync::RwLock};
use libc::{kill, SIGTERM};

// This struct represents state
#[derive(Clone)]
pub struct AppState {
    pub map: HashMap<String,  String>,
    pub conn: Addr<RedisActor>
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

#[derive(Serialize)]
struct ResponseStart {
    started: bool,
    hls_url:  String,
    dash_url: String,
    mp3_url: String,
    aac_url: String,
    rtmp_url: String,
    flv_url: String,
    srt_url: String,
}

#[derive(Serialize)]
struct ResponseStop {
    started: bool
}


#[derive(Serialize)]
struct ResponseRecordingAlreadyStarted {
    started: bool,
    message: String,
}

#[get("/healthz")]
async fn get_health_status() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/json")
        .body("Healthy!")
}

async fn send_data_to_pricing_service(room_name: String, action: String, authorization_header: String) {
    let mut map = HashMap::new();
    let st = SystemTime::now().into();
    let st_str: String=  humantime::format_rfc3339_seconds(st).to_string();
    map.insert("roomJid", format!("{}@muc.sariska.io", room_name));
    map.insert("timestamp",  st_str);
    map.insert("action", action);
    map.insert("type", "stream".to_owned());

    let service_secret_key = match env::var_os("X_SERVICE_TOKEN") {
        Some(v) => v.into_string().unwrap(),
        None => panic!("$X-SERVICE-TOKEN is not set")
    };

    let mut headers = HeaderMap::new();
        headers.insert("Authorization", authorization_header.parse().unwrap());
        headers.insert("X-SERVICE-TOKEN", service_secret_key.parse().unwrap());

    let client = reqwest::Client::new();
    let res = client.post( env::var("RECORDING_SERVICE_URL").unwrap_or("none".to_string()))
        .headers(headers)
        .json(&map)
        .send()
        .await;
}

#[get("/startRecording")]
async fn start_recording(_req: HttpRequest, app_state: web::Data<RwLock<AppState>>) -> HttpResponse {
    let params = web::Query::<Params>::from_query(_req.query_string()).unwrap();
    println!("{:?}", params);

    let app: String =  Alphanumeric.sample_string(&mut rand::thread_rng(), 16).to_lowercase();
    let stream: String =  Alphanumeric.sample_string(&mut rand::thread_rng(), 16).to_lowercase();
    let mut redis_actor = &app_state.read().unwrap().conn;

    let comm = InfoCommandGet {
        command: "GET".to_string(),
        arg: format!("production::room_key::{}", params.room_name).to_string(),
        arg2: None,
    };
    
    let mut run_async = || async move {
        redis_actor.send(comm).await
    };

    let result = async move {
        // AssertUnwindSafe moved to the future
        std::panic::AssertUnwindSafe(run_async()).catch_unwind().await
    }.await;        
    match result {
        Ok(Ok(Ok(Some(value))))  => {
            let obj = ResponseRecordingAlreadyStarted {
                started: false,
                message: "Recording already started".to_string()
            };
            return HttpResponse::Ok().json(obj)
        },
        Ok(Ok(Ok(None))) => (),
        Err(_)=> (),
        Ok(Err(_))=>(),
        Ok(Ok(Err(_)))=>()
    }
    let comm = InfoCommandSet {
        command: "SET".to_string(),
        arg2: params.room_name.to_string(),
        arg: format!("production::room_key::{}", params.room_name).to_string()
    };
    redis_actor.send(comm).await;
    
    let location = format!("{}/{}/{}", RTMP_OUT_LOCATION, app, stream);
    println!("{}", location);

    let gstreamer_pipeline = format!("./gst-meet --web-socket-url=wss://api.sariska.io/api/v1/media/websocket \
     --xmpp-domain=sariska.io  --muc-domain=muc.sariska.io \
     --recv-video-scale-width=640 \
     --recv-video-scale-height=360 \
     --room-name={} \
     --recv-pipeline='audiomixer name=audio ! voaacenc bitrate=128000 ! mux. compositor name=video sink_1::xpos=640 \
        ! queue \
        ! x264enc cabac=1 bframes=2 ref=1 \
        ! video/x-h264,profile=main \
        ! flvmux streamable=true name=mux \
        ! rtmpsink location={}'", params.room_name, location);

    println!("{}", gstreamer_pipeline);

    let _auth = _req.headers().get("Authorization");
    
    let _split: Vec<&str> = _auth.unwrap().to_str().unwrap().split("Bearer").collect();
    let token = _split[1].trim();
    let header  =  decode_header(&token);
    let request_url = env::var("SECRET_MANAGEMENT_SERVICE_PUBLIC_KEY_URL").unwrap_or("none".to_string());
    let header_data = match header {
        Ok(_token) => _token.kid,
        Err(_e) => None,
    };
    let kid = header_data.as_deref().unwrap_or("default string");
        // create a Sha256 object
    let api_key_url =  format!("{}/{}", request_url, kid);
    

    let response = reqwest::get(api_key_url)
        .await
        .unwrap()
        .text()
        .await;


    let public_key  = match response {
        Ok(_publickey)=>_publickey,
        _ => "default string".to_string(),
    };

    let deserialized: PublicKey = serde_json::from_str(&public_key).unwrap();
    let decoded_claims = decode::<Claims>(
        &token,
        &DecodingKey::from_rsa_components(&deserialized.n, &deserialized.e),
        &Validation::new(Algorithm::RS256));

        match decoded_claims {
            Ok(v) => {
            },
            Err(e) => {
              println!("Error decoding json: {:?}", e);
              return HttpResponse::Unauthorized().json("{}");
            },
        }

        let child = Command::new("sh")
        .arg("-c")
        .arg(gstreamer_pipeline)
        .spawn()
        .expect("failed to execute process");
        app_state.write().unwrap().map.insert(params.room_name.to_string(), child.id().to_string());
        // child_processes.insert(params.room_name.to_string(),child);

        send_data_to_pricing_service(params.room_name.to_string(), "start".to_owned(), token.to_owned()).await;

        let obj = create_response_start(app.clone(), stream.clone());
        HttpResponse::Ok().json(obj)
}

fn create_response_start(app :String, stream: String) -> ResponseStart {
    let obj = ResponseStart {
        started: true,
        hls_url: format!("https://edge.sariska.io/play/hls/{}/{}.m3u8", app, stream),
        dash_url: format!("https://edge.sariska.io/play/dash/{}/{}.mpd", app, stream),
        mp3_url: format!("https://edge.sariska.io/play/mp3/{}/{}.mp3",app, stream),
        aac_url: format!("https://edge.sariska.io/play/aac/{}/{}.aac", app, stream),
        rtmp_url: format!("rtmp://a0f32a67911bd43b08097a2a99e6eac6-b0099fdbb77fd73a.elb.ap-south-1.amazonaws.com:1935/{}{}", app, stream),
        flv_url: format!("https://edge.sariska.io/play/flv/{}/{}.flv", app, stream),
        srt_url: format!("srt://a23d4c35634a24dd8a0a932f57f40380-f2266220d83cf36b.elb.ap-south-1.amazonaws.com:10080?streamid=#!::r={}/{},m=request", app, stream),
    };
    obj
}

#[get("/stopRecording")]
async fn stop_recording(_req: HttpRequest, app_state: web::Data<RwLock<AppState>>) -> HttpResponse {
    let params = web::Query::<Params>::from_query(_req.query_string()).unwrap();
    let _auth = _req.headers().get("Authorization");
    let _split: Vec<&str> = _auth.unwrap().to_str().unwrap().split("Bearer").collect();
    let token = _split[1].trim();

    let mut redis_actor = &app_state.read().unwrap().conn;

    let comm = InfoCommandDel {
        command: "DEL".to_string(),
        arg: format!("production::room_key::{}", params.room_name).to_string(),
    };
    
    let mut run_async = || async move {
        redis_actor.send(comm).await
    };

    let result = async move {
        // AssertUnwindSafe moved to the future
        std::panic::AssertUnwindSafe(run_async()).catch_unwind().await
    }.await;
    
    println!("{:?}", result);

    let child_ids = &app_state.read().unwrap().map;
    let child_os_id = child_ids.get(&params.room_name.to_string());
    let my_int = child_os_id.unwrap().parse::<i32>().unwrap();
    unsafe {
        kill(my_int, SIGTERM);
    }
    send_data_to_pricing_service(params.room_name.to_string(), "stop".to_owned(), token.to_owned()).await;
    let obj = ResponseStop {
        started: false,
    };
    HttpResponse::Ok().json(obj)
}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(start_recording);
    cfg.service(stop_recording);
    cfg.service(get_health_status);
}

