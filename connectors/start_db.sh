#!/bin/bash

podman run -itd \
    --restart always \
    -e POSTGRES_USER=genbu \
    -e POSTGRES_PASSWORD=strong_password \
    -e POSTGRES_DB=genbu \
    -p 5432:5432 \
    postgres:11;
