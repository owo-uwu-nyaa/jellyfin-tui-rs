-- Add migration script here
create table creds(
       device_name text not null,
       client_name text not null,
       client_version text not null,
       user_name text not null,
       access_token text not null,
       added integer not null default (unixepoch()),
       unique (device_name, client_name, client_version, user_name) on conflict replace
) strict;
