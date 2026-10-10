FROM alpine:3.19
ADD ./app /srv/app
ADD config.json /etc/config.json
CMD ["/srv/app"]
