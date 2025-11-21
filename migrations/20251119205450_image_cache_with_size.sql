
drop table image_cache;

create table image_cache(
       item_id text not null,
       image_type text not null,
       tag text not null,
       size_x integer not null,
       size_y integer not null,
       val blob not null,
       added integer not null default (unixepoch()),
       unique (item_id, image_type, tag, size_x, size_y) on conflict replace
) strict;
