podman run --name minio -d \
  -p 9000:9000 \
  -p 9001:9001 \
  bitnami/minio:latest