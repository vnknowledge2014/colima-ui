# syntax=docker/dockerfile:1
FROM alpine:3.19
RUN <<EOF
apk add --no-cache curl
adduser -D app
EOF
USER app
