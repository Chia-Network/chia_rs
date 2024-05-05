FROM alpine

RUN apk update
RUN apk add \
    bash \
    build-base \
    python3 \
    python3-dev \
    rust \
    cargo

WORKDIR /var/data
COPY . .

ENTRYPOINT ["tail", "-f", "/dev/null"]
