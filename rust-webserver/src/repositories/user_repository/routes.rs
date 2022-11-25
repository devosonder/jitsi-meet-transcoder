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
use async_process::{ Command, Stdio };
use std::time::{SystemTime};
use rand::distributions::{Alphanumeric, DistString};
use reqwest::header::{HeaderMap};
use redis::{Client, aio::MultiplexedConnection};
use actix::Message;
use std::panic;
use minreq;
use serde_json::Error;

#[derive(Message, Debug)]
#[rtype(result = "Result<Option<String>, redis::RedisError>")]
pub struct InfoCommandGet {
    pub command: String,
    pub arg: String,
    pub arg2: Option<String>
}


#[derive(Message, Debug)]
#[rtype(result = "Result<Option<String>, redis::RedisError>")]
pub struct InfoCommandSet {
    pub command: String,
    pub arg: String,
    pub arg2: String
}

#[derive(Message, Debug)]
#[rtype(result = "Result<Option<String>, redis::RedisError>")]
pub struct InfoCommandDel {
    pub command: String,
    pub arg: String
}

#[derive(Message, Debug)]
#[rtype(result = "Result<Option<String>, redis::RedisError>")]
pub struct InfoCommandPublish {
    pub command: String,
    pub channel: String,
    pub message: String
}

#[derive(Clone)]
pub struct RedisActor {
    pub conn: MultiplexedConnection
}

// need to change this later when load balancer giving all correct IP's
static RTMP_OUT_LOCATION_VIDEO: &str = "rtmp://3.7.148.117:1935";
static RTMP_OUT_LOCATION_AUDIO: &str = "rtmp://3.7.148.117:1935";

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

