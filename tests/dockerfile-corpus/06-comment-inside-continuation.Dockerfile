FROM debian:bookworm
RUN apt-get update \
# the recommends flag keeps the image small
    && apt-get install -y --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/*
