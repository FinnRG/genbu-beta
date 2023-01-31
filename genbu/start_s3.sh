podman run --name minio -d \
  -p 9000:9000 \
  -p 9001:9001 \
  -e MINIO_ROOT_USER="minioadmin" \
  -e MINIO_ROOT_PASSWORD="minioadmin" \
  bitnami/minio:latest