-- Add down migration script here

drop trigger if exists outbox_notify_trigger on outbox;
drop function if exists notify_outbox_event();
drop table if exists outbox;
drop table if exists post;
