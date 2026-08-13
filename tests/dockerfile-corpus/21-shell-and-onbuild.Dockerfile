FROM alpine:3.19
SHELL ["/bin/ash", "-eo", "pipefail", "-c"]
ONBUILD COPY . /app
STOPSIGNAL SIGTERM
CMD ["sh"]
