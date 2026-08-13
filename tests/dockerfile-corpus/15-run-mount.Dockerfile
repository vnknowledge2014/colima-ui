# syntax=docker/dockerfile:1
FROM golang:1.22
WORKDIR /src
RUN --mount=type=cache,target=/root/.cache/go-build \
    --mount=type=bind,source=.,target=/src \
    go build -o /out/app ./...
