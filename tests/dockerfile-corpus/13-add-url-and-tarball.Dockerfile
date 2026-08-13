FROM alpine:3.19
ADD https://example.com/tool.sh /usr/local/bin/tool
ADD release.tar.gz /opt/
RUN chmod +x /usr/local/bin/tool
