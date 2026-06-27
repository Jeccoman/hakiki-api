-- Contact-form submissions from the marketing site. Distinct from
-- waitlist_entries: this captures the free-text enquiry, not just a signup.

create table contact_messages (
    id            uuid        primary key,
    name          text        not null,
    email         text        not null,
    organisation  text,
    message       text,
    created_at    timestamptz not null default now()
);
