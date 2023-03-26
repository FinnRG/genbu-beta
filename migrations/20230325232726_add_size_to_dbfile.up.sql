alter table "file" add column size int8 not null default 0;
alter table "file" alter column size drop default;
alter table "file" alter column lock type text using lock::text;
