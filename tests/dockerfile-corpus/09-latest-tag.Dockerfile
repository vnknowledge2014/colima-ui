FROM ubuntu:latest
RUN apt-get update && apt-get install -y python3
CMD ["python3"]
