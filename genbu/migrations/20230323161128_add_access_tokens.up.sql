create table if not exists "access_token" (
    token uuid not null unique default gen_random_uuid(),
    user_id uuid not null,
    file_id uuid not null,
    created_from inet not null,
    created_at timestamptz not null default now(),
    primary key (user_id, file_id),
    constraint fk_user_id
        foreign key(user_id)
            references "user"(id),
    constraint fk_file_id
        foreign key(file_id)
            references "file"(id)
)