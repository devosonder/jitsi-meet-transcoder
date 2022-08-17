use actix_web::{get, App, HttpResponse, HttpServer, Result};
use std::process::Command;

#[get("/health")]
pub async fn health() -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().body("success".to_string()))
}

#[get("/echo")]
async fn echo(req_body: String) ->  Result<HttpResponse> {
    let output = if cfg!(target_os = "windows") {
        Command::new("cmd")
                .args(["/C", "echo hello"])
                .output()
                .expect("failed to execute process")
    } else {
        Command::new("sh")
                .arg("-c")
                .arg("./rclone.sh")
                .output()
                .expect("failed to execute process")
    };
    // let hello = output.stdout;

    let stdout = String::from_utf8(output.stdout).unwrap();

    println!("{} here........", stdout);

    // print!("Logging my_string {}", output);

    // println!("{}", output);
    Ok(HttpResponse::Ok().body("success".to_string()))

}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    println!("Starting Web server");
    HttpServer::new(|| App::new()
            .service(echo)
                    .service(health))
        .bind("0.0.0.0:8080")?
        .run()
        .await
}
