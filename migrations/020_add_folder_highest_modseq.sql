-- The folder's HIGHESTMODSEQ (RFC 7162 CONDSTORE) when its cached flags were
-- last refreshed. The next refresh fetches only flags changed since then; NULL
-- (no CONDSTORE, never refreshed, or reset) means fetch all of them.
ALTER TABLE folders ADD COLUMN highest_modseq INTEGER;
