-- Add migration script here
create table image_cache(
       item_id text not null,
       image_type text not null,
       tag text not null,
       val blob not null,
       added integer not null default (unixepoch()),
       unique (item_id, image_type, tag) on conflict replace
) strict;
