create table t (a varchar(255), b INTEGER, c "int4", d TIMESTAMP(3), e timestamp with time zone, f timestamptz, g serial, h int[], i text, j character varying(10), k Int, l "integer", m public.int, n timestamp without time zone, o bigint);
alter table t add column p int, alter column q type integer, alter column r set data type timestamp;
alter table t drop constraint x, add constraint y foreign key (a) references u(id);
alter table only "t" drop constraint if exists "x";
alter domain dd drop constraint cc;
