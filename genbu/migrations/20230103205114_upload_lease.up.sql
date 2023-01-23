create type bucket as enum ('profileimages', 'videofiles', 'userfiles', 'notebookfiles');

create table "upload_lease" (
    id uuid primary key,
    s3_upload_id text not null,
    owner uuid not null,
    name text not null,
    bucket bucket not null,
    completed boolean not null default false,
    size int8 not null,
    created_at timestamptz not null default now(),
    expires_at timestamptz not null,
    constraint fk_user
        foreign key(owner)
            references "user"(id)
)