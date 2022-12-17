#!/bin/bash

docker run -itd \
    --restart always \
    -e POSTGRES_USER=genbu \
    -e POSTGRES_PASSWORD=strong_password \
    -e POSTGRES_DB=genbu \
    -p 5432:5432 \
    -v /srv/docker/postgresql:/var/lib/postgresql \
    postgres:11;
