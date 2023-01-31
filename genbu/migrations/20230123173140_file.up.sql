create table "file" (
    id uuid primary key,
    path text not null,
    lock uuid,
    lock_expires_at timestamptz,
    created_by uuid not null,
    created_at timestamptz not null default now(),
    constraint fk_user
        foreign key(created_by)
            references "user"(id)
)