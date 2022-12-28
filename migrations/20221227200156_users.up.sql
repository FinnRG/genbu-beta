create table "user" (
  id uuid primary key,
  name text not null,
  email text collate "case_insensitive" unique not null,
  created_at timestamptz not null,
  avatar uuid,
  hash text not null
);