#[derive(Debug, Deserialize, Serialize)]
struct Params {
    room_name: String,
    is_audio: Option<bool>,
    is_vod: Option<bool>,
    is_recording: Option<bool>,
    stream_urls: Option<Vec<String>>,
    stream_keys: Option<Vec<StreamKeyDict>>
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct StreamKeyDict {
    stream_key: String,
    stream_value: String,
}

fn print_type_of<T>(_: &T) {
    println!("{}", std::any::type_name::<T>())
}

#[derive(Serialize)]
struct ResponseAudioStart {
    started: bool,
    hls_url:  String,
    dash_url: String,
    rtmp_url: String,
    aac_url: String,
    srt_url: String,
    hds_url: String,
}

#[derive(Serialize)]
struct ResponseVideoStart {
    started: bool,
    hls_url:  String,
    dash_url: String,
    rtmp_url: String,
    flv_url: String,
    srt_url: String,
    hds_url: String,
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



#[derive(Serialize, Deserialize, Debug)]
pub struct SetRoomInfo {
    pub hostname: String,
    pub process_id: String,
    pub room_name: String,
}


#[get("/healthz")]
async fn get_health_status() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/json")
        .body("Healthy!")
}

async fn send_data_to_pricing_service(room_name: String, action: String, authorization_header: String) -> serde_json::Result<()> {
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
    let json = serde_json::to_string(&map)?;
    let response = minreq::post(env::var("RECORDING_SERVICE_URL").unwrap_or("none".to_string()))
    .with_header("Authorization", authorization_header)
    .with_header("X-SERVICE-TOKEN", service_secret_key)
    .with_body(json)
    .send();
    Ok(())
}

#[get("/startRecording")]
async fn start_recording(_req: HttpRequest, app_state: web::Data<RwLock<AppState>>) -> HttpResponse {
    let params = web::Query::<Params>::from_query(_req.query_string()).unwrap();
    println!("{:?}", params);

    let app: String =  Alphanumeric.sample_string(&mut rand::thread_rng(), 16).to_lowercase();
    let stream: String =  Alphanumeric.sample_string(&mut rand::thread_rng(), 16).to_lowercase();
    let mut redis_actor = &app_state.read().unwrap().conn;
    let _auth = _req.headers().get("Authorization");

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

    let mut location;
    let gstreamer_pipeline;
    let _split: Vec<&str> = _auth.unwrap().to_str().unwrap().split("Bearer").collect();
    let token = _split[1].trim();

    print!("{:?} params.is_audio ", params.is_audio );
    if  let None = params.is_audio  {
        location = format!("{}/{}/{}", RTMP_OUT_LOCATION_VIDEO, app, stream);
        location = format!("{}?vhost=flv.sariska.io&token={}", location, token);
        gstreamer_pipeline = format!("./gst-meet --web-socket-url=wss://api.sariska.io/api/v1/media/websocket \
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
    } else {
        location = format!("{}/{}/{}", RTMP_OUT_LOCATION_AUDIO, app, stream);
        location = format!("{}?vhost=aac.sariska.io&token={}", location, token);
        gstreamer_pipeline = format!("./gst-meet --web-socket-url=wss://api.sariska.io/api/v1/media/websocket \
     --xmpp-domain=sariska.io  --muc-domain=muc.sariska.io \
     --room-name={} \
     --recv-pipeline='audiomixer name=audio ! voaacenc bitrate=128000 flvmux streamable=true name=mux \
        ! rtmpsink location={}'", params.room_name, location);
    }

    let encoded = serde_json::to_string(&Params {
        is_audio: params.is_audio,
        is_vod: params.is_vod,
        room_name: params.room_name.clone(),
        is_recording: params.is_recording.clone(),
        stream_keys: params.stream_keys.clone(),
        stream_urls: params.stream_urls.clone()
    });
    
    let encoded = match encoded {
        Ok(v) => v,
        _ => "test".to_owned()
    };

    println!("{:?}", encoded);

    location = format!("{}&param={}", location, encoded);
    println!("{:?}", location);


    println!("{}", gstreamer_pipeline);
    let header  =  decode_header(&token);
    let request_url = env::var("SECRET_MANAGEMENT_SERVICE_PUBLIC_KEY_URL").unwrap_or("none".to_string());
    let header_data = match header {
        Ok(_token) => _token.kid,
        Err(_e) => None,
    };
    let kid = header_data.as_deref().unwrap_or("default string");
        // create a Sha256 object
    let api_key_url =  format!("{}/{}", request_url, kid);
    println!("{:?}", api_key_url);

    let response = minreq::get(api_key_url).send();
    match response {
            Ok(response)=>{
                let public_key = response.as_str().unwrap_or("default");
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
            },
            _=>{
                return HttpResponse::Unauthorized().json("{}");
            }
    }

    let child = Command::new("sh")
    .arg("-c")
    .arg(gstreamer_pipeline)
    .spawn();

    let hostname = env::var("HOSTNAME").unwrap_or("none".to_string());
    let room_info = SetRoomInfo {
        room_name: params.room_name.to_string(),
        process_id: child.unwrap().id().to_string(),
        hostname: hostname
    };

    let comm = InfoCommandSet {
        command: "SET".to_string(),
        arg2: serde_json::to_string(&room_info).unwrap(),
        arg: format!("production::room_key::{}", params.room_name).to_string()
    };
    redis_actor.send(comm).await;
    send_data_to_pricing_service(params.room_name.to_string(), "start".to_owned(), token.to_owned()).await;
   
    match params.is_audio {
        None => {
            let obj = create_response_start_video(app.clone(), stream.clone());
            HttpResponse::Ok().json(obj)
        },
        Some(i) => {
            let obj = create_response_start_audio(app.clone(), stream.clone());
                HttpResponse::Ok().json(obj)
            },
        }
}

fn create_response_start_audio(app :String, stream: String) -> ResponseAudioStart {
    let obj = ResponseAudioStart {
        started: true,
        hls_url: format!("https://edge.sariska.io/play/hls/{}/{}.m3u8", app, stream),
        hds_url: format!("https://edge.sariska.io/play/hds/{}/{}.f4m", app, stream),
        dash_url: format!("https://edge.sariska.io/play/dash/{}/{}.mpd", app, stream),
        aac_url: format!("http://a1888dceaa234469683e47544f5f0d33-c703d14b936b1688.elb.ap-south-1.amazonaws.com:8080{}/{}.aac?vhost=aac.sariska.io", app, stream),
        rtmp_url: format!("rtmp://a1888dceaa234469683e47544f5f0d33-c703d14b936b1688.elb.ap-south-1.amazonaws.com:1935/{}{}?vhost=aac.sariska.io", app, stream),
        srt_url: format!("srt://a23d4c35634a24dd8a0a932f57f40380-f2266220d83cf36b.elb.ap-south-1.amazonaws.com:10080?streamid=#!::r={}/{},m=request&vhost=aac.sariska.io", app, stream),
    };
    obj
}

fn create_response_start_video(app :String, stream: String) -> ResponseVideoStart {
    let obj = ResponseVideoStart {
        started: true,
        hls_url: format!("https://edge.sariska.io/play/hls/{}/{}.m3u8", app, stream),
        hds_url: format!("https://edge.sariska.io/play/hds/{}/{}.f4m", app, stream),
        dash_url: format!("https://edge.sariska.io/play/dash/{}/{}.mpd", app, stream),
        rtmp_url: format!("rtmp://a1888dceaa234469683e47544f5f0d33-c703d14b936b1688.elb.ap-south-1.amazonaws.com:1935/{}{}?vhost=flv.sariska.io", app, stream),
        flv_url: format!("http://a1888dceaa234469683e47544f5f0d33-c703d14b936b1688.elb.ap-south-1.amazonaws.com:8080/{}/{}.flv?vhost=flv.sariska.io", app, stream),
        srt_url: format!("srt://a23d4c35634a24dd8a0a932f57f40380-f2266220d83cf36b.elb.ap-south-1.amazonaws.com:10080?streamid=#!::r={}/{},m=request&vhost=flv.sariska.io", app, stream),
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
           let room_info: SetRoomInfo = serde_json::from_str(&value).unwrap();
           let hostname = env::var("HOSTNAME").unwrap_or("none".to_string());
           println!("{:?}", room_info);
           if room_info.hostname == hostname {
                let my_int = room_info.process_id.parse::<i32>().unwrap();
                unsafe {
                    kill(my_int, SIGTERM);
                }
           } else {
                let comm = InfoCommandPublish {
                    command: "PUBLISH".to_string(),
                    channel: "sariska_channel".to_string(),
                    message: value
                };
                redis_actor.send(comm).await;
           }
        },
        Ok(Ok(Ok(None))) => (),
        Err(_)=> (),
        Ok(Err(_))=>(),
        Ok(Ok(Err(_)))=>()
    };

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

