-- Add up migration script here

create table post (
    id uuid primary key,
    title text not null,
    content text not null,
    created_at timestamp default current_timestamp
);

create table outbox (
    id bigserial primary key,
    topic text not null,
    payload jsonb not null,
    created_at timestamp default current_timestamp,
    processed_at timestamp
);

create or replace function notify_outbox_event()
returns trigger as $$
begin
    perform pg_notify('outbox_channel', 'ping');
    return new;
end;
$$ language plpgsql;

create trigger outbox_notify_trigger
after insert on outbox
for each row
execute function notify_outbox_event();
