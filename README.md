## Description

- lib-gst-meet is server side rust implementation( lib-jitsi-meet for browser ), it allows to record and stream jitsi meet conferences without using headless chrome to save cost and resouces.

## Components 

 - Gstreamer
 - Rclone to upload recordings to (AWS, GCP, AZURE and others)
 - Redis
 - actix-web server
 - lib-gst-meet is server side rust implementation of lib-jitsi-meet javascript library
 - autoscalable pipeline

## Deployment 
 - please refer to Makefile
 
