
FROM ekidd/rust-musl-builder:stable as builder
RUN USER=root cargo new --bin actix-web-docker-example

COPY ./rust-webserver  ./actix-web-docker-example

WORKDIR ./actix-web-docker-example
RUN cargo build --release
RUN rm -r ./target/x86_64-unknown-linux-musl/release/deps
RUN cargo build --release

FROM docker.io/library/alpine:3.16 AS builder1

COPY ./streaming-service-bridge  ./streaming-service-bridge
WORKDIR ./streaming-service-bridge
RUN apk --no-cache add gstreamer-dev gst-plugins-base-dev 
RUN apk --no-cache add build-base openssl-dev cargo libnice-dev
RUN cargo build --release -p gst-meet

FROM docker.io/library/alpine:3.16
RUN apk update
RUN apk --no-cache add curl
RUN apk --no-cache add sed
RUN apk add --no-cache --upgrade bash
RUN apk --no-cache add jq
RUN apk --no-cache add unzip
RUN apk --no-cache add gstreamer gst-plugins-good gst-plugins-bad gst-plugins-ugly gst-libav libnice-gstreamer
RUN apk --no-cache add libnice openssl libnice

RUN mkdir -p /home/appuser/.config/rclone/


ENV RCLONE_VER=v1.59.1 \
    ARCH=amd64 \
    SUBCMD="" \
    CONFIG="--config /usr/src/app/rclone.conf" \
    PARAMS=""

RUN curl -O "https://downloads.rclone.org/v1.59.1/rclone-v1.59.1-linux-amd64.zip"
RUN unzip rclone-v1.59.1-linux-amd64.zip
RUN cd rclone-v1.59.1-linux-amd64
RUN cp rclone-v1.59.1-linux-amd64/rclone /usr/bin/
RUN chown root:root /usr/bin/rclone
RUN chmod 755 /usr/bin/rclone
RUN mkdir -p /usr/share/man/man1
RUN cp rclone-v1.59.1-linux-amd64/rclone.1 /usr/share/man/man1/
RUN rm -f rclone-v1.59.1-linux-amd64.zip
RUN rm -r rclone-v1.59.1-linux-amd64
ARG APP=/usr/src/app
EXPOSE 8080

ENV TZ=Etc/UTC \
    APP_USER=appuser

RUN addgroup -S $APP_USER \
    && adduser -S -g $APP_USER $APP_USER

RUN apk update \
    && apk add --no-cache ca-certificates tzdata \
    && rm -rf /var/cache/apk/*
COPY ./rust-webserver/rclone.sh  /usr/src/app/
COPY ./rust-webserver/rclone.conf  /home/appuser/.config/rclone/
COPY --from=builder1 /streaming-service-bridge/target/release/gst-meet  /usr/src/app/
COPY --from=builder /home/rust/src/actix-web-docker-example/target/x86_64-unknown-linux-musl/release/actix-web-docker-example ${APP}/actix-web-docker-example
RUN chown -R $APP_USER:$APP_USER ${APP}
USER $APP_USER
WORKDIR ${APP}

CMD ["./actix-web-docker-example"]
