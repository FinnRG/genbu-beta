create table if not exists "group" (
    group_id uuid primary key,
    name text not null,
    created_by uuid not null,
    created_at timestamptz not null default now(),
    constraint fk_created_by
        foreign key(created_by)
            references "user"(id)
);

create table if not exists "user_group" (
    group_id uuid not null,
    user_id uuid not null,
    primary key (group_id, user_id),
    constraint fk_group_id
        foreign key(group_id)
            references "group"(group_id),
    constraint fk_user_id
        foreign key(user_id)
            references "user"(id)
);